#!/usr/bin/env python3
"""Compare repeated M00 release benchmarks against a same-host Git baseline."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import tarfile
from datetime import datetime, timezone
from pathlib import Path


BENCH_COMMAND = (
    "bench",
    "-p",
    "forge-kernel",
    "--bench",
    "m00_baselines",
    "--locked",
    "--offline",
)
BENCHMARK_LINE = re.compile(
    r"^benchmark=(?P<name>\S+) samples=(?P<samples>\d+) "
    r"iterations_per_sample=(?P<iterations>\d+) "
    r"median_ns_per_operation=(?P<median>\d+) "
    r"min_ns_per_operation=(?P<minimum>\d+) "
    r"max_ns_per_operation=(?P<maximum>\d+)\s*$",
    re.MULTILINE,
)
CARDINALITY_LINE = re.compile(
    r"^benchmark_sample=(?P<name>\S+) cardinality_start=(?P<start>\d+) "
    r"cardinality_end=(?P<end>\d+) operations=(?P<operations>\d+) "
    r"elapsed_ns=(?P<elapsed>\d+) ns_per_operation=(?P<per_operation>\d+)\s*$",
    re.MULTILINE,
)
COMMON_REQUIRED = {
    "blake3_1k",
    "typed_content_fingerprint_1k",
    "contract_validate",
    "contract_compile",
    "capability_resolution_one_candidate",
    "capability_registration",
    "change_cone_100_node_chain",
    "resource_lease_and_delegation",
    "resource_usage_durable",
    "proof_obligation_compile",
    "proof_minimal_selection",
    "readiness_query_one_check",
    "telemetry_observation",
    "ephemeral_event_fanout",
    "cancellation_lineage_check",
    "cancellation_parent_to_child",
    "runtime_start_and_shutdown",
    "cold_native_boot",
    "warm_native_boot",
    "semantic_cache_lookup",
    "state_transaction_write",
    "state_read",
    "durable_event_outbox",
    "command_dispatch_local_mutation",
    "state_backup_snapshot",
    "state_backup_restore",
}
CANDIDATE_REQUIRED = {
    "change_cone_100_node_chain",
    "change_cone_1000_node_chain",
    "change_cone_10000_node_chain",
    "scheduler_owner_rotation_100",
    "scheduler_owner_rotation_1000",
    "scheduler_owner_rotation_10000",
    "git_exact_change_assessment",
}


def run(command: list[str], cwd: Path, env: dict[str, str]) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if completed.returncode != 0:
        tail = completed.stdout[-12_000:]
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(command)}\n{tail}"
        )
    return completed.stdout


def git_value(root: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(root), *arguments],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"git {' '.join(arguments)} failed: {result.stderr.strip()}")
    return result.stdout.strip()


def parse_output(output: str, expected_inner_samples: int = 5) -> dict[str, object]:
    benchmarks: dict[str, dict[str, int]] = {}
    for match in BENCHMARK_LINE.finditer(output):
        name = match.group("name")
        if name in benchmarks:
            raise RuntimeError(f"benchmark emitted duplicate result: {name}")
        row = {
            "samples": int(match.group("samples")),
            "iterations_per_sample": int(match.group("iterations")),
            "median_ns_per_operation": int(match.group("median")),
            "min_ns_per_operation": int(match.group("minimum")),
            "max_ns_per_operation": int(match.group("maximum")),
        }
        if row["samples"] != expected_inner_samples:
            raise RuntimeError(f"{name}: expected five inner samples, got {row['samples']}")
        benchmarks[name] = row

    cardinality: dict[str, list[dict[str, int]]] = {}
    for match in CARDINALITY_LINE.finditer(output):
        name = match.group("name")
        cardinality.setdefault(name, []).append(
            {
                "cardinality_start": int(match.group("start")),
                "cardinality_end": int(match.group("end")),
                "operations": int(match.group("operations")),
                "elapsed_ns": int(match.group("elapsed")),
                "ns_per_operation": int(match.group("per_operation")),
            }
        )
    return {"benchmarks": benchmarks, "cardinality_samples": cardinality}


def bench_workspace(root: Path, env: dict[str, str], *, no_run: bool) -> str:
    arguments = ["cargo", *BENCH_COMMAND]
    if no_run:
        arguments.append("--no-run")
    return run(arguments, root, env)


def verify_baseline_snapshot(root: Path, baseline: Path, revision: str) -> None:
    object_type = git_value(root, "cat-file", "-t", f"{revision}^{{commit}}")
    if object_type != "commit":
        raise RuntimeError("baseline SHA does not resolve to a commit in the candidate repository")

    archived = subprocess.run(
        ["git", "-C", str(root), "archive", "--format=tar", revision],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if archived.returncode != 0:
        raise RuntimeError(f"could not read baseline Git archive: {archived.stderr.decode('utf-8', 'replace')}")

    expected: dict[str, bytes] = {}
    try:
        with tarfile.open(fileobj=io.BytesIO(archived.stdout), mode="r:") as archive:
            for member in archive.getmembers():
                path = Path(*Path(member.name).parts)
                if path.is_absolute() or ".." in path.parts:
                    raise RuntimeError("baseline Git archive contains an unsafe path")
                if member.isdir():
                    continue
                if not member.isfile():
                    raise RuntimeError(f"baseline Git archive contains a non-regular file: {member.name}")
                content = archive.extractfile(member)
                if content is None:
                    raise RuntimeError(f"could not read baseline archive member: {member.name}")
                expected[member.name.replace("\\", "/")] = content.read()
    except tarfile.TarError as exc:
        raise RuntimeError(f"baseline Git archive is invalid: {exc}") from exc

    actual_paths: set[str] = set()
    for path in baseline.rglob("*"):
        if path.is_symlink():
            raise RuntimeError(f"baseline workspace contains an unexpected symlink: {path}")
        if path.is_file():
            actual_paths.add(path.relative_to(baseline).as_posix())
    expected_paths = set(expected)
    if actual_paths != expected_paths:
        missing = sorted(expected_paths - actual_paths)[:10]
        extra = sorted(actual_paths - expected_paths)[:10]
        raise RuntimeError(f"baseline workspace does not match Git archive; missing={missing}, extra={extra}")
    for relative in sorted(expected_paths):
        actual = (baseline / Path(*Path(relative).parts)).read_bytes()
        if hashlib.sha256(actual).digest() != hashlib.sha256(expected[relative]).digest():
            raise RuntimeError(f"baseline workspace file differs from Git archive: {relative}")


def median_mad_budget(values: list[int]) -> dict[str, int]:
    median = int(statistics.median(values))
    mad = int(statistics.median(abs(value - median) for value in values))
    # 3*MAD is robust to a single noisy run. The observed maximum is a floor,
    # so no baseline sample can fail its own frozen regression budget.
    upper = max(max(values), median + 3 * mad)
    return {
        "median_ns": median,
        "mad_ns": mad,
        "observed_max_ns": max(values),
        "upper_budget_ns": upper,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dir", type=Path, required=True)
    parser.add_argument("--baseline-sha", required=True)
    parser.add_argument("--candidate-dir", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.runs < 5:
        parser.error("at least five outer runs are required")

    candidate = args.candidate_dir.resolve(strict=True)
    baseline = args.baseline_dir.resolve(strict=True)
    if not (candidate / "Cargo.toml").is_file() or not (baseline / "Cargo.toml").is_file():
        parser.error("both workspaces must contain Cargo.toml")
    if not re.fullmatch(r"(?:[0-9a-f]{40}|[0-9a-f]{64})", args.baseline_sha):
        parser.error("baseline SHA must be a full SHA-1 or SHA-256 commit ID")
    candidate_sha = git_value(candidate, "rev-parse", "HEAD")
    status = git_value(candidate, "status", "--porcelain", "--untracked-files=normal")
    if status:
        parser.error("candidate worktree must be clean so measurements bind to its exact HEAD")
    if candidate_sha == args.baseline_sha:
        parser.error("baseline and candidate must be different commits")
    verify_baseline_snapshot(candidate, baseline, args.baseline_sha)

    target_dir = args.target_dir or candidate / "target" / "m00-performance-budget"
    target_dir = target_dir.resolve()
    target_dir.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target_dir)

    toolchain = run(["rustc", "-Vv"], candidate, environment).strip()
    cargo_version = run(["cargo", "-V"], candidate, environment).strip()
    # Compile both workspaces before timing, then alternate runs to avoid giving
    # one revision every first-run or warm-cache advantage.
    bench_workspace(baseline, environment, no_run=True)
    bench_workspace(candidate, environment, no_run=True)

    baseline_runs: list[dict[str, object]] = []
    candidate_runs: list[dict[str, object]] = []
    for run_number in range(1, args.runs + 1):
        baseline_output = bench_workspace(baseline, environment, no_run=False)
        candidate_output = bench_workspace(candidate, environment, no_run=False)
        baseline_runs.append(parse_output(baseline_output))
        candidate_runs.append(parse_output(candidate_output))
        print(f"performance_pair={run_number}/{args.runs} PASS", flush=True)

    baseline_names = set.intersection(
        *(set(run_result["benchmarks"]) for run_result in baseline_runs)
    )
    candidate_names = set.intersection(
        *(set(run_result["benchmarks"]) for run_result in candidate_runs)
    )
    if not COMMON_REQUIRED.issubset(baseline_names & candidate_names):
        missing = sorted(COMMON_REQUIRED - (baseline_names & candidate_names))
        raise RuntimeError(f"missing common required benchmarks: {missing}")
    if not CANDIDATE_REQUIRED.issubset(candidate_names):
        missing = sorted(CANDIDATE_REQUIRED - candidate_names)
        raise RuntimeError(f"missing candidate scaling benchmarks: {missing}")

    comparison: dict[str, object] = {}
    failures: list[str] = []
    for name in sorted(COMMON_REQUIRED):
        baseline_values = [
            result["benchmarks"][name]["median_ns_per_operation"] for result in baseline_runs
        ]
        candidate_values = [
            result["benchmarks"][name]["median_ns_per_operation"] for result in candidate_runs
        ]
        budget = median_mad_budget(baseline_values)
        candidate_summary = median_mad_budget(candidate_values)
        passed = candidate_summary["median_ns"] <= budget["upper_budget_ns"]
        comparison[name] = {
            "baseline_outer_medians_ns": baseline_values,
            "baseline_budget": budget,
            "candidate_outer_medians_ns": candidate_values,
            "candidate_summary": candidate_summary,
            "pass": passed,
        }
        if not passed:
            failures.append(name)

    candidate_new_workloads: dict[str, object] = {}
    for name in sorted(candidate_names - COMMON_REQUIRED):
        values = [
            result["benchmarks"][name]["median_ns_per_operation"] for result in candidate_runs
        ]
        candidate_new_workloads[name] = {
            "outer_medians_ns": values,
            "initial_budget": median_mad_budget(values),
            "historical_comparison": "none; first measured in this correction",
        }

    def medians(name: str) -> list[int]:
        return [
            result["benchmarks"][name]["median_ns_per_operation"] for result in candidate_runs
        ]

    graph_per_node = {
        count: int(statistics.median(medians(f"change_cone_{count}_node_chain"))) / count
        for count in (100, 1_000, 10_000)
    }
    scheduler_per_operation = {
        count: int(statistics.median(medians(f"scheduler_owner_rotation_{count}")))
        for count in (100, 1_000, 10_000)
    }
    ledger_rows = []
    for result in candidate_runs:
        rows = result["cardinality_samples"].get("resource_usage_durable", [])
        if len(rows) != 5:
            raise RuntimeError("resource ledger must report all five cardinality intervals")
        ledger_rows.append(rows)
    ledger_per_interval = []
    for interval_index in range(5):
        values = [rows[interval_index]["ns_per_operation"] for rows in ledger_rows]
        ledger_per_interval.append(
            {
                "cardinality_start": ledger_rows[0][interval_index]["cardinality_start"],
                "cardinality_end": ledger_rows[0][interval_index]["cardinality_end"],
                "outer_medians_ns_per_operation": values,
                "summary": median_mad_budget(values),
            }
        )

    # Graph and scheduler operations use ordered maps/sets. Their logarithmic
    # factor is 2x across 100x cardinality; 3x allows 50% measurement variance
    # while rejecting quadratic growth. Ledger writes update fixed-size totals;
    # a 2x limit across five 200-record intervals rejects history scans.
    graph_ratio = max(graph_per_node.values()) / min(graph_per_node.values())
    scheduler_ratio = max(scheduler_per_operation.values()) / min(scheduler_per_operation.values())
    ledger_medians = [row["summary"]["median_ns"] for row in ledger_per_interval]
    ledger_ratio = max(ledger_medians) / min(ledger_medians)
    graph_limit = 3.0
    scheduler_limit = 3.0
    ledger_limit = 2.0
    scaling_checks = {
        "change_cone_ns_per_node": {
            "values": graph_per_node,
            "max_to_min_ratio": graph_ratio,
            "limit": graph_limit,
            "pass": graph_ratio <= graph_limit,
        },
        "scheduler_rotation_ns_per_operation": {
            "values": scheduler_per_operation,
            "max_to_min_ratio": scheduler_ratio,
            "limit": scheduler_limit,
            "pass": scheduler_ratio <= scheduler_limit,
        },
        "resource_usage_ns_per_write_by_ledger_cardinality": {
            "intervals": ledger_per_interval,
            "max_to_min_ratio": ledger_ratio,
            "limit": ledger_limit,
            "pass": ledger_ratio <= ledger_limit,
        },
    }
    failures.extend(
        name for name, result in scaling_checks.items() if not result["pass"]
    )

    report = {
        "schema_version": 1,
        "work_order": "FGE-004-M00",
        "measurement_time_utc": datetime.now(timezone.utc).isoformat(),
        "baseline_sha": args.baseline_sha,
        "baseline_snapshot_verified_against_git_archive": True,
        "candidate_sha": candidate_sha,
        "candidate_worktree_clean": True,
        "outer_runs": args.runs,
        "inner_samples_per_workload": 5,
        "environment": {
            "platform": platform.platform(),
            "processor": platform.processor(),
            "architecture": platform.machine(),
            "rustc_vv": toolchain,
            "cargo": cargo_version,
        },
        "threshold_method": "median of five outer-run medians plus three median absolute deviations, floored at the highest observed baseline run",
        "common_workload_comparison": comparison,
        "candidate_only_workloads": candidate_new_workloads,
        "scaling_checks": scaling_checks,
        "verdict": "PASS" if not failures else "FAIL",
        "failures": failures,
    }
    serialized = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        output = args.output.resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(serialized, encoding="utf-8")
    print(serialized)
    return 0 if not failures else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as exc:
        print(f"performance budget check failed to run: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
