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
import queue
import re
import statistics
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
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
            # Rust calls commonly end their final argument with a comma.
            # Strip it before selecting the variable whose setup defines
            # this sampled workload region.
            last_argument = arguments.rstrip().rstrip(",").rsplit(",", 1)[-1].strip()
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


def candidate_reference_identity_errors(
    reference: object,
    *,
    candidate_sha: str,
    candidate_tree_sha: str,
    baseline_sha: str,
    outer_runs: int,
    environment: dict[str, str],
) -> list[str]:
    if not isinstance(reference, dict):
        return ["reference_not_object"]

    errors: list[str] = []
    expected = {
        "schema_version": 1,
        "work_order": "FGE-004-M00",
        "reference_kind": "repeated_candidate_baseline_not_external_slo",
        "candidate_sha": candidate_sha,
        "candidate_tree_sha": candidate_tree_sha,
        "baseline_sha": baseline_sha,
        "outer_runs": outer_runs,
        "inner_samples_per_workload": 5,
        "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
        "environment": environment,
    }
    for field, value in expected.items():
        actual = reference.get(field)
        if (type(value) is int and type(actual) is not int) or actual != value:
            errors.append(f"{field}_mismatch")

    workloads = reference.get("workloads")
    if not isinstance(workloads, dict):
        errors.append("workloads_not_object")
    return errors


def candidate_workload_reference_errors(
    reference: object,
    *,
    current_definition_sha256: str,
    current_iterations_per_sample: int,
    current_inner_samples: int,
    expected_outer_runs: int,
) -> list[str]:
    if not isinstance(reference, dict):
        return ["reference_not_object"]

    errors: list[str] = []
    if reference.get("definition_sha256") != current_definition_sha256:
        errors.append("definition_sha256_mismatch")
    if (
        type(reference.get("iterations_per_sample")) is not int
        or reference.get("iterations_per_sample") != current_iterations_per_sample
    ):
        errors.append("iterations_per_sample_mismatch")
    if (
        type(reference.get("inner_samples_per_outer_run")) is not int
        or reference.get("inner_samples_per_outer_run") != current_inner_samples
    ):
        errors.append("inner_samples_per_outer_run_mismatch")

    outer_medians = reference.get("outer_medians_ns")
    if (
        not isinstance(outer_medians, list)
        or len(outer_medians) != expected_outer_runs
        or any(type(value) is not int or value <= 0 for value in outer_medians)
    ):
        errors.append("outer_medians_invalid")
        return errors

    measured_summary = median_mad_budget(outer_medians)
    reference_median = reference.get("reference_median_ns")
    if type(reference_median) is not int or reference_median != measured_summary["median_ns"]:
        errors.append("reference_median_mismatch")
    if reference.get("measurement_summary") != measured_summary:
        errors.append("measurement_summary_mismatch")

    allowance = reference.get("relative_allowance_percent")
    if type(allowance) is not int or allowance != COMMON_RELATIVE_ALLOWANCE_PERCENT:
        errors.append("relative_allowance_mismatch")
    expected_upper_budget = math.ceil(
        measured_summary["median_ns"]
        * (100 + COMMON_RELATIVE_ALLOWANCE_PERCENT)
        / 100
    )
    upper_budget = reference.get("upper_budget_ns")
    if type(upper_budget) is not int or upper_budget != expected_upper_budget:
        errors.append("upper_budget_mismatch")
    return errors


def process_resident_memory_bytes(pid: int) -> tuple[int, str]:
    if sys.platform == "win32":
        import ctypes
        from ctypes import wintypes

        class ProcessMemoryCountersEx(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
                ("PrivateUsage", ctypes.c_size_t),
            ]

        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        kernel32.OpenProcess.restype = wintypes.HANDLE
        kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
        kernel32.CloseHandle.restype = wintypes.BOOL
        psapi.GetProcessMemoryInfo.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(ProcessMemoryCountersEx),
            wintypes.DWORD,
        ]
        psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
        handle = kernel32.OpenProcess(0x0400, False, pid)  # PROCESS_QUERY_INFORMATION
        if not handle:
            raise RuntimeError(f"cannot open idle probe process for memory sampling (winerror={ctypes.get_last_error()})")
        try:
            counters = ProcessMemoryCountersEx()
            counters.cb = ctypes.sizeof(counters)
            if not psapi.GetProcessMemoryInfo(
                handle, ctypes.byref(counters), counters.cb
            ):
                raise RuntimeError(f"cannot sample idle probe working set (winerror={ctypes.get_last_error()})")
            return int(counters.WorkingSetSize), "windows_process_working_set_bytes"
        finally:
            kernel32.CloseHandle(handle)

    if sys.platform.startswith("linux"):
        status = Path(f"/proc/{pid}/status").read_text(encoding="ascii")
        for line in status.splitlines():
            if line.startswith("VmRSS:"):
                parts = line.split()
                if len(parts) >= 2 and parts[1].isdigit():
                    return int(parts[1]) * 1024, "linux_proc_vm_rss_bytes"
                break
        raise RuntimeError("idle probe process has no readable VmRSS value")

    raise RuntimeError(f"idle memory sampling is unsupported on {sys.platform}")


def sample_idle_memory(executable: Path, cwd: Path, env: dict[str, str]) -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix="forge-m00-idle-memory-") as temporary:
        state_root = Path(temporary) / "state"
        process = subprocess.Popen(
            [str(executable), str(state_root), "10"],
            cwd=cwd,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        assert process.stdout is not None
        first_line: queue.Queue[str] = queue.Queue(maxsize=1)
        reader = threading.Thread(
            target=lambda: first_line.put(process.stdout.readline()), daemon=True
        )
        reader.start()
        try:
            try:
                ready = first_line.get(timeout=30)
            except queue.Empty as exc:
                raise RuntimeError("idle memory probe did not report native readiness within 30 seconds") from exc
            match = re.fullmatch(
                r"FORGE_IDLE_READY pid=(?P<pid>\d+) schema=(?P<schema>\d+) boot_fingerprint=(?P<fingerprint>[0-9a-f]{64})\s*\n?",
                ready,
            )
            if match is None or int(match.group("pid")) != process.pid:
                raise RuntimeError(f"idle memory probe did not return a valid native-ready identity: {ready[-300:]!r}")
            samples: list[int] = []
            measurement = ""
            for sample_index in range(5):
                value, measurement = process_resident_memory_bytes(process.pid)
                samples.append(value)
                if sample_index != 4:
                    time.sleep(0.5)
            return {
                "pid": process.pid,
                "schema": int(match.group("schema")),
                "boot_fingerprint": match.group("fingerprint"),
                "samples_bytes": samples,
                "median_bytes": int(statistics.median(samples)),
                "peak_bytes": max(samples),
                "measurement": measurement,
            }
        finally:
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
            reader.join(timeout=1)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-dir", type=Path, required=True)
    parser.add_argument("--baseline-sha", required=True)
    parser.add_argument("--candidate-dir", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--candidate-budget-file", type=Path)
    parser.add_argument(
        "--capture-candidate-budgets",
        type=Path,
        help="write a repeated candidate-only reference; this is a capture, never a PASS verdict",
    )
    args = parser.parse_args()
    if args.runs < 10:
        parser.error("at least ten paired outer runs are required to establish performance budgets")
    if bool(args.candidate_budget_file) == bool(args.capture_candidate_budgets):
        parser.error("provide exactly one of --candidate-budget-file or --capture-candidate-budgets")
    candidate_budget_reference: dict[str, object] | None = None
    candidate_budget_sha256: str | None = None
    if args.candidate_budget_file:
        reference_path = args.candidate_budget_file.resolve(strict=True)
        reference_bytes = reference_path.read_bytes()
        try:
            candidate_budget_reference = json.loads(reference_bytes)
        except json.JSONDecodeError as exc:
            parser.error(f"candidate budget file is invalid JSON: {exc}")
        candidate_budget_sha256 = hashlib.sha256(reference_bytes).hexdigest()
        if (
            not isinstance(candidate_budget_reference, dict)
            or candidate_budget_reference.get("schema_version") != 1
            or candidate_budget_reference.get("work_order") != "FGE-004-M00"
            or candidate_budget_reference.get("relative_allowance_percent")
            != COMMON_RELATIVE_ALLOWANCE_PERCENT
            or not isinstance(candidate_budget_reference.get("workloads"), dict)
        ):
            parser.error("candidate budget file has an unsupported or unbound schema")

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
    measurement_environment = {
        "platform": platform.platform(),
        "processor": platform.processor(),
        "architecture": platform.machine(),
        "rustc_vv": toolchain,
        "cargo": cargo_version,
    }
    # Compile both workspaces before timing. Alternate which source tree runs
    # first on each pair to reduce temporal and warm-cache bias.
    bench_workspace(baseline, baseline_environment, no_run=True)
    bench_workspace(candidate, candidate_environment, no_run=True)
    idle_memory_build = [
        "cargo",
        "build",
        "-p",
        "forge-cli",
        "--example",
        "m00_idle_memory",
        "--release",
        "--locked",
        "--offline",
    ]
    run(idle_memory_build, baseline, baseline_environment)
    run(idle_memory_build, candidate, candidate_environment)
    executable_name = "m00_idle_memory.exe" if os.name == "nt" else "m00_idle_memory"
    baseline_idle_executable = (
        Path(baseline_environment["CARGO_TARGET_DIR"]) / "release" / "examples" / executable_name
    )
    candidate_idle_executable = (
        Path(candidate_environment["CARGO_TARGET_DIR"]) / "release" / "examples" / executable_name
    )
    if not baseline_idle_executable.is_file() or not candidate_idle_executable.is_file():
        raise RuntimeError("release idle memory probe binary was not produced for both source trees")

    baseline_runs: list[dict[str, object]] = []
    candidate_runs: list[dict[str, object]] = []
    baseline_idle_runs: list[dict[str, object]] = []
    candidate_idle_runs: list[dict[str, object]] = []
    for run_number in range(1, args.runs + 1):
        if run_number % 2:
            baseline_output = bench_workspace(baseline, baseline_environment, no_run=False)
            candidate_output = bench_workspace(candidate, candidate_environment, no_run=False)
            baseline_idle = sample_idle_memory(
                baseline_idle_executable, baseline, baseline_environment
            )
            candidate_idle = sample_idle_memory(
                candidate_idle_executable, candidate, candidate_environment
            )
        else:
            candidate_output = bench_workspace(candidate, candidate_environment, no_run=False)
            baseline_output = bench_workspace(baseline, baseline_environment, no_run=False)
            candidate_idle = sample_idle_memory(
                candidate_idle_executable, candidate, candidate_environment
            )
            baseline_idle = sample_idle_memory(
                baseline_idle_executable, baseline, baseline_environment
            )
        baseline_runs.append(parse_output(baseline_output, baseline_workload_definitions))
        candidate_runs.append(parse_output(candidate_output, candidate_workload_definitions))
        baseline_idle_runs.append(baseline_idle)
        candidate_idle_runs.append(candidate_idle)
        print(f"performance_pair={run_number}/{args.runs} measured", flush=True)

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

    candidate_budget_names = set(non_comparable_workloads) | (candidate_names - COMMON_REQUIRED)
    candidate_budget_capture: dict[str, object] | None = None
    candidate_budget_gate: dict[str, object] | None = None
    if args.capture_candidate_budgets:
        captured_workloads: dict[str, object] = {}
        for name in sorted(candidate_budget_names):
            values = [
                result["benchmarks"][name]["median_ns_per_operation"]
                for result in candidate_runs
            ]
            summary = median_mad_budget(values)
            upper_budget = math.ceil(
                summary["median_ns"]
                * (100 + COMMON_RELATIVE_ALLOWANCE_PERCENT)
                / 100
            )
            captured_workloads[name] = {
                "definition_sha256": candidate_runs[0]["benchmarks"][name]["definition_sha256"],
                "iterations_per_sample": candidate_runs[0]["benchmarks"][name]["iterations_per_sample"],
                "inner_samples_per_outer_run": candidate_runs[0]["benchmarks"][name]["samples"],
                "outer_medians_ns": values,
                "reference_median_ns": summary["median_ns"],
                "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
                "upper_budget_ns": upper_budget,
                "measurement_summary": summary,
            }
        candidate_budget_capture = {
            "schema_version": 1,
            "work_order": "FGE-004-M00",
            "reference_kind": "repeated_candidate_baseline_not_external_slo",
            "candidate_sha": candidate_sha,
            "candidate_tree_sha": git_value(candidate, "rev-parse", "HEAD^{tree}"),
            "baseline_sha": args.baseline_sha,
            "outer_runs": args.runs,
            "inner_samples_per_workload": 5,
            "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
            "threshold_method": (
                "Each frozen candidate-reference median receives the same predeclared fixed "
                f"{COMMON_RELATIVE_ALLOWANCE_PERCENT}% allowance as common baseline comparisons; "
                "outer-run variance never widens the budget."
            ),
            "environment": measurement_environment,
            "workloads": captured_workloads,
        }
        capture_path = args.capture_candidate_budgets.resolve()
        capture_path.parent.mkdir(parents=True, exist_ok=True)
        capture_path.write_text(
            json.dumps(candidate_budget_capture, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    else:
        assert candidate_budget_reference is not None
        reference_workloads = candidate_budget_reference["workloads"]
        expected_names = set(candidate_budget_names)
        recorded_names = set(reference_workloads)
        missing_names = sorted(expected_names - recorded_names)
        stale_names = sorted(recorded_names - expected_names)
        for name in missing_names:
            failures.append(f"candidate_budget_missing:{name}")
        for name in stale_names:
            failures.append(f"candidate_budget_stale:{name}")
        candidate_tree_sha = git_value(candidate, "rev-parse", "HEAD^{tree}")
        reference_identity_errors = candidate_reference_identity_errors(
            candidate_budget_reference,
            candidate_sha=candidate_sha,
            candidate_tree_sha=candidate_tree_sha,
            baseline_sha=args.baseline_sha,
            outer_runs=args.runs,
            environment=measurement_environment,
        )
        for error in reference_identity_errors:
            failures.append(f"candidate_budget_reference_{error}")
        reference_identity_matches = not reference_identity_errors
        candidate_budget_gate = {
            "schema_version": candidate_budget_reference["schema_version"],
            "reference_kind": candidate_budget_reference.get("reference_kind"),
            "reference_identity_matches": reference_identity_matches,
            "reference_identity_errors": reference_identity_errors,
            "reference_candidate_sha": candidate_budget_reference.get("candidate_sha"),
            "reference_candidate_tree_sha": candidate_budget_reference.get("candidate_tree_sha"),
            "candidate_sha": candidate_sha,
            "candidate_tree_sha": candidate_tree_sha,
            "reference_baseline_sha": candidate_budget_reference.get("baseline_sha"),
            "baseline_sha": args.baseline_sha,
            "reference_environment": candidate_budget_reference.get("environment"),
            "reference_sha256": candidate_budget_sha256,
            "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
            "workload_results": {},
        }
        for name in sorted(expected_names & recorded_names):
            reference = reference_workloads[name]
            if not isinstance(reference, dict):
                failures.append(f"candidate_budget_invalid:{name}")
                continue
            current_meta = candidate_runs[0]["benchmarks"][name]
            current_values = [
                result["benchmarks"][name]["median_ns_per_operation"]
                for result in candidate_runs
            ]
            current_summary = median_mad_budget(current_values)
            reference_median = reference.get("reference_median_ns")
            allowance = reference.get("relative_allowance_percent")
            upper_budget = reference.get("upper_budget_ns")
            workload_reference_errors = candidate_workload_reference_errors(
                reference,
                current_definition_sha256=current_meta["definition_sha256"],
                current_iterations_per_sample=current_meta["iterations_per_sample"],
                current_inner_samples=current_meta["samples"],
                expected_outer_runs=args.runs,
            )
            valid_reference = reference_identity_matches and not workload_reference_errors
            passed = (
                valid_reference
                and type(upper_budget) is int
                and current_summary["median_ns"] <= upper_budget
            )
            result = {
                "comparison_status": "frozen_candidate_reference",
                "reference_definition_sha256": reference.get("definition_sha256"),
                "current_definition_sha256": current_meta["definition_sha256"],
                "reference_outer_medians_ns": reference.get("outer_medians_ns"),
                "reference_median_ns": reference_median,
                "relative_allowance_percent": allowance,
                "upper_budget_ns": upper_budget,
                "candidate_outer_medians_ns": current_values,
                "candidate_summary": current_summary,
                "reference_valid": valid_reference,
                "reference_errors": workload_reference_errors,
                "pass": passed,
            }
            candidate_budget_gate["workload_results"][name] = result
            if name in non_comparable_workloads:
                non_comparable_workloads[name].update(result)
                non_comparable_workloads[name]["acceptance_status"] = (
                    "PASS against frozen candidate reference"
                    if passed
                    else "FAIL or invalid frozen candidate reference"
                )
            else:
                candidate_new_workloads[name].update(result)
                candidate_new_workloads[name]["acceptance_status"] = (
                    "PASS against frozen candidate reference"
                    if passed
                    else "FAIL or invalid frozen candidate reference"
                )
            if not passed:
                failures.append(f"candidate_budget:{name}")

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

    baseline_idle_peaks = [row["peak_bytes"] for row in baseline_idle_runs]
    candidate_idle_peaks = [row["peak_bytes"] for row in candidate_idle_runs]
    baseline_idle_median = int(statistics.median(baseline_idle_peaks))
    candidate_idle_median = int(statistics.median(candidate_idle_peaks))
    candidate_idle_mad = int(
        statistics.median(abs(value - candidate_idle_median) for value in candidate_idle_peaks)
    )
    idle_memory_upper_budget = math.ceil(
        baseline_idle_median
        * (100 + COMMON_RELATIVE_ALLOWANCE_PERCENT)
        / 100
    )
    idle_memory_pass = candidate_idle_median <= idle_memory_upper_budget
    if len({row["measurement"] for row in baseline_idle_runs + candidate_idle_runs}) != 1:
        failures.append("idle_memory_measurement_method_mismatch")
        idle_memory_pass = False
    idle_memory_comparison = {
        "measurement": baseline_idle_runs[0]["measurement"],
        "baseline_schema_versions": sorted({row["schema"] for row in baseline_idle_runs}),
        "candidate_schema_versions": sorted({row["schema"] for row in candidate_idle_runs}),
        "samples_per_process": 5,
        "interval_ms": 500,
        "outer_runs": args.runs,
        "baseline_outer_peak_bytes": baseline_idle_peaks,
        "baseline_median_peak_bytes": baseline_idle_median,
        "relative_allowance_percent": COMMON_RELATIVE_ALLOWANCE_PERCENT,
        "upper_budget_bytes": idle_memory_upper_budget,
        "candidate_outer_peak_bytes": candidate_idle_peaks,
        "candidate_median_peak_bytes": candidate_idle_median,
        "candidate_mad_peak_bytes": candidate_idle_mad,
        "candidate_max_peak_bytes": max(candidate_idle_peaks),
        "pass": idle_memory_pass,
        "scope": "same-host paired regression budget; not an external memory SLO",
    }
    if not idle_memory_pass:
        failures.append("idle_memory_working_set")

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
            **measurement_environment,
            "idle_memory_measurement": baseline_idle_runs[0]["measurement"],
        },
        "threshold_method": (
            f"candidate median must be at most the paired baseline median plus the predeclared "
            f"{COMMON_RELATIVE_ALLOWANCE_PERCENT}% relative allowance; baseline outliers and "
            "run-specific MAD do not extend the limit. Non-comparable and candidate-only "
            "workloads are gated against a separate frozen ten-run candidate reference."
        ),
        "execution_order": "baseline first on odd-numbered pairs; candidate first on even-numbered pairs",
        "semantic_change_classifications": SEMANTICALLY_CHANGED,
        "common_workload_comparison": comparison,
        "candidate_only_workloads": candidate_new_workloads,
        "non_comparable_workloads": non_comparable_workloads,
        "candidate_budget_reference": (
            {
                "status": "captured_not_validated",
                "path": str(args.capture_candidate_budgets.resolve()),
                "candidate_sha256": hashlib.sha256(
                    args.capture_candidate_budgets.resolve().read_bytes()
                ).hexdigest(),
                "workloads": sorted(candidate_budget_names),
            }
            if args.capture_candidate_budgets
            else candidate_budget_gate
        ),
        "idle_memory_comparison": idle_memory_comparison,
        "scaling_checks": scaling_checks,
        "verdict": (
            "CANDIDATE_REFERENCE_CAPTURE_ONLY"
            if args.capture_candidate_budgets
            else ("PASS" if not failures else "FAIL")
        ),
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
    return 0 if args.capture_candidate_budgets or not failures else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except RuntimeError as exc:
        print(f"performance budget check failed to run: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
