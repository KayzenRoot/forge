# FGE-004-M00 Evidence Bundle — implementation candidate

Status: CANDIDATE EVIDENCE; NOT AN APPROVAL OR CHECKPOINT PROMOTION

## Identity

- Work Order: `FGE-004-M00` (M00, S01-S22, HIGH risk).
- Admitted base / `origin/main`: `5b6c41da4a7a871d0538f17b164fb5e4ebd9b80c`.
- Branch: `fge-004-m00-implementation`.
- Final candidate commit: recorded in the exact-head PR description after the evidence commit is pushed. The PR description is the authoritative post-commit binding; this file cannot contain its own containing commit hash.
- UADS: v0.12.1; refreshed specialist catalog has 26 enabled profiles. `systems-runtime-specialist` covers rust, kernel-runtime, contracts, event-driven-systems, state-recovery and offline-first; `test-engineer` covers testing; `independent-reviewer` covers certification. The selected M00 plan is HIGH risk with security, performance, reliability and independent-review assurance roles. Its run stopped because the candidate paths were rejected as out-of-scope. Failure `fail_62bb1455fe330004` and diagnosis `diag_c2836157984e75b6` record the blocker at change digest `3de856412e0552852b807651f6573d43f61c19c7284bf48d3d6b94e026bbcfa6`, while the branch was still dirty at the admitted base. Evidence notes, `.gitattributes` and the final source-fingerprint file were edited after the run stopped, so this UADS digest does not attest the final candidate or commit.

## Candidate structure and decisions

- `forge-contracts`: contract IDs/versions, JSON Schema validation, typed errors and fingerprints.
- `forge-state`: SQLite canonical state, CAS, versioned schema initialization, idempotency, outbox, evidence, disposable cache and verified backup/restore.
- `forge-kernel`: single-authority S01-S22 modules with no CLI dependency.
- `forge-cli`: offline native doctor, file hash and contract validator.
- Boundary rationale: [ADR-FGE-004-M00-01](../../decisions/FGE-004-M00-RUST-WORKSPACE.md).
- Dependency checks require internal path dependencies to declare workspace versions. `deny.toml` blocks HTTP crates in the Forge graph and permits only listed licenses/sources. Private unpublished workspace crates are excluded from third-party license policy checks; no project license was guessed.
- HIVE remains optional. Native boot reports HIVE not configured and semantic embeddings disabled; no external LLM or embedding API is called.

## Exact source inventory

Canonical source SHA-256 values are in [SOURCE-FINGERPRINTS.sha256](SOURCE-FINGERPRINTS.sha256). Toolchain: rustc 1.98.1, commit `48a229ceaefd4985c50990b14116b6d856af0985`, target `x86_64-pc-windows-msvc`, LLVM 22.1.8. Hardware and candidate performance observations are in [PERFORMANCE-M00-CANDIDATE.md](../../../docs/PERFORMANCE-M00-CANDIDATE.md).

## Executed evidence

| Command/check | Candidate result |
|---|---|
| `cargo fmt --all` | PASS |
| `cargo check --workspace --locked --offline` | PASS |
| `cargo clippy --workspace --all-targets --locked --offline -- -A clippy::pedantic -D warnings` | PASS |
| `cargo test --workspace --locked --offline` | PASS: 76 unit tests across workspace; doc tests pass (0 defined) |
| `cargo check -p forge-kernel --benches --locked --offline` | PASS |
| `cargo bench -p forge-kernel --bench m00_baselines --locked --offline` | PASS on Windows; includes resource charge/attribution (0.949 us/op); measurements and method in performance report |
| `cargo-deny 0.20.2 check` | PASS: advisories, bans, licenses and sources; warns about duplicate `cpufeatures`, `hashbrown`, and `syn` versions |
| `cargo-audit 0.22.2 audit --deny warnings` | PASS against 1,264 loaded RustSec advisories and 189 locked dependencies on 2026-09-23 |
| `cargo run -p forge-cli --locked --offline -- doctor --data-dir <fresh-temp-root>` | PASS on Windows: `native_ready`, database integrity `ok`, schema v1, HIVE `not_configured`, embeddings `disabled` |
| GitHub Actions run `35816105638` at candidate evidence head `75775bde55945bb16d7f89bba2c703e029d48db0` | PASS: governance, Windows, Linux/network-blocked doctor, and security-supply-chain jobs; PR checks are authoritative for later heads and must be refreshed after each commit |
| `uads verify --json` | BLOCKED: plan `wo_782d35d1e669291c` rejects the Rust workspace, workflow, M00 docs/modules, ADR and evidence paths as out-of-scope; after stopping, UADS refuses another verify until a new plan/run is created |
| `uads status --json` | BLOCKED: phase `stopped`, status `blocked`, eight gates pending, four reviewers pending |
| `uads cache status --json` / `uads cost status --json` | Cache: 0 reusable records; budget status `ok`; token estimate is a byte heuristic and no gates were executed |
| `uads failure record` / `uads diagnose` | Recorded as `fail_62bb1455fe330004` / `diag_c2836157984e75b6`, status `needs-evidence`, bound to the pre-commit dirty change digest above |

The first cargo-deny run correctly failed on an undeclared third-party `MIT-0` license, missing licenses for private workspace crates, and unversioned path dependencies. The policy now explicitly permits `MIT-0`, ignores unpublished workspace members as documented by cargo-deny, and uses workspace-versioned path dependencies. The clean rerun passed with only duplicate-version warnings. Initial Rust compile/lint failures were fixed and rechecked.

## Security, resource and cost evidence

- `unsafe_code` is forbidden by workspace lint. Parser sizes, identifier shapes, queue sizes, handler count, metric cardinality and resource leases are bounded. Unit tests cover contract rejection, privacy denial, path shapes, backup tampering, stale health, event outbox identity, side-effect idempotency, extension scopes, queue limits and resource conservation.
- No known advisory was reported by the local cargo-audit snapshot; cargo-deny passed with the duplicate-version warnings above. This is not an independent security audit.
- Resource vectors include hierarchical CPU/memory/disk/network/token/cost limits; ordinary and survival leases use separate budgets, and ordinary work cannot borrow the configured reserve. Charges retain owner/category and cache-token-reuse counts, and cumulative token/cost use remains consumed after leases drop. The ledger is in-memory and is not wired into live model/runtime billing; measured cache token savings are not claimed. Remote model calls are not required or included.

## Platform status and gaps

- Windows local offline CLI smoke and full local test suite are candidate evidence.
- Linux certification is not claimed until hosted CI reports success for the final head.
- Actual DNS/outbound isolation is delegated to the new platform CI jobs; local smoke alone does not prove isolation.
- Idle memory, statistically repeated baselines, crash-kill injection at each transition, fault/disk-full injection, failure-neighborhood execution, full proof preservation, confidence/independence attestation and every I01-I22 obligation are not all proven. See the [Certification Evidence Capsule](CEC.md).
- `M00` is not complete, approved or promoted by this executor. Exact-head CI, independent assurance and the canonical checkpoint gate remain authoritative.
