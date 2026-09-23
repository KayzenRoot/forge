# M00 candidate performance baseline

This is a reproducible first measurement snapshot, not an approved SLO or a regression threshold. It records one release-profile run of `cargo bench -p forge-kernel --bench m00_baselines --locked --offline` on 2026-09-23.

## Environment and method

- Platform: Windows x86_64, MSVC target.
- CPU: AMD Ryzen 3 4300GE, 4 cores / 8 logical processors.
- Physical memory: 15.8 GiB.
- Rust: 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), LLVM 22.1.8.
- Profile: Cargo bench optimized build; elapsed wall time divided by the stated operation count.
- Each row is one run with the workload sizes shown in the benchmark executable. There are no confidence intervals, warmup distributions or frozen budgets yet; machine contention and storage state can move the values.

## Observed values

| Workload | Iterations | Mean per operation |
|---|---:|---:|
| BLAKE3 hash, 1 KiB | 100,000 | 1.053 us |
| Typed content fingerprint, 1 KiB | 100,000 | 1.219 us |
| Contract validation, small object | 20,000 | 0.169 us |
| Contract compilation, small schema | 2,000 | 14.979 us |
| Capability resolution, one candidate | 100,000 | 1.803 us |
| Capability registration plus snapshot | 2,000 | 2.917 us |
| Change cone, 100-node chain | 20,000 | 90.765 us |
| Resource lease plus child delegation | 50,000 | 0.447 us |
| Resource charge plus usage attribution | 10,000 | 0.949 us |
| Proof-obligation compilation | 20,000 | 1.262 us |
| Minimal proof selection, one obligation | 20,000 | 1.699 us |
| Readiness query, one check | 100,000 | 2.295 us |
| Telemetry observation | 100,000 | 0.086 us |
| Ephemeral event fanout | 20,000 | 0.944 us |
| Cancellation lineage check | 100,000 | 0.034 us |
| Parent cancellation to child observation | 10,000 | 0.851 us |
| Runtime create plus orderly shutdown | 100 | 218.838 us |
| Cold native boot with new state root | 100 | 49.020 ms |
| Warm native boot with existing state root | 100 | 31.273 ms |
| Semantic cache lookup | 10,000 | 62.456 us |
| Canonical SQLite transaction write | 2,000 | 4.187 ms |
| Canonical SQLite read | 10,000 | 60.396 us |
| Durable event outbox publish | 1,000 | 5.036 ms |
| Local-mutation command dispatch | 1,000 | 12.705 ms |
| SQLite backup snapshot | 1 | 46.798 ms |
| Backup restore plus integrity check | 20 | 49.613 ms |

## Limits and next measurements

Idle resident memory was not measured. The run also does not provide multi-candidate resolver scaling, queue saturation curves, contention/race performance, cache reuse savings, command replay, exporter failure overhead, process crash recovery, or statistical repeatability. No performance budget is frozen from this single machine/run. The command, durable-event and SQLite write paths are the first optimization candidates because their observed per-operation time dominates the in-memory primitives.
