# FGE-004-M00 Evidence Bundle — implementation candidate

Status: CANDIDATE EVIDENCE; NOT AN APPROVAL OR CHECKPOINT PROMOTION. Implementation head `0ce565eb217ea3dd1d38e31bf16bdecf772c4175` has exact-head CI and a passing ten-pair performance rerun; the earlier failing run is retained. Four fresh package-bound reviews remain required. This bundle does not certify its own documentation revision.

## Identity and authority

- Work Order: `FGE-004-M00` (M00 S01-S22, HIGH risk).
- Admitted base: `5b6c41da4a7a871d0538f17b164fb5e4ebd9b80c`.
- Branch: `fge-004-m00-implementation`; original implementation starting head: `90efa8846b0aaa6615114b4ad363c84b7da46a20`; CD3 implementation candidate: `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac`.
- PR: #9 in `KayzenRoot/forge`; it remains draft and unmerged. Actions run `35890844050` passed all four configured jobs on the exact CD3 candidate SHA; Ubuntu and Windows both passed the hosted outbound-network-blocked doctor check. This run does not certify a later evidence-only commit; PR #9 is the authoritative location for the current exact-head check binding.
- UADS: installed v0.12.1; HIGH plan `wo_b161d2f667649b5c`; pre-CD3 run `er_586620b1f8dd4753`, linked to Codex thread `01a0cafa-b9b8-77b2-b017-f3187d711916`. Its recorded digest `1e7bb0a5d6c779590680c790cf8801c59c5827e7f1af1cf224bd5f9ad6f77889` predates CD3 and is not certification evidence for CD3/CD4. After the CD3 commit, `uads verify --json` returned `no implementation change to verify` on the clean worktree and marked the run `blocked`; fresh digest-bound CD3/CD4 execution evidence is not established. Four distinct assurance sessions remain unproven.
- Prepared Codex contract: schema v0.11.0, adapter contract v0.10.0. The `implement` phase exposes 11 non-review assignments through sequential `role-cycling`; the current `review` bundle has zero active assignments and blocks with `INDEPENDENT_REVIEW_BACKEND_REQUIRED`. Hidden execution/subagent/parallel capability remains `unknown`.
- The nine executable UADS PASS records belong to a pre-CD3 digest and do not certify CD3/CD4. `security-review` and `performance-check` remain pending, with four distinct assurance reviewer sessions missing. Historical failures `fail_0a0885d3ae84936c` and `fail_d24e593c9f468786` remain preserved. Do not treat visible role-cycling as independent review.
- HIVE remains optional. Native Forge boot reports HIVE not configured and semantic embeddings disabled; no external model or embedding API is called.

## C03 correction evidence (historical fda8969 snapshot; superseded by the current overlay at the end)

- Corrected implementation candidate: `fda8969097320cdad9b8f92dffb4fddd8fc2424e`; tree `f6c4c21a7f5e910c85c6bf5844ddb3b294679e13`; baseline `2f95efd4a05ade63dd3e44f3a202c0afe0a46bc4`. PR #9 is currently open, draft, and unmerged. Exact-head Actions run [36270900434](https://github.com/KayzenRoot/forge/actions/runs/36270900434) passed all four configured jobs, including Ubuntu and Windows blocked-egress doctor checks. This run does not certify a later evidence-only commit.
- Source fingerprints: 68 manifest entries, zero mismatches on `fda8969097320cdad9b8f92dffb4fddd8fc2424e`; the final documentation revision will be re-bound and verified before the review package is frozen.
- Performance: ten paired outer runs with five inner samples; 16/16 comparable workloads and all three scale checks passed. New or semantically changed workpaths have proposed candidate budgets only, with no external SLO approval implied. The first five-pair report failed two short workloads; it is preserved beside the passing ten-pair result and its interpretation is documented in [the performance report](../../../docs/PERFORMANCE-M00-CANDIDATE.md).
- H1-H5 and ML1-ML7 correction and regression coverage are summarized in [C03 correction matrix](C03-CORRECTION-MATRIX.md). Four fresh, distinct native Codex reviewers must assess the same final frozen package digest; their reviewer results remain separate from this package snapshot.

## Candidate structure and changes

- `forge-contracts`: contract IDs/versions, JSON Schema validation, typed errors and fingerprints.
- `forge-state`: SQLite canonical state, CAS, schema migration, idempotency, outbox, evidence, disposable cache, verified backup/restore, and append-only resource usage ledger.
- `forge-kernel`: S01-S22 modules; live boot restores durable token/cost usage before granting leases, records usage idempotently, and enforces shared pool ceilings atomically in SQLite. CD3 holds a per-lease gate across usage validation, durable insertion and in-memory application; ambiguous/cancelled transitions mark the governor dirty and block further authority until boot reconciles from the durable ledger.
- `forge-cli`: offline native doctor, file hash and contract validator; `m00_idle_memory` is a benchmark-only helper.
- This CD1 correction also records observed cache lookup/hit/miss counters, fixes optional HIVE readiness semantics, and makes optional telemetry export failure retain local observations.
- Boundary rationale: [ADR-FGE-004-M00-01](../../decisions/FGE-004-M00-RUST-WORKSPACE.md). Dependency policy blocks HTTP crates and declares allowed licenses/sources; no project license was guessed.

## Source inventory and performance

Canonical source SHA-256 values are listed in [SOURCE-FINGERPRINTS.sha256](SOURCE-FINGERPRINTS.sha256). Toolchain: rustc 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), target `x86_64-pc-windows-msvc`, LLVM 22.1.8. Hardware and historical CD3 snapshot are followed by the C03 ten-pair budgeted comparison in [PERFORMANCE-M00-CANDIDATE.md](../../../docs/PERFORMANCE-M00-CANDIDATE.md). The repeated C03 JSON reports and their SHA-256 digests are preserved in the external correction package.

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
- Local UADS gates passed: typecheck, build, 50 files / 415 tests, adapter evaluation (41/41), focused adapter tests (15/15), host execution tests (53/53), and installed Codex bundle preparation. The pre-CD3 Forge run's digest predates the CD3 candidate; after the CD3 commit, `uads verify --json` returned `no implementation change to verify` on the clean worktree and marked the run `blocked`. Its nine executable PASS records do not certify CD3/CD4, and the two reviewer gates remain pending. The historical UADS adapter still cannot establish hidden execution capability and remains `unknown`; it is not treated as the C03 reviewer backend. C03 uses four fresh, distinct native Codex subagent sessions against the frozen evidence-package digest, and records their actual dispatch identities and verdicts separately.
- Pre-edit source/global-skill/dist backup and rollback material: `C:\Users\csn19\.uads\backups\FGE-004-M00-CD2-UADS-Delegation-Recovery-20260923T085322`. The two historical Failure Records remain unchanged.

## Security, cost and assurance limits

- `unsafe_code` is forbidden. Tests cover contract rejection, privacy, path shapes, backup tampering, health freshness, outbox identity, command idempotency, extension scopes, queue/resource bounds, and the new durable usage invariants.
- Resource budgets default to zero until a caller supplies explicit limits. Parent/child lease limits and cumulative durable pool ceilings are enforced; provider billing is not integrated.
- The dependency gates passed locally with the duplicate-version warnings above. This is not an independent security audit.
- Linux and Windows blocked-egress doctor checks passed for implementation candidate `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac` in hosted run `35890844050`; local firewall configuration was denied. Any later evidence-only commit has a separate exact-head check binding through PR #9.
- Independent security, performance, reliability and certification decisions require a proven separate reviewer execution backend. The UADS capability snapshot is `unknown`; do not self-approve.
- M00 remains unapproved. Keep the PR unmerged, canonical checkpoint unchanged, and M01 out of scope.

## C03 current correction overlay — implementation head 0ce565e (2026-09-26)

This overlay supersedes earlier C03 candidate, source-count, schema, performance, and pending-hosted-proof
statements above. Those entries remain historical evidence for their named heads. The documentation
commit containing this overlay must receive its own exact-head Actions result; the implementation run
below cannot certify a later SHA.

- Implementation head/tree: `0ce565eb217ea3dd1d38e31bf16bdecf772c4175` /
  `27e4edd886a8cadf802db1359bef45fe265c920c`; base `5b6c41da4a7a871d0538f17b164fb5e4ebd9b80c`.
- Exact-head [Actions run 36278984106](https://github.com/KayzenRoot/forge/actions/runs/36278984106):
  governance, security/supply-chain, Ubuntu native runtime, and Windows native runtime all passed;
  both hosted outbound-egress-blocked doctor checks passed.
- Source verification passed against this implementation head: 72 raw Git blobs, SHA-1 object format,
  zero errors/mismatches. Build and verification inputs, including `crates/forge-kernel/build.rs`,
  are inventory-bound; a negative fixture proves an unlisted build input is rejected.
- Corrections include schema-v4 host-signed UOR with safe retry/backup semantics, bounded monotonic
  resource-lease expiry and reclamation, secret-safe error persistence, matching staged/published
  durable event IDs, and a private committed-event publication path. Regression evidence is in the
  mapped C03 matrix and exact source snapshot.
- Local Windows checks on the implementation head passed the Rust suite (1 CLI, 6 contracts,
  84 kernel, 22 state, and one doctest), format/clippy, source-fingerprint fixtures, dependency checks,
  release doctor, and all six S21 readiness groups. Doctor reports `native_ready`, integrity `ok`,
  schema v4, HIVE `not_configured`, and embeddings disabled. The local firewall rule was denied, so
  hosted CI is the egress-block evidence.
- Performance reports on this same code head are both preserved: initial ten-pair FAIL SHA-256
  `b8d4cd3396c88409140f24dbc88ad0ea5690984d03234544371eca91ef693273`; ten-pair rerun PASS SHA-256
  `f87b3a5d72f6be92642bd191041140880980f360d935a47a26a50f8f389b08df`. The passing rerun is 16/16
  comparable workloads and 3/3 scaling checks. Resource-lease median was 664 ns/op vs 667 ns/op
  budget (C02 median 445 ns/op); this narrow runner pass and observed overhead are disclosed, not an
  external SLO claim.
- Four fresh, distinct native Codex reviews (security, performance, reliability, certification-systems)
  must assess one frozen package digest. Their verdicts are maintained in an external lock/report;
  the old review sessions and old package digest are not reused.

The package remains candidate evidence only. PR #9 stays open, draft, and unmerged; canonical checkpoint
files remain unchanged; M00 is not promoted and M01 has not started.

## C03 final executor correction overlay — implementation head f03da4a (2026-09-26)

This section supersedes earlier C03 candidate evidence where it differs. It remains candidate
evidence pending final evidence-head CI and the fresh four-review gate.

- Implementation head `f03da4aa02e619b5affcff09baf6fd56337a286e`; final documentation/evidence head,
  source-fingerprint result, CI run and PR snapshot are bound in the external context lock and frozen
  package.
- Code corrections include monotonic committed-effect finalization, exact host-signed local
  transition revision, atomic state/outbox/receipt/final marker, per-producer outbox ordering,
  per-consumer acknowledgements, three-attempt poison-event quarantine, independent backup digest
  trust and CAS inventory validation, and secret-safe payload filtering. SQLite read/modify/write
  paths reserve the writer before outbox sequencing and confirmed-effect staging.
- Local checks passed: format, workspace check, clippy, complete test suite (1 CLI, 6 contracts,
  85 kernel, 26 state, one compile-fail doctest), dependency policy/audit, release doctor and all six
  S21 readiness groups. Local egress blocking was not asserted. Doctor: `native_ready`, schema 7,
  SQLite integrity `ok`, offline ready, optional services unconfigured.
- Performance on `f03da4a`: ten paired outer runs x five inner samples; fixed `baseline median + 20%`
  acceptance; 13/13 comparable workloads and all three scaling checks passed. Two workloads are
  `not_comparable`. Report SHA-256 `4b068854c60575b0a975b8495c956005194070ea381a1be52a44070fbf523b7f`;
  the report and two exact harness snapshots are packaged.
- `cargo deny check` passed with duplicate-version warnings for `cpufeatures`, `hashbrown`, and
  `syn`; all policy categories passed. `cargo audit --deny warnings` passed.

The final exact-head hosted CI and four independent reviewer verdicts are bound only after packaging.
No PR merge, checkpoint edit/promotion, M00 certification or M01 admission is authorized by this
evidence overlay.

## C03 correction evidence overlay — implementation commit e1da083 (2026-09-27)

Current corrected implementation evidence is bound to commit
`e1da083893d9335118519392c1cdb36021cbfc74`, tree
`c5a9d5950af40e43f978d47b89186803121f3615`, on the admitted FGE-004-M00 base. The prior run
36289138683 and round-1 frozen package remain historical evidence for 5e4fd5d only.

- Reliability correction: SQLite writer reservation precedes outbox identity/sequence reads;
  BUSY/LOCKED (including extended result codes) is retryable only for pure-command commit contention;
  concurrent store and command regressions passed.
- Performance correction: final report is PASS with 13/13 comparable checks, 19/19 candidate or
  changed-definition reference gates, 3/3 scaling checks, and repeated idle-memory under its fixed
  budget. The capture report is explicitly capture-only. Full JSON, frozen budgets and harness
  snapshots are in `C03-CORRECTION/`; report hashes are recorded in the performance document and
  correction matrix. The verified baseline archive and matching raw harness snapshots are included
  with the replacement external review package.
- Local exact-code validation: format, workspace check, clippy, all 122 Rust unit tests and one
  compile-fail doctest, cargo-deny, cargo-audit, release build, nine performance-gate tests, 73-entry
  raw source verification and its fixture suite passed. Release doctor and six local S21 groups
  passed; local egress blocking was not asserted.
- Fingerprint inventory retains 73 paths in its admitted order and path-sequence digest. The exact
  implementation-head verifier output is included as a JSON artifact; the final evidence-head verifier
  will be recorded externally after the evidence commit.

The new evidence-head hosted check and fresh four-role independent review have not yet run. The
replacement immutable package must bind one matching current context lock to that head, its PR #9
check, and the same package digest supplied to all four reviewers. No prior reviewer result is reused.
