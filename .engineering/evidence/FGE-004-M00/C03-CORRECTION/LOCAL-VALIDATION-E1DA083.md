# C03 exact-code local validation — e1da083

- Commit: `e1da083893d9335118519392c1cdb36021cbfc74`
- Tree: `c5a9d5950af40e43f978d47b89186803121f3615`
- Platform: Windows 11 build 26200 x86_64; Ryzen 3 4300GE; Rust/Cargo 1.98.1; LLVM 22.1.8.
- All commands below ran with the tracked source at the commit above. This is implementation-code evidence;
  later documentation/evidence changes require their own source verification and exact-head hosted CI.

| Check | Exact invocation | Result / raw output |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | PASS |
| Workspace compilation | `cargo check --workspace --all-targets --locked --offline` | PASS |
| Clippy | `cargo clippy --workspace --all-targets --locked --offline -- -A clippy::pedantic -D warnings` | PASS |
| Workspace tests | `cargo test --workspace --locked --offline` | PASS: 1 CLI + 6 contracts + 87 kernel + 28 state unit tests; one compile-fail doctest |
| Dependency policy | `cargo deny check` | PASS for advisories, bans, licenses, and sources; existing duplicate-version warnings for cpufeatures, hashbrown, and syn |
| Dependency audit | `cargo audit --deny warnings` | PASS: 189 locked dependencies scanned against 1,271 advisories |
| Release binary | `cargo build -p forge-cli --release --locked --offline` | PASS |
| Performance reference unit tests | `python -B .engineering/scripts/test_m00_performance.py` | PASS: 9/9 |
| Raw-Git source verifier | `python -B .engineering/scripts/verify_source_fingerprints.py --repo . --commit e1da083893d9335118519392c1cdb36021cbfc74` | PASS: SHA-1, 73 paths, zero errors/mismatches |
| Source verifier fixtures | `python -B .engineering/scripts/test_source_fingerprints.py` | PASS: deterministic, malformed/missing/traversal, limits, symlink loops, SHA-1/SHA-256, fabricated commit and Git failure fixtures |
| Release doctor | `target/release/forge.exe doctor --data-dir <isolated evidence directory>` | PASS: native_ready, offline_ready, SQLite integrity ok, schema 7, optional integrations unconfigured |
| S21 readiness cases | `python -B .engineering/scripts/test_zdrp_scenarios.py target/release/forge.exe` | PASS: six mapped readiness groups; local outbound-egress assertion was skipped |

Raw output files for the command checks, doctor JSON, source-verification JSON, and S21 scenarios are
stored alongside this document in `C03-CORRECTION/`. The six local S21 cases prove local readiness and
absence handling only. They do not prove blocked outbound networking; the hosted Linux and Windows
egress gates must pass for the final evidence head.

The associated final local performance report is
`performance-final-e1da083.json` (SHA-256
`92cff3738fd17116eab29287aa82fad012f76a163aaaa90062779437d6210e59`): 13/13 comparable workloads,
19/19 frozen-reference gates, 3/3 scaling checks, and the repeated idle-memory budget passed. Its
capture-only input and candidate reference JSON are retained separately.
