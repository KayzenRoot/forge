# M00 candidate performance snapshot

This is a reproducible local measurement, not an approved SLO or regression threshold. It records the five-sample release-profile run of `cargo bench -p forge-kernel --bench m00_baselines --locked --offline` after the CD3 accounting correction, on 2026-09-23.

## Environment and method

- Platform: Windows x86_64, `x86_64-pc-windows-msvc`.
- CPU: AMD Ryzen 3 4300GE with Radeon Graphics, 4 cores / 8 logical processors.
- Rust: 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), LLVM 22.1.8.
- Profile: optimized Cargo bench build. Each row reports median, minimum and maximum from five timed samples, divided by that sample's operation count.
- The benchmark's operation counts vary by workload and appear in the table. These five samples are a small local snapshot; they are not a confidence interval or a frozen budget.
- Idle memory used the release `m00_idle_memory` example with a fresh state directory and schema v2. After the helper reported native boot ready, Windows working set was sampled five times at 500 ms intervals. All five readings were 6,868,992 bytes (6.55 MiB). This is one Windows process measurement, not a cross-platform memory limit.

## Observed values

All latency values are nanoseconds per operation. `resource_usage_durable` exercises the live boot accounting API and SQLite ledger. Cache evidence is reported separately below.

| Workload | Iterations/sample | Median ns/op | Min ns/op | Max ns/op |
|---|---:|---:|---:|---:|
| BLAKE3 hash, 1 KiB | 20,000 | 1,037 | 1,034 | 1,041 |
| Typed content fingerprint, 1 KiB | 20,000 | 1,173 | 1,172 | 1,217 |
| Contract validation, small object | 4,000 | 127 | 127 | 135 |
| Contract compilation, small schema | 400 | 12,021 | 11,897 | 12,268 |
| Capability resolution, one candidate | 20,000 | 1,735 | 1,688 | 2,099 |
| Capability registration plus snapshot | 400 | 2,471 | 2,217 | 2,645 |
| Change cone, 100-node chain | 4,000 | 89,660 | 83,131 | 94,175 |
| Resource lease plus child delegation | 10,000 | 476 | 468 | 482 |
| Durable resource usage accounting | 200 | 7,607,090 | 5,615,543 | 8,602,431 |
| Proof-obligation compilation | 4,000 | 1,180 | 1,156 | 1,231 |
| Minimal proof selection, one obligation | 4,000 | 1,449 | 1,432 | 1,519 |
| Readiness query, one check | 20,000 | 2,200 | 2,153 | 2,209 |
| Telemetry observation | 20,000 | 80 | 78 | 80 |
| Ephemeral event fanout | 4,000 | 874 | 831 | 1,358 |
| Cancellation lineage check | 20,000 | 30 | 30 | 31 |
| Parent cancellation to child observation | 2,000 | 816 | 807 | 839 |
| Runtime create plus orderly shutdown | 20 | 237,785 | 213,255 | 289,780 |
| Cold native boot with new state root | 20 | 50,582,930 | 48,343,220 | 56,401,030 |
| Warm native boot with existing state root | 20 | 25,551,495 | 23,480,570 | 27,858,875 |
| Semantic cache lookup | 2,000 | 69,434 | 62,724 | 76,872 |
| Canonical SQLite transaction write | 400 | 4,030,248 | 3,890,603 | 4,190,578 |
| Canonical SQLite read | 2,000 | 57,951 | 56,296 | 63,301 |
| Durable event outbox publish | 200 | 4,911,357 | 4,350,179 | 6,184,390 |
| Local-mutation command dispatch | 200 | 11,716,391 | 11,114,146 | 12,586,298 |
| SQLite backup snapshot | 1 | 44,377,700 | 42,060,700 | 67,181,900 |
| Backup restore plus integrity check | 4 | 43,949,425 | 41,339,250 | 46,696,675 |

## Cache observation and limits

The exact semantic cache identity `f6e13cc5078ff46865061748350c7bba031350c07cb72a4e8eade14321a754a0` produced 10,000 lookups, 10,000 hits, zero misses, zero stale entries and zero fingerprint mismatches. Token and money savings remain `not_measured`; these counters prove reuse for the repeated input only.

The host did not permit creating a temporary Windows Firewall rule (`Access denied`), so the local `doctor` run proves native readiness with HIVE unconfigured and embeddings disabled, but does not prove blocked outbound networking. The hosted Linux/Windows firewall gates must prove that on the exact pushed head. No multi-run confidence interval, contention scaling curve or frozen performance budget is claimed. The SQLite-backed usage write is the slowest new hot path in this sample at 7,607,090 ns/op median and remains a candidate for future optimization after independent review.


## C03 repeated exact-state comparison (2026-09-26)

This section records the executable comparison added for the C03 correction. It is measured evidence, not an approved external SLO or M00 certification.

- Candidate: `fda8969097320cdad9b8f92dffb4fddd8fc2424e` (tree `f6c4c21a7f5e910c85c6bf5844ddb3b294679e13`); baseline: `2f95efd4a05ade63dd3e44f3a202c0afe0a46bc4`. The baseline archive was verified against its Git commit, and the candidate worktree was clean at measurement time.
- Exact-head hosted validation for the candidate: [Actions run 36270900434](https://github.com/KayzenRoot/forge/actions/runs/36270900434) — all four configured jobs passed; Linux and Windows each passed the hosted outbound-network-blocked doctor gate.
- Environment: Windows 11 x86_64 / AMD64 Family 23 Model 96 Stepping 1 (AMD Ryzen 3 4300GE); Rust 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), LLVM 22.1.8, Cargo 1.98.1.
- Method: ten paired outer runs, five inner samples per workload; baseline ran first on odd pairs and candidate first on even pairs. Both Cargo builds completed before timing, using isolated target directories. Each baseline budget is `max(highest observed baseline outer median, median + 3 × MAD)`; candidate passes when its ten-run median is at or below that budget.
- Result at `2026-09-26T21:13:58.970226Z`: **PASS — 16/16 comparable workload checks**; no common-workload failures.
- Three scaling checks passed: Change Cone per-node ratio 2.296× / 3.0× limit; scheduler per-operation ratio 2.108× / 3.0×; durable-ledger per-write ratio 1.079× / 2.0×.

### Comparable workloads

All values are nanoseconds per operation. “Candidate upper” is the candidate's observed summary (`median + 3 × MAD`, floored at its observed maximum), shown for context; acceptance uses the baseline budget.

| Workload | Baseline budget | Candidate median | Candidate upper | Result |
|---|---:|---:|---:|---|
| `blake3_1k` | 1,052 | 1,037 | 1,118 | PASS |
| `cancellation_lineage_check` | 33 | 32 | 32 | PASS |
| `cancellation_parent_to_child` | 918 | 791 | 806 | PASS |
| `change_cone_100_node_chain` | 89,804 | 46,727 | 51,352 | PASS |
| `contract_compile` | 12,267 | 11,675 | 13,021 | PASS |
| `contract_validate` | 122 | 111 | 120 | PASS |
| `durable_event_outbox` | 4,007,643 | 3,834,548 | 3,953,924 | PASS |
| `readiness_query_one_check` | 2,986 | 2,185 | 2,301 | PASS |
| `resource_lease_and_delegation` | 674 | 477 | 498 | PASS |
| `resource_usage_durable` | 5,051,554 | 4,496,962 | 4,894,429 | PASS |
| `runtime_start_and_shutdown` | 347,093 | 207,737 | 277,001 | PASS |
| `semantic_cache_lookup` | 76,058 | 49,090 | 73,096 | PASS |
| `state_read` | 45,418 | 40,178 | 66,878 | PASS |
| `state_transaction_write` | 3,697,124 | 3,528,449 | 3,734,894 | PASS |
| `telemetry_observation` | 113 | 75 | 79 | PASS |
| `typed_content_fingerprint_1k` | 1,193 | 1,179 | 1,225 | PASS |

### New or semantically changed workloads

These measurements are recorded as candidate baselines only. `not_comparable` workloads changed behavior between C02 and this candidate; candidate-only entries had no prior comparison. None of the proposed budgets imply external SLO approval.

| Workload | Classification | Candidate median | Proposed upper budget | Reason |
|---|---|---:|---:|---|
| `capability_registration` | not_comparable | 2,282 | 2,342 | C02 registered caller-asserted ConformancePassed/Ready values; the correction only permits public Declared/Unknown registration and records verified runtime evidence through a trusted path. |
| `capability_resolution_one_candidate` | not_comparable | 1,749 | 2,622 | C02 resolved a caller-asserted ConformancePassed/Ready candidate; the corrected public fixture is Declared/Unknown because callers can no longer assert provenance. |
| `change_cone_10000_node_chain` | candidate_only | 10,726,355 | 11,562,455 | first measured in this correction |
| `change_cone_1000_node_chain` | candidate_only | 686,343 | 790,210 | first measured in this correction |
| `cold_native_boot` | not_comparable | 56,405,937 | 58,942,032 | The candidate includes schema-v3 shared-ledger integrity, trusted capability evidence, and expanded optional-service diagnostics absent from C02. |
| `command_dispatch_local_mutation` | not_comparable | 11,333,630 | 11,980,181 | The candidate verifies and durably consumes a host-signed scoped decision, then commits state, outbox, and command receipt atomically; C02 did not perform these checks. |
| `ephemeral_event_fanout` | not_comparable | 1,048 | 1,191 | The candidate validates every event payload with the bounded secret filter before fanout; C02 did not perform this validation. |
| `git_exact_change_assessment` | candidate_only | 381,602,075 | 452,094,425 | first measured in this correction |
| `proof_minimal_selection` | not_comparable | 4,869 | 4,992 | C02 selected caller-asserted unsigned Passed nodes; the candidate verifies a backend-authenticated receipt bound to the exact proof and change set. |
| `proof_obligation_compile` | not_comparable | 7,235 | 7,811 | C02 compiled a caller-constructed assessment; the candidate derives changed paths from Git and conservatively derives obligations. |
| `scheduler_owner_rotation_100` | candidate_only | 139 | 183 | first measured in this correction |
| `scheduler_owner_rotation_1000` | candidate_only | 195 | 405 | first measured in this correction |
| `scheduler_owner_rotation_10000` | candidate_only | 293 | 329 | first measured in this correction |
| `state_backup_restore` | not_comparable | 57,897,750 | 70,343,175 | The candidate restore path validates bounded secret-safe payloads and schema-v3 shared-ledger totals; C02 restored the earlier state format and invariants. |
| `state_backup_snapshot` | not_comparable | 40,176,950 | 47,533,400 | The candidate snapshot includes schema-v3 owner, shared-ledger, and authorization tables plus their integrity state; C02 used the earlier schema and data shape. |
| `warm_native_boot` | not_comparable | 29,406,905 | 33,420,275 | The candidate includes schema-v3 shared-ledger integrity, owner heartbeat/recovery, trusted capability evidence, and expanded optional-service diagnostics absent from C02. |

### Repeated-run history and artifacts

The first five-pair run on the same clean candidate SHA returned `FAIL` for `contract_validate` (candidate median 135 ns/op versus a 125 ns/op baseline budget) and `resource_lease_and_delegation` (502 versus 491 ns/op). The ten-pair rerun returned `PASS` for those same workloads: 111 versus 122 ns/op and 477 versus 674 ns/op, respectively. The implementation files for those two methods are unchanged between baseline and candidate; the discrepancy is retained as measurement variance/history rather than reclassified as a semantic change.

- Ten-pair passing report SHA-256: `3ca0a256e7c08318706cf5f190acfe31150290cf66676e9958494eaf47b3e217` (`performance-candidate-fda8969-10runs.json`).
- Five-pair initial failing report SHA-256: `b2301d693189c18af57c8646f8700b19f8726d1645c1fe0fe3bd05a167a77189` (`performance-candidate-fda8969-5runs-fail.json`). Both files are preserved in the external correction evidence directory and included in the frozen evidence package.
- The candidate source fingerprint verifier separately passed with 68 manifest entries, zero mismatches, and the tree recorded above.
- This comparison does not approve SLOs for new/changed workpaths, establish universal latency guarantees, or satisfy the independent performance reviewer gate.
