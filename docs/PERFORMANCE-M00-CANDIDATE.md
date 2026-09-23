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
