#!/usr/bin/env python3
"""Compare repeated M00 release benchmarks against a same-host Git baseline."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import math
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
    "change_cone_100_node_chain",
    "resource_usage_durable",
    "readiness_query_one_check",
    "telemetry_observation",
    "cancellation_lineage_check",
    "cancellation_parent_to_child",
    "runtime_start_and_shutdown",
    "semantic_cache_lookup",
    "state_transaction_write",
    "state_read",
    "durable_event_outbox",
}
SEMANTICALLY_CHANGED = {
    "capability_registration": (
        "C02 registered caller-asserted ConformancePassed/Ready values; the correction "
        "only permits public Declared/Unknown registration and records verified runtime "
        "evidence through a trusted path."
    ),
    "capability_resolution_one_candidate": (
        "C02 resolved a caller-asserted ConformancePassed/Ready candidate; the corrected "
        "public fixture is Declared/Unknown because callers can no longer assert provenance."
    ),
    "cold_native_boot": (
        "The candidate includes schema-v4 shared-ledger and command-outcome integrity, trusted capability "
        "evidence, and expanded optional-service diagnostics absent from C02."
    ),
    "warm_native_boot": (
        "The candidate includes schema-v4 shared-ledger and command-outcome integrity, owner heartbeat/recovery, "
        "trusted capability evidence, and expanded optional-service diagnostics absent from C02."
    ),
    "ephemeral_event_fanout": (
        "The candidate validates every event payload with the bounded secret filter before "
        "fanout; C02 did not perform this validation."
    ),
    "command_dispatch_local_mutation": (
        "The candidate verifies and durably consumes a host-signed scoped decision, then "
        "commits state, outbox, and command receipt atomically; C02 did not perform these checks."
    ),
    "state_backup_restore": (
        "The candidate restore path validates bounded secret-safe payloads and schema-v4 "
        "shared-ledger and outcome-resolution state; C02 restored the earlier state format and invariants."
    ),
    "state_backup_snapshot": (
        "The candidate snapshot includes schema-v4 owner, shared-ledger, authorization, and outcome-resolution "
        "tables plus their integrity state; C02 used the earlier schema and data shape."
    ),
    "resource_lease_and_delegation": (
        "The candidate adds bounded monotonic TTL and expiry reclamation to lease creation and delegation. "
        "The measured operation does not wait for expiry or exercise reclamation, so its result is a "
        "candidate lease/delegation baseline and is not comparable to the earlier implementation."
    ),
    "proof_obligation_compile": (
        "C02 compiled a caller-constructed assessment; the candidate derives changed paths "
        "from Git and conservatively derives obligations."
    ),
    "proof_minimal_selection": (
        "C02 selected caller-asserted unsigned Passed nodes; the candidate verifies a "
        "backend-authenticated receipt bound to the exact proof and change set."
    ),
}

# A fixed relative allowance is declared before each measurement. Baseline
# outliers and run-specific dispersion never increase this limit.
COMMON_RELATIVE_ALLOWANCE_PERCENT = 20
MINIMUM_COMPARABLE_WORKLOADS = 10
CANDIDATE_REQUIRED = {
    "change_cone_100_node_chain",
    "change_cone_1000_node_chain",
    "change_cone_10000_node_chain",
    "scheduler_owner_rotation_100",
    "scheduler_owner_rotation_1000",
    "scheduler_owner_rotation_10000",
    "git_exact_change_assessment",
    *SEMANTICALLY_CHANGED.keys(),
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


def _call_end(source: str, open_index: int) -> int:
    depth = 0
    index = open_index
    in_string = False
    escaped = False
    in_line_comment = False
    in_block_comment = False
    while index < len(source):
        if in_line_comment:
            if source[index] == "\n":
                in_line_comment = False
        elif in_block_comment:
            if source.startswith("*/", index):
                in_block_comment = False
                index += 1
        elif in_string:
            if escaped:
                escaped = False
            elif source[index] == "\\":
                escaped = True
            elif source[index] == '"':
                in_string = False
        elif source.startswith("//", index):
            in_line_comment = True
            index += 1
        elif source.startswith("/*", index):
            in_block_comment = True
            index += 1
        elif source[index] == '"':
            in_string = True
        elif source[index] == "(":
            depth += 1
        elif source[index] == ")":
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    raise RuntimeError("unterminated benchmark call in Rust harness")


def workload_definition_fingerprints(source_bytes: bytes) -> dict[str, str]:
    source = source_bytes.decode("utf-8")
    definitions: dict[str, str] = {}
    for match in re.finditer(r"\b(measure|report_samples)\s*\(", source):
        open_index = source.find("(", match.start())
        end = _call_end(source, open_index)
        call = source[match.start() : end]
        name_expression = call[call.find("(") + 1 : call.find(",")]
        name_match = re.search(r'"([A-Za-z0-9_{}]+)"', name_expression)
        if not name_match:
            continue
        template = name_match.group(1)
        region_start = match.start()
        if match.group(1) == "report_samples":
            arguments = call[call.find("(") + 1 : -1]
            last_argument = arguments.rsplit(",", 1)[-1].strip()
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", last_argument):
                continue
            declaration = re.search(
                rf"let\s+mut\s+{re.escape(last_argument)}\s*=\s*Vec::new\(\)\s*;",
                source[: match.start()],
            )
            if declaration:
                region_start = declaration.start()
        definition = source[region_start:end].replace("\r\n", "\n").encode("utf-8")
        digest = hashlib.sha256(definition).hexdigest()
        if "{" in template:
            variable = re.search(r"\{([A-Za-z_][A-Za-z0-9_]*)\}", template)
            if variable and variable.group(1) == "node_count":
                for node_count in (100, 1_000, 10_000):
                    definitions[template.replace("{node_count}", str(node_count))] = digest
            elif variable and variable.group(1) == "owner_count":
                for owner_count in (100, 1_000, 10_000):
                    definitions[template.replace("{owner_count}", str(owner_count))] = digest
        else:
            definitions[template] = digest
    return definitions


def parse_output(
    output: str,
    workload_definitions: dict[str, str],
    expected_inner_samples: int = 5,
) -> dict[str, object]:
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
        definition_fingerprint = workload_definitions.get(name)
        if definition_fingerprint is None:
            raise RuntimeError(f"{name}: benchmark source definition was not found in the harness")
        row["definition_sha256"] = definition_fingerprint
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


def fixed_relative_budget(values: list[int]) -> dict[str, int]:
    median = int(statistics.median(values))
    upper = math.ceil(median * (100 + COMMON_RELATIVE_ALLOWANCE_PERCENT) / 100)
    return {
        "baseline_median_ns": median,
        "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
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

    harness_relative_path = Path("crates/forge-kernel/benches/m00_baselines.rs")
    baseline_harness = (baseline / harness_relative_path).read_bytes()
    candidate_harness = (candidate / harness_relative_path).read_bytes()
    harness_sources = {
        "baseline": {
            "git_sha": args.baseline_sha,
            "path": harness_relative_path.as_posix(),
            "sha256": hashlib.sha256(baseline_harness).hexdigest(),
        },
        "candidate": {
            "git_sha": candidate_sha,
            "path": harness_relative_path.as_posix(),
            "sha256": hashlib.sha256(candidate_harness).hexdigest(),
        },
    }
    baseline_workload_definitions = workload_definition_fingerprints(baseline_harness)
    candidate_workload_definitions = workload_definition_fingerprints(candidate_harness)

    target_dir = args.target_dir or candidate / "target" / "m00-performance-budget"
    target_dir = target_dir.resolve()
    target_dir.mkdir(parents=True, exist_ok=True)
    # Give each source tree its own Cargo artifacts. Sharing one target dir lets
    # Cargo reuse a bench executable across the extracted baseline and candidate
    # workspaces, which can silently turn a comparison into two runs of one tree.
    baseline_environment = os.environ.copy()
    baseline_environment["CARGO_TARGET_DIR"] = str(target_dir / "baseline")
    candidate_environment = os.environ.copy()
    candidate_environment["CARGO_TARGET_DIR"] = str(target_dir / "candidate")

    toolchain = run(["rustc", "-Vv"], candidate, candidate_environment).strip()
    cargo_version = run(["cargo", "-V"], candidate, candidate_environment).strip()
    # Compile both workspaces before timing. Alternate which source tree runs
    # first on each pair to reduce temporal and warm-cache bias.
    bench_workspace(baseline, baseline_environment, no_run=True)
    bench_workspace(candidate, candidate_environment, no_run=True)

    baseline_runs: list[dict[str, object]] = []
    candidate_runs: list[dict[str, object]] = []
    for run_number in range(1, args.runs + 1):
        if run_number % 2:
            baseline_output = bench_workspace(baseline, baseline_environment, no_run=False)
            candidate_output = bench_workspace(candidate, candidate_environment, no_run=False)
        else:
            candidate_output = bench_workspace(candidate, candidate_environment, no_run=False)
            baseline_output = bench_workspace(baseline, baseline_environment, no_run=False)
        baseline_runs.append(parse_output(baseline_output, baseline_workload_definitions))
        candidate_runs.append(parse_output(candidate_output, candidate_workload_definitions))
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
    non_comparable_workloads: dict[str, object] = {}
    failures: list[str] = []
    for name in sorted(COMMON_REQUIRED):
        baseline_metadata = [result["benchmarks"][name] for result in baseline_runs]
        candidate_metadata = [result["benchmarks"][name] for result in candidate_runs]
        harness_parity = {
            "samples_match_per_pair": all(
                baseline_row["samples"] == candidate_row["samples"]
                for baseline_row, candidate_row in zip(baseline_metadata, candidate_metadata, strict=True)
            ),
            "iterations_match_per_pair": all(
                baseline_row["iterations_per_sample"] == candidate_row["iterations_per_sample"]
                for baseline_row, candidate_row in zip(baseline_metadata, candidate_metadata, strict=True)
            ),
            "workload_definitions_match": (
                baseline_metadata[0]["definition_sha256"]
                == candidate_metadata[0]["definition_sha256"]
            ),
            "baseline_workload_definition_sha256": baseline_metadata[0]["definition_sha256"],
            "candidate_workload_definition_sha256": candidate_metadata[0]["definition_sha256"],
            "baseline_iterations_per_sample": [row["iterations_per_sample"] for row in baseline_metadata],
            "candidate_iterations_per_sample": [row["iterations_per_sample"] for row in candidate_metadata],
            "baseline_samples_per_outer_run": [row["samples"] for row in baseline_metadata],
            "candidate_samples_per_outer_run": [row["samples"] for row in candidate_metadata],
            "source_files": harness_sources,
        }
        if (
            not harness_parity["samples_match_per_pair"]
            or not harness_parity["iterations_match_per_pair"]
            or not harness_parity["workload_definitions_match"]
        ):
            non_comparable_workloads[name] = {
                "comparison_status": "not_comparable",
                "reason": "baseline and candidate workload definition, operation count, or inner sample count differs",
                "harness_parity": harness_parity,
            }
            continue
        baseline_values = [
            result["benchmarks"][name]["median_ns_per_operation"] for result in baseline_runs
        ]
        candidate_values = [
            result["benchmarks"][name]["median_ns_per_operation"] for result in candidate_runs
        ]
        budget = fixed_relative_budget(baseline_values)
        candidate_summary = median_mad_budget(candidate_values)
        passed = candidate_summary["median_ns"] <= budget["upper_budget_ns"]
        comparison[name] = {
            "baseline_outer_medians_ns": baseline_values,
            "baseline_budget": budget,
            "candidate_outer_medians_ns": candidate_values,
            "candidate_summary": candidate_summary,
            "harness_parity": harness_parity,
            "pass": passed,
        }
        if not passed:
            failures.append(name)

    candidate_new_workloads: dict[str, object] = {}
    for name in sorted(candidate_names - COMMON_REQUIRED):
        values = [
            result["benchmarks"][name]["median_ns_per_operation"] for result in candidate_runs
        ]
        entry: dict[str, object] = {
            "comparison_status": "not_comparable" if name in SEMANTICALLY_CHANGED else "candidate_only",
            "reason": SEMANTICALLY_CHANGED.get(name, "first measured in this correction"),
            "outer_medians_ns": values,
            "proposed_candidate_budget": median_mad_budget(values),
            "acceptance_status": "candidate baseline recorded; no external SLO approval implied",
        }
        if name in SEMANTICALLY_CHANGED and name in baseline_names:
            reference_values = [
                result["benchmarks"][name]["median_ns_per_operation"] for result in baseline_runs
            ]
            entry["historical_reference"] = {
                "baseline_outer_medians_ns": reference_values,
                "baseline_summary": median_mad_budget(reference_values),
                "median_change_percent": round(
                    100 * (statistics.median(values) / statistics.median(reference_values) - 1), 2
                ),
                "interpretation": "reported for context only; workload semantics differ",
            }
        candidate_new_workloads[name] = entry

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
    if len(comparison) < MINIMUM_COMPARABLE_WORKLOADS:
        failures.append("insufficient_comparable_common_workloads")

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
        "benchmark_harness_sources": harness_sources,
        "environment": {
            "platform": platform.platform(),
            "processor": platform.processor(),
            "architecture": platform.machine(),
            "rustc_vv": toolchain,
            "cargo": cargo_version,
        },
        "threshold_method": (
            f"candidate median must be at most the paired baseline median plus the predeclared "
            f"{COMMON_RELATIVE_ALLOWANCE_PERCENT}% relative allowance; baseline outliers and "
            "run-specific MAD do not extend the limit"
        ),
        "execution_order": "baseline first on odd-numbered pairs; candidate first on even-numbered pairs",
        "semantic_change_classifications": SEMANTICALLY_CHANGED,
        "common_workload_comparison": comparison,
        "candidate_only_workloads": candidate_new_workloads,
        "non_comparable_workloads": non_comparable_workloads,
        "scaling_checks": scaling_checks,
        "verdict": "PASS" if not failures else "FAIL",
        "failures": failures,
    }
    if args.output:
        output = args.output.resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        baseline_source_path = output.with_name(f"{output.stem}.baseline-{args.baseline_sha[:7]}.rs")
        candidate_source_path = output.with_name(f"{output.stem}.candidate-{candidate_sha[:7]}.rs")
        baseline_source_path.write_bytes(baseline_harness)
        candidate_source_path.write_bytes(candidate_harness)
        report["benchmark_harness_sources"]["baseline"]["evidence_file"] = baseline_source_path.name
        report["benchmark_harness_sources"]["candidate"]["evidence_file"] = candidate_source_path.name
        serialized = json.dumps(report, indent=2, sort_keys=True) + "\n"
        output.write_text(serialized, encoding="utf-8")
    else:
        serialized = json.dumps(report, indent=2, sort_keys=True) + "\n"
    print(serialized)
    return 0 if not failures else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as exc:
        print(f"performance budget check failed to run: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
