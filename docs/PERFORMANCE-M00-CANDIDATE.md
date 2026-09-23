# M00 candidate performance snapshot

This is a reproducible local measurement, not an approved SLO or regression threshold. It records the five-sample release-profile run of `cargo bench -p forge-kernel --bench m00_baselines --locked --offline` on 2026-09-23.

## Environment and method

- Platform: Windows x86_64, `x86_64-pc-windows-msvc`.
- CPU: AMD Ryzen 3 4300GE with Radeon Graphics, 4 cores / 8 logical processors.
- Rust: 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), LLVM 22.1.8.
- Profile: optimized Cargo bench build. Each row reports median, minimum and maximum from five timed samples, divided by that sample's operation count.
- The benchmark's operation counts vary by workload and appear in the table. These five samples are a small local snapshot; they are not a confidence interval or a frozen budget.
- Idle memory used the release `m00_idle_memory` example with a fresh state directory and schema v2. After the helper reported native boot ready, Windows working set was sampled five times at 500 ms intervals. All five readings were 6,803,456 bytes (6.49 MiB). This is one Windows process measurement, not a cross-platform memory limit.

## Observed values

All latency values are nanoseconds per operation. `resource_usage_durable` exercises the live boot accounting API and SQLite ledger. Cache evidence is reported separately below.

| Workload | Iterations/sample | Median ns/op | Min ns/op | Max ns/op |
|---|---:|---:|---:|---:|
| BLAKE3 hash, 1 KiB | 20,000 | 1,066 | 1,054 | 1,094 |
| Typed content fingerprint, 1 KiB | 20,000 | 1,215 | 1,181 | 1,247 |
| Contract validation, small object | 4,000 | 120 | 119 | 130 |
| Contract compilation, small schema | 400 | 13,015 | 12,780 | 15,768 |
| Capability resolution, one candidate | 20,000 | 1,723 | 1,657 | 2,034 |
| Capability registration plus snapshot | 400 | 3,586 | 3,319 | 3,782 |
| Change cone, 100-node chain | 4,000 | 96,576 | 88,589 | 109,579 |
| Resource lease plus child delegation | 10,000 | 487 | 435 | 657 |
| Durable resource usage accounting | 200 | 6,728,889 | 5,711,500 | 7,508,405 |
| Proof-obligation compilation | 4,000 | 1,386 | 1,304 | 1,436 |
| Minimal proof selection, one obligation | 4,000 | 1,598 | 1,561 | 2,096 |
| Readiness query, one check | 20,000 | 2,367 | 2,214 | 3,057 |
| Telemetry observation | 20,000 | 124 | 101 | 136 |
| Ephemeral event fanout | 4,000 | 1,462 | 1,302 | 1,692 |
| Cancellation lineage check | 20,000 | 31 | 31 | 34 |
| Parent cancellation to child observation | 2,000 | 876 | 836 | 991 |
| Runtime create plus orderly shutdown | 20 | 338,275 | 319,440 | 347,505 |
| Cold native boot with new state root | 20 | 53,191,155 | 47,047,855 | 63,395,630 |
| Warm native boot with existing state root | 20 | 27,473,735 | 24,468,925 | 29,523,910 |
| Semantic cache lookup | 2,000 | 56,772 | 56,208 | 64,666 |
| Canonical SQLite transaction write | 400 | 3,908,491 | 3,685,742 | 5,005,575 |
| Canonical SQLite read | 2,000 | 49,751 | 49,300 | 55,518 |
| Durable event outbox publish | 200 | 4,513,372 | 4,274,054 | 4,847,000 |
| Local-mutation command dispatch | 200 | 11,104,829 | 10,338,656 | 12,017,992 |
| SQLite backup snapshot | 1 | 45,220,200 | 41,915,700 | 60,740,200 |
| Backup restore plus integrity check | 4 | 46,039,600 | 43,116,800 | 55,198,800 |

## Cache observation and limits

The exact semantic cache identity `f6e13cc5078ff46865061748350c7bba031350c07cb72a4e8eade14321a754a0` produced 10,000 lookups, 10,000 hits, zero misses, zero stale entries and zero fingerprint mismatches. Token and money savings remain `not_measured`; these counters prove reuse for the repeated input only.

The host did not permit creating a temporary Windows Firewall rule (`Access denied`), so the local `doctor` run proves native readiness with HIVE unconfigured and embeddings disabled, but does not prove blocked outbound networking. The hosted Linux/Windows firewall gates must prove that on the exact pushed head. No multi-run confidence interval, contention scaling curve or frozen performance budget is claimed. The SQLite-backed usage write is the slowest new hot path in this sample and remains a candidate for future optimization after independent review.
