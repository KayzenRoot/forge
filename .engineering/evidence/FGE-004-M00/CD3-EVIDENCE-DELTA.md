# FGE-004-M00 CD3 — Accounting Atomicity Evidence Delta

Status: LOCAL CORRECTION QUALIFIED; NOT CERTIFIED OR APPROVED

## Identity

- Work Order: `FGE-004-M00-CD3-Accounting-Atomicity-Evidence-Assurance`.
- Branch: `fge-004-m00-implementation`.
- Admitted parent / PR #9 head: `58a0616da55d574e7e556056f0e47cc46571d7c7`.
- Base: `5b6c41da4a7a871d0538f17b164fb5e4ebd9b80c`.
- Implementation candidate: `4d6109024d652fd0ac7454e14eeb5aaa9f5113ac`. PR #9 must remain draft and unmerged. Parent Actions run `35878684088` is evidence for the parent only; exact-head run `35890844050` passed all four configured jobs on the implementation candidate, including Ubuntu and Windows outbound-network-blocked doctor checks. Any later evidence-only commit must use its own PR #9 exact-head check binding and does not inherit this result.

## Correction

The previous live-accounting sequence validated a lease, awaited a durable SQLite insert, then mutated the lease's shared in-memory state. A clone could delegate while that await was in progress. Cancellation, revocation, or an ambiguous storage result could also leave a durable row without a matching live charge.

CD3 holds a per-lease asynchronous accounting gate from validation through durable insertion and in-memory charge. Clones share the gate through their lease node; `ensure_active` and delegation report `AccountingInProgress` during the transition. The existing SQLite transaction still enforces the shared durable pool ceiling across boots. Exact replay of a record already applied in memory is a no-op; conflicting identity/content fails closed.

An armed accounting guard marks the live governor dirty if cancellation, persistence ambiguity, or a post-insert failure leaves the transition uncertain. Dirty state rejects new leases and further lease activity with `AccountingReconciliationRequired`; a fresh native boot replays the immutable ledger before granting new authority. Revocation may race with persistence; if the durable row lands but the lease can no longer be charged, the governor fails closed and restart reconciles the row.

## Local verification

- Focused `forge-kernel` boot accounting tests: 10 passed, including clone/delegation contention, cancellation after durable insert, revocation after durable insert, persistence failure, conflicting replay, shared-pool race, restart reconciliation, reduced limits, and Survival Reserve separation.
- `cargo test --workspace --locked --offline`: 95 passed (1 CLI, 6 contracts, 73 kernel, 15 state); doc tests passed (none defined).
- `cargo fmt --all -- --check`, workspace `cargo check`, and workspace Clippy with `-D warnings`: passed.
- Release CLI and idle-memory helper builds: passed.
- `cargo deny check`: passed; duplicate `cpufeatures`, `hashbrown`, and `syn` versions remain warnings.
- `cargo audit --deny warnings`: passed; 1,267 advisories loaded and 189 locked dependencies scanned.
- Fresh release `doctor`: `native_ready`, database integrity `ok`, schema v2, HIVE `not_configured`, semantic embeddings `disabled`.
- Five-sample release benchmark: durable usage accounting median `7,607,090 ns/op`; resource lease/delegation median `476 ns/op`. Exact cache identity had 10,000 hits and zero misses; token and monetary savings remain `not_measured`.
- Fresh idle-memory helper: all five working-set samples `6,868,992` bytes (`6.55 MiB`).
- Local Windows firewall setup remains blocked by `Access denied`; local doctor is not outbound-isolation evidence.

## Documentation and evidence changes

Updated S08, S12 and S20 module contracts, runtime/recovery documentation, performance snapshot, CEC, Evidence Bundle and proposed checkpoint delta. `.engineering/CHECKPOINT.md` and `.engineering/CHECKPOINT.json` remain unchanged. The complete tracked-source SHA-256 inventory is maintained in [SOURCE-FINGERPRINTS.sha256](SOURCE-FINGERPRINTS.sha256).

## Assurance boundary and disposition

UADS plan/run `wo_b161d2f667649b5c` / `er_586620b1f8dd4753` records a pre-CD3 digest, `1e7bb0a5d6c779590680c790cf8801c59c5827e7f1af1cf224bd5f9ad6f77889`; it does not certify CD3 or CD4. After the CD3 commit, `uads verify --json` returned `no implementation change to verify` on the clean worktree and the run was marked `blocked`, so fresh digest-bound CD3/CD4 execution evidence is not established. PR #9 records the exact-head candidate CI and is the authoritative location for the check binding of any later evidence-only commit. Independent security/performance review and four distinct assurance sessions are required; no visible executor result substitutes for those reviewer verdicts. If UADS cannot prove distinct reviewer sessions, stop as `BLOCKED — INDEPENDENT_REVIEW_BACKEND_REQUIRED`.

No merge, checkpoint promotion, M01 work, or UADS repository commit/push is authorized by this delta.
