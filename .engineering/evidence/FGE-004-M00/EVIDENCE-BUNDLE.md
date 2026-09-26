# FGE-004-M00 Evidence Bundle — implementation candidate

Status: CANDIDATE EVIDENCE; NOT AN APPROVAL OR CHECKPOINT PROMOTION. This bundle does not certify its own documentation revision.

## Identity and authority

- Work Order: `FGE-004-M00` (M00 S01-S22, HIGH risk).
- Admitted base: `5b6c41da4a7a871d0538f17b164fb5e4ebd9b80c`.
- Branch: `fge-004-m00-implementation`; original implementation starting head: `90efa8846b0aaa6615114b4ad363c84b7da46a20`; CD3 implementation candidate: `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac`.
- PR: #9 in `KayzenRoot/forge`; it remains draft and unmerged. Actions run `35890844050` passed all four configured jobs on the exact CD3 candidate SHA; Ubuntu and Windows both passed the hosted outbound-network-blocked doctor check. This run does not certify a later evidence-only commit; PR #9 is the authoritative location for the current exact-head check binding.
- UADS: installed v0.12.1; HIGH plan `wo_b161d2f667649b5c`; pre-CD3 run `er_586620b1f8dd4753`, linked to Codex thread `01a0cafa-b9b8-77b2-b017-f3187d711916`. Its recorded digest `1e7bb0a5d6c779590680c790cf8801c59c5827e7f1af1cf224bd5f9ad6f77889` predates CD3 and is not certification evidence for CD3/CD4. After the CD3 commit, `uads verify --json` returned `no implementation change to verify` on the clean worktree and marked the run `blocked`; fresh digest-bound CD3/CD4 execution evidence is not established. Four distinct assurance sessions remain unproven.
- Prepared Codex contract: schema v0.11.0, adapter contract v0.10.0. The `implement` phase exposes 11 non-review assignments through sequential `role-cycling`; the current `review` bundle has zero active assignments and blocks with `INDEPENDENT_REVIEW_BACKEND_REQUIRED`. Hidden execution/subagent/parallel capability remains `unknown`.
- The nine executable UADS PASS records belong to a pre-CD3 digest and do not certify CD3/CD4. `security-review` and `performance-check` remain pending, with four distinct assurance reviewer sessions missing. Historical failures `fail_0a0885d3ae84936c` and `fail_d24e593c9f468786` remain preserved. Do not treat visible role-cycling as independent review.
- HIVE remains optional. Native Forge boot reports HIVE not configured and semantic embeddings disabled; no external model or embedding API is called.

## Candidate structure and changes

- `forge-contracts`: contract IDs/versions, JSON Schema validation, typed errors and fingerprints.
- `forge-state`: SQLite canonical state, CAS, schema migration, idempotency, outbox, evidence, disposable cache, verified backup/restore, and append-only resource usage ledger.
- `forge-kernel`: S01-S22 modules; live boot restores durable token/cost usage before granting leases, records usage idempotently, and enforces shared pool ceilings atomically in SQLite. CD3 holds a per-lease gate across usage validation, durable insertion and in-memory application; ambiguous/cancelled transitions mark the governor dirty and block further authority until boot reconciles from the durable ledger.
- `forge-cli`: offline native doctor, file hash and contract validator; `m00_idle_memory` is a benchmark-only helper.
- This CD1 correction also records observed cache lookup/hit/miss counters, fixes optional HIVE readiness semantics, and makes optional telemetry export failure retain local observations.
- Boundary rationale: [ADR-FGE-004-M00-01](../../decisions/FGE-004-M00-RUST-WORKSPACE.md). Dependency policy blocks HTTP crates and declares allowed licenses/sources; no project license was guessed.

## Source inventory and performance

Canonical source SHA-256 values are listed in [SOURCE-FINGERPRINTS.sha256](SOURCE-FINGERPRINTS.sha256). Toolchain: rustc 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), target `x86_64-pc-windows-msvc`, LLVM 22.1.8. Hardware, five-sample benchmark results, cache counters and idle memory method are in [PERFORMANCE-M00-CANDIDATE.md](../../../docs/PERFORMANCE-M00-CANDIDATE.md).

## IA1 source fingerprint integrity correction

The C01 coverage correction adds subprocess fixtures for reordered manifest paths, a case-only alias in the Git tree, and a missing referenced source blob. All three pass locally on Windows; the existing verifier behavior, source manifest and checkpoint remain unchanged. Scenario details, exact assertions and local results are recorded in the C01 section of the IA1 source fingerprint verification record. The final clean-clone result and exact-head four-job CI binding are recorded in PR #9 after push; C01 does not claim independent review or M00 approval.

The 68-entry inventory is now bound to raw Git blob bytes and its path order is locked. The former 22 checkout-byte mismatches, complete old-to-corrected hash table, deterministic verifier, isolated fixture results and preserved `output/` metadata inventory are recorded in [IA1 source fingerprint verification](IA1-SOURCE-FINGERPRINT-VERIFICATION.md). PR #9 remains draft and unmerged; the exact correction head and its hosted Actions result are recorded in the PR after push. This evidence resolves the source-fingerprint integrity blocker only; independent assurance remains pending, and no M00 approval or checkpoint promotion is claimed.

## Executed local evidence

| Command/check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS on Rust 1.98.1 |
| `cargo check --workspace --all-targets --locked --offline` | PASS |
| `cargo clippy --workspace --all-targets --locked --offline -- -A clippy::pedantic -D warnings` | PASS |
| `cargo test --workspace --locked --offline` | PASS: 95 tests (1 CLI, 6 contracts, 73 kernel, 15 state); doc tests pass (0 defined) |
| `cargo build -p forge-cli --release --locked --offline` | PASS |
| `cargo build -p forge-cli --example m00_idle_memory --release --locked --offline` | PASS |
| `cargo bench -p forge-kernel --bench m00_baselines --locked --offline` | PASS after final CD3 code: five samples/workload; durable usage accounting median 7,607,090 ns/op; exact repeated cache identity 10,000 hits / 0 misses |
| Idle memory helper | PASS after CD3 changes: schema v2; five Windows working-set samples all 6,868,992 bytes (6.55 MiB) |
| `cargo-deny check` (0.20.2) | PASS for advisories, bans, licenses and sources; duplicate `cpufeatures`, `hashbrown` and `syn` warnings remain |
| `cargo-audit audit --deny warnings` (0.22.2) | PASS: 1,267 advisories loaded; 189 locked dependencies scanned |
| Release CLI `doctor` with a fresh temporary state root | PASS locally: `native_ready`, integrity `ok`, schema 2, HIVE `not_configured`, embeddings `disabled`; resource ledger fingerprint returned |
| Local outbound-firewall doctor gate | BLOCKED: `New-NetFirewallRule` returned `Access denied`; the doctor report without a rule is not network-isolation evidence |
| UADS digest verification/evidence preparation | The pre-CD3 run's recorded digest predates CD3; after the CD3 commit, `uads verify --json` returned `no implementation change to verify` on the clean worktree and marked the run `blocked`. No fresh digest-bound CD3/CD4 execution evidence is established. `security-review` and `performance-check` require independent reviewers; four distinct sessions are not recorded |
| GitHub Actions exact-head evidence | Implementation candidate `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac` passed all four configured jobs in run `35890844050`, including Ubuntu and Windows outbound-network-blocked doctor checks. The historical parent run `35878684088` applies only to its parent. A later evidence-only commit must use its own exact-head check binding in PR #9; never transfer the candidate result to a later SHA |

Durable-accounting tests prove migration from schema v1 to v2, exact replay deduplication, restart recovery, concurrent usage writes, the shared SQLite pool ceiling across two boots, and rollback of an uncommitted write after child-process termination. CD3 adds deterministic proof that cloned-lease delegation is blocked during accounting, and that cancellation, revocation, persistence failure or conflicting replay fail closed and reconcile from the durable ledger on restart. Cache hit counts do not prove tokens or money saved; those values are `not_measured`.

## UADS delegation repair record

- Owning package: source at `D:\Projetos Codex\uads`, exposed to the installed CLI by the `C:\Users\csn19\AppData\Roaming\npm\node_modules\uads` junction. Source starting head: `ad2e6c8ec8d416bf4c7d4b80d41fb24c63835699`; no UADS commit or push is authorized.
- Bundle/schema is v0.11.0 while the adapter contract remains v0.10.0. Run phase and status derive the active assignment list; implement phase excludes reviewers; hidden execution capability must be runtime-proven before reviewer assignments or assurance handoff can be accepted.
- Re-preparation previously compared the live refreshed repository index with the frozen specialist-selection digest, so an in-scope implementation edit was misclassified as stale planning. The adapter now keeps the live index in the bundle while validating selection against its frozen Work Order/Context Plan bindings; evaluation AD41 proves re-preparation after an edit, empty assignments in `verify`, and fail-closed `review` when hidden execution is unknown.
- Local UADS gates passed: typecheck, build, 50 files / 415 tests, adapter evaluation (41/41), focused adapter tests (15/15), host execution tests (53/53), and installed Codex bundle preparation. The pre-CD3 Forge run's digest predates the CD3 candidate; after the CD3 commit, `uads verify --json` returned `no implementation change to verify` on the clean worktree and marked the run `blocked`. Its nine executable PASS records do not certify CD3/CD4, and the two reviewer gates remain pending. The review packet identifies the required distinct reviewers, and hidden execution capability remains `unknown`.
- Pre-edit source/global-skill/dist backup and rollback material: `C:\Users\csn19\.uads\backups\FGE-004-M00-CD2-UADS-Delegation-Recovery-20260923T085322`. The two historical Failure Records remain unchanged.

## Security, cost and assurance limits

- `unsafe_code` is forbidden. Tests cover contract rejection, privacy, path shapes, backup tampering, health freshness, outbox identity, command idempotency, extension scopes, queue/resource bounds, and the new durable usage invariants.
- Resource budgets default to zero until a caller supplies explicit limits. Parent/child lease limits and cumulative durable pool ceilings are enforced; provider billing is not integrated.
- The dependency gates passed locally with the duplicate-version warnings above. This is not an independent security audit.
- Linux and Windows blocked-egress doctor checks passed for implementation candidate `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac` in hosted run `35890844050`; local firewall configuration was denied. Any later evidence-only commit has a separate exact-head check binding through PR #9.
- Independent security, performance, reliability and certification decisions require a proven separate reviewer execution backend. The UADS capability snapshot is `unknown`; do not self-approve.
- M00 remains unapproved. Keep the PR unmerged, canonical checkpoint unchanged, and M01 out of scope.
