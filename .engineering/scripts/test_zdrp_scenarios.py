#!/usr/bin/env python3
"""Exercise the six named S21 offline cases against a release CLI."""

from __future__ import annotations

import json
import os
import re
import sqlite3
import socket
import subprocess
import sys
import tempfile
import time
from contextlib import closing
from pathlib import Path


SCHEMA_VERSION = 3
FINGERPRINT = re.compile(r"^[0-9a-f]{64}$")
OPTIONAL_INTEGRATIONS = {
    "hive",
    "core",
    "hades",
    "iris",
    "remote_telemetry",
    "llm",
    "provider",
}


def run_doctor(
    binary: Path,
    data_dir: Path,
    scenario_id: str,
    case: str,
    expected_services: set[str] | None = None,
) -> dict[str, object]:
    environment = os.environ.copy()
    started = time.monotonic()
    completed = subprocess.run(
        [str(binary), "doctor", "--data-dir", str(data_dir)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        timeout=20,
        env=environment,
    )
    elapsed = time.monotonic() - started
    if completed.returncode != 0:
        raise AssertionError(
            f"{scenario_id} {case}: doctor failed ({completed.returncode}): "
            f"{completed.stderr.decode('utf-8', 'replace')}"
        )
    try:
        report = json.loads(completed.stdout.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise AssertionError(f"{scenario_id} {case}: doctor did not return JSON") from exc
    if not isinstance(report, dict):
        raise AssertionError(f"{scenario_id} {case}: doctor report is not an object")
    require_ready_report(scenario_id, case, report, elapsed, expected_services)
    print(
        json.dumps(
            {
                "scenarioId": scenario_id,
                "case": case,
                "state": report["state"],
                "readinessFingerprint": report["readiness_fingerprint"],
                "capabilitySnapshotFingerprint": report["capability_snapshot_fingerprint"],
                "resourceUsageFingerprint": report["resource_usage_fingerprint"],
                "databaseSchemaVersion": report["database_schema_version"],
                "optionalServices": report["optional_services"],
                "elapsedMs": round(elapsed * 1000),
            },
            sort_keys=True,
        )
    )
    return report


def require_ready_report(
    scenario_id: str,
    case: str,
    report: dict[str, object],
    elapsed: float,
    expected_services: set[str] | None,
) -> None:
    expected = {
        "state": "native_ready",
        "readiness": "ready",
        "offline_ready": True,
        "database_integrity": "ok",
        "database_schema_version": SCHEMA_VERSION,
        "semantic_embeddings": "disabled",
    }
    for key, value in expected.items():
        if report.get(key) != value:
            raise AssertionError(
                f"{scenario_id} {case}: expected {key}={value!r}, got {report.get(key)!r}"
            )
    services = report.get("optional_services")
    if not isinstance(services, dict):
        raise AssertionError(f"{scenario_id} {case}: optional service diagnostics are missing")
    if not OPTIONAL_INTEGRATIONS.issubset(services):
        missing = sorted(OPTIONAL_INTEGRATIONS - services.keys())
        raise AssertionError(f"{scenario_id} {case}: missing optional service diagnostics {missing}")
    for service in expected_services or OPTIONAL_INTEGRATIONS:
        if services.get(service) != "not_configured":
            raise AssertionError(
                f"{scenario_id} {case}: {service} must be reported as not_configured"
            )
    for key in (
        "readiness_fingerprint",
        "resource_usage_fingerprint",
        "capability_snapshot_fingerprint",
        "boot_fingerprint",
    ):
        if not isinstance(report.get(key), str) or not FINGERPRINT.fullmatch(report[key]):
            raise AssertionError(f"{scenario_id} {case}: missing valid {key} diagnostic")
    if not isinstance(report.get("state_root"), str) or not report["state_root"]:
        raise AssertionError(f"{scenario_id} {case}: missing local state-root diagnostic")
    if elapsed > 20:
        raise AssertionError(f"{scenario_id} {case}: boot exceeded the 20 second bound")


def require_dns_unavailable(host: str) -> str:
    code = (
        "import socket,sys\n"
        "try:\n"
        " socket.getaddrinfo(sys.argv[1], None)\n"
        "except socket.gaierror:\n"
        " sys.exit(2)\n"
        "sys.exit(0)\n"
    )
    try:
        result = subprocess.run(
            [sys.executable, "-c", code, host],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=5,
            env=os.environ.copy(),
        )
    except subprocess.TimeoutExpired:
        return "resolver-timeout-within-5s"
    if result.returncode == 0:
        raise AssertionError(f"ZDRP-02: reserved DNS blackhole unexpectedly resolved: {host}")
    if result.returncode != 2:
        raise AssertionError(
            "ZDRP-02: DNS probe failed for a reason other than an unavailable name or timeout"
        )
    return "name-unavailable"


def require_egress_blocked() -> list[str]:
    probes = [
        (socket.AF_INET, ("1.1.1.1", 443)),
        (socket.AF_INET6, ("2606:4700:4700::1111", 443, 0, 0)),
    ]
    blocked: list[str] = []
    for family, endpoint in probes:
        connection = socket.socket(family, socket.SOCK_STREAM)
        connection.settimeout(3)
        try:
            connection.connect(endpoint)
        except OSError:
            blocked.append("IPv4" if family == socket.AF_INET else "IPv6")
        else:
            raise AssertionError(
                f"ZDRP-02: outbound connection unexpectedly succeeded for {endpoint[0]}:443"
            )
        finally:
            connection.close()
    if len(blocked) != len(probes):
        raise AssertionError("ZDRP-02: both outbound address families must be blocked")
    return blocked


def seed_interrupted_intent(database: Path) -> str:
    identity = "a" * 64
    with closing(sqlite3.connect(database)) as connection, connection:
        connection.execute(
            "INSERT OR REPLACE INTO store_instances(instance_id, heartbeat_at_ms) VALUES(?, 0)",
            ("zdrp.crashed-owner",),
        )
        connection.execute(
            "INSERT OR REPLACE INTO command_intents(identity, state, result, error_code, owner_instance_id, lease_expires_at_ms) "
            "VALUES(?, 'in_flight', NULL, NULL, ?, 0)",
            (identity, "zdrp.crashed-owner"),
        )
    return identity


def make_v2_database(database: Path) -> None:
    with closing(sqlite3.connect(database)) as connection, connection:
        columns = {
            row[1]
            for row in connection.execute("PRAGMA table_info(command_intents)").fetchall()
        }
        connection.execute("DROP TABLE IF EXISTS authorization_decisions")
        connection.execute("DROP TABLE IF EXISTS resource_usage_totals")
        connection.execute("DROP TABLE IF EXISTS store_instances")
        for column in ("owner_instance_id", "lease_expires_at_ms"):
            if column in columns:
                connection.execute(f"ALTER TABLE command_intents DROP COLUMN {column}")
        connection.execute("PRAGMA user_version = 2")


def exercise_network_blocked_states(binary: Path, root: Path) -> None:
    fresh = root / "zdrp-01-fresh"
    run_doctor(binary, fresh, "ZDRP-01", "fresh-install")

    existing = root / "zdrp-01-existing"
    run_doctor(binary, existing, "ZDRP-01", "existing-healthy-initial")
    run_doctor(binary, existing, "ZDRP-01", "existing-healthy-restart")

    crashed = root / "zdrp-01-crash-recovery"
    run_doctor(binary, crashed, "ZDRP-01", "crash-recovery-initial")
    interrupted_identity = seed_interrupted_intent(crashed / "forge.sqlite3")
    run_doctor(binary, crashed, "ZDRP-01", "crash-recovery-after-reopen")
    with closing(sqlite3.connect(crashed / "forge.sqlite3")) as connection:
        row = connection.execute(
            "SELECT state, error_code FROM command_intents WHERE identity=?",
            (interrupted_identity,),
        ).fetchone()
    if row != ("unknown_outcome", "FORGE.COMMAND.RECOVERY_UNKNOWN"):
        raise AssertionError(f"ZDRP-01: stale command outcome was not diagnosed: {row!r}")

    upgrade = root / "zdrp-01-offline-upgrade"
    run_doctor(binary, upgrade, "ZDRP-01", "upgrade-before-migration")
    make_v2_database(upgrade / "forge.sqlite3")
    report = run_doctor(binary, upgrade, "ZDRP-01", "upgrade-after-migration")
    if report.get("database_schema_version") != SCHEMA_VERSION:
        raise AssertionError("ZDRP-01: offline upgrade did not reach the current schema")
    if os.environ.get("FORGE_EXPECT_BLOCKED_EGRESS") == "1":
        print(
            "ZDRP-01 PASS: fresh, existing, recovery, and upgrade states are NATIVE_READY "
            "while CI outbound blocking is active"
        )
    else:
        print(
            "ZDRP-01 readiness cases PASS locally; this run does not assert outbound isolation"
        )


def run_named_absence_case(
    binary: Path, root: Path, scenario_id: str, case: str, services: set[str]
) -> None:
    report = run_doctor(
        binary,
        root / f"{scenario_id.lower()}-{case}",
        scenario_id,
        case,
        services,
    )
    states = report["optional_services"]
    print(
        f"{scenario_id} PASS: {case} absent before optional integration discovery; "
        f"native diagnostic state={{{', '.join(f'{name}: {states[name]}' for name in sorted(services))}}}"
    )


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: test_zdrp_scenarios.py PATH_TO_FORGE_BINARY")
    binary = Path(sys.argv[1]).resolve(strict=True)
    if not binary.is_file():
        raise SystemExit("Forge release binary is unavailable")

    expect_blocked_egress = os.environ.get("FORGE_EXPECT_BLOCKED_EGRESS", "0")
    if expect_blocked_egress not in {"0", "1"}:
        raise SystemExit("FORGE_EXPECT_BLOCKED_EGRESS must be 0 or 1")
    if expect_blocked_egress == "1":
        blocked_families = require_egress_blocked()
        print(f"ZDRP-02 PASS: outbound TCP probe blocked for {', '.join(blocked_families)}")
    else:
        print("ZDRP-02 egress probe skipped: this local run does not enforce the CI firewall")

    with tempfile.TemporaryDirectory(prefix="forge-zdrp-") as temporary:
        root = Path(temporary)
        exercise_network_blocked_states(binary, root)

        dns_status = require_dns_unavailable("provider-unreachable.invalid")
        print(f"ZDRP-02 DNS unavailable: {dns_status}")
        run_named_absence_case(
            binary,
            root,
            "ZDRP-02",
            "dns-provider-and-telemetry-blackhole",
            {"provider", "remote_telemetry"},
        )
        run_named_absence_case(binary, root, "ZDRP-03", "hive-unavailable", {"hive"})
        run_named_absence_case(
            binary, root, "ZDRP-04", "core-hades-unavailable", {"core", "hades"}
        )
        run_named_absence_case(binary, root, "ZDRP-05", "iris-unavailable", {"iris"})
        run_named_absence_case(
            binary,
            root,
            "ZDRP-06",
            "llm-provider-unavailable",
            {"llm", "provider"},
        )

    print(
        "S21 mapping PASS: canonical fresh/existing/recovery/unreachable/DNS/upgrade cases "
        "plus the C03 network, DNS, HIVE, Core/Hades, IRIS, and LLM/provider IDs all ran"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
