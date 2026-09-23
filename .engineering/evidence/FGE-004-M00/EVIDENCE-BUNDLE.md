# FGE-004-M00 Evidence Bundle — implementation candidate

Status: CANDIDATE EVIDENCE; NOT AN APPROVAL OR CHECKPOINT PROMOTION

## Identity and authority

- Work Order: `FGE-004-M00` (M00 S01-S22, HIGH risk).
- Admitted base: `5b6c41da4a7a871d0538f17b164fb5e4ebd9b80c`.
- Branch: `fge-004-m00-implementation`; implementation starting head: `90efa8846b0aaa6615114b4ad363c84b7da46a20`.
- PR: #9 in `KayzenRoot/forge`; it remains unmerged. The final pushed candidate SHA and exact-head CI results must be recorded in the PR description after push.
- UADS: installed v0.12.1; fresh HIGH plan `wo_b161d2f667649b5c`; fresh run `er_586620b1f8dd4753`, final phase `review` / status `in_progress`, current Codex thread `01a0cafa-b9b8-77b2-b017-f3187d711916`.
- Prepared Codex contract: schema v0.11.0, adapter contract v0.10.0. The `implement` phase exposes 11 non-review assignments through sequential `role-cycling`; the current `review` bundle has zero active assignments and blocks with `INDEPENDENT_REVIEW_BACKEND_REQUIRED`. Hidden execution/subagent/parallel capability remains `unknown`.
- UADS binds all nine executable gate PASS records to the final candidate digest; `security-review` and `performance-check` remain PENDING, with four distinct assurance reviewer sessions missing. Historical failures `fail_0a0885d3ae84936c` and `fail_d24e593c9f468786` remain preserved. Do not treat visible role-cycling as independent review.
- HIVE remains optional. Native Forge boot reports HIVE not configured and semantic embeddings disabled; no external model or embedding API is called.

## Candidate structure and changes

- `forge-contracts`: contract IDs/versions, JSON Schema validation, typed errors and fingerprints.
- `forge-state`: SQLite canonical state, CAS, schema migration, idempotency, outbox, evidence, disposable cache, verified backup/restore, and append-only resource usage ledger.
- `forge-kernel`: S01-S22 modules; live boot restores durable token/cost usage before granting leases, records usage idempotently, and enforces shared pool ceilings atomically in SQLite.
- `forge-cli`: offline native doctor, file hash and contract validator; `m00_idle_memory` is a benchmark-only helper.
- This CD1 correction also records observed cache lookup/hit/miss counters, fixes optional HIVE readiness semantics, and makes optional telemetry export failure retain local observations.
- Boundary rationale: [ADR-FGE-004-M00-01](../../decisions/FGE-004-M00-RUST-WORKSPACE.md). Dependency policy blocks HTTP crates and declares allowed licenses/sources; no project license was guessed.

## Source inventory and performance

Canonical source SHA-256 values are listed in [SOURCE-FINGERPRINTS.sha256](SOURCE-FINGERPRINTS.sha256). Toolchain: rustc 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), target `x86_64-pc-windows-msvc`, LLVM 22.1.8. Hardware, five-sample benchmark results, cache counters and idle memory method are in [PERFORMANCE-M00-CANDIDATE.md](../../../docs/PERFORMANCE-M00-CANDIDATE.md).

## Executed local evidence

| Command/check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS on Rust 1.98.1 |
| `cargo check --workspace --all-targets --locked --offline` | PASS |
| `cargo clippy --workspace --all-targets --locked --offline -- -A clippy::pedantic -D warnings` | PASS |
| `cargo test --workspace --locked --offline` | PASS: 88 tests (1 CLI, 6 contracts, 66 kernel, 15 state); doc tests pass (0 defined) |
| `cargo build -p forge-cli --release --locked --offline` | PASS |
| `cargo build -p forge-cli --example m00_idle_memory --release --locked --offline` | PASS |
| `cargo bench -p forge-kernel --bench m00_baselines --locked --offline` | PASS: five samples/workload; usage-ledger write median 6,728,889 ns/op; exact repeated cache identity 10,000 hits / 0 misses |
| Idle memory helper | PASS: schema v2; five Windows working-set samples all 6,803,456 bytes (6.49 MiB) |
| `cargo deny check` (0.20.2) | PASS for advisories, bans, licenses and sources; duplicate `cpufeatures`, `hashbrown` and `syn` warnings remain |
| `cargo audit --deny warnings` (0.22.2) | PASS: 1,267 advisories loaded; 189 locked dependencies scanned |
| Release CLI `doctor` with a fresh state root | PASS locally: `native_ready`, integrity `ok`, schema 2, HIVE `not_configured`, embeddings `disabled`; resource ledger fingerprint returned |
| Local outbound-firewall doctor gate | BLOCKED: `New-NetFirewallRule` returned `Access denied`; the doctor report without a rule is not network-isolation evidence |
| UADS `verify`, evidence ledger and `assurance start` | Nine executable gates have digest-bound PASS evidence; `security-review` and `performance-check` remain PENDING. Assurance packet requires four distinct reviewers; no reviewer verdict is recorded |
| GitHub Actions on the final candidate head | PENDING; must be refreshed after push and bound to the exact SHA |

Durable-accounting tests prove migration from schema v1 to v2, exact replay deduplication, restart recovery, concurrent usage writes, the shared SQLite pool ceiling across two boots, and rollback of an uncommitted write after child-process termination. Cache hit counts do not prove tokens or money saved; those values are `not_measured`.

## UADS delegation repair record

- Owning package: source at `D:\Projetos Codex\uads`, exposed to the installed CLI by the `C:\Users\csn19\AppData\Roaming\npm\node_modules\uads` junction. Source starting head: `ad2e6c8ec8d416bf4c7d4b80d41fb24c63835699`; no UADS commit or push is authorized.
- Bundle/schema is v0.11.0 while the adapter contract remains v0.10.0. Run phase and status derive the active assignment list; implement phase excludes reviewers; hidden execution capability must be runtime-proven before reviewer assignments or assurance handoff can be accepted.
- Re-preparation previously compared the live refreshed repository index with the frozen specialist-selection digest, so an in-scope implementation edit was misclassified as stale planning. The adapter now keeps the live index in the bundle while validating selection against its frozen Work Order/Context Plan bindings; evaluation AD41 proves re-preparation after an edit, empty assignments in `verify`, and fail-closed `review` when hidden execution is unknown.
- Local UADS gates passed: typecheck, build, 50 files / 415 tests, adapter evaluation (41/41), focused adapter tests (15/15), host execution tests (53/53), and installed Codex bundle preparation. On the fresh Forge run, UADS binds command/file evidence to the candidate digest; nine executable gates are PASS and the two reviewer gates remain PENDING. The review packet identifies the required distinct reviewers, and hidden execution capability remains `unknown`.
- Pre-edit source/global-skill/dist backup and rollback material: `C:\Users\csn19\.uads\backups\FGE-004-M00-CD2-UADS-Delegation-Recovery-20260923T085322`. The two historical Failure Records remain unchanged.

## Security, cost and assurance limits

- `unsafe_code` is forbidden. Tests cover contract rejection, privacy, path shapes, backup tampering, health freshness, outbox identity, command idempotency, extension scopes, queue/resource bounds, and the new durable usage invariants.
- Resource budgets default to zero until a caller supplies explicit limits. Parent/child lease limits and cumulative durable pool ceilings are enforced; provider billing is not integrated.
- The dependency gates passed locally with the duplicate-version warnings above. This is not an independent security audit.
- Linux and exact-head Windows blocked-egress proof remain pending hosted CI. Local firewall configuration was denied.
- Independent security, performance, reliability and certification decisions require a proven separate reviewer execution backend. The UADS capability snapshot is `unknown`; do not self-approve.
- M00 remains unapproved. Keep the PR unmerged, canonical checkpoint unchanged, and M01 out of scope.
