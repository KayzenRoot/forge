# FGE-004-M00 C03 Correction Matrix

Historical C03 matrix snapshot: candidate `fda8969097320cdad9b8f92dffb4fddd8fc2424e` (tree `f6c4c21a7f5e910c85c6bf5844ddb3b294679e13`). The current implementation candidate is `0ce565eb217ea3dd1d38e31bf16bdecf772c4175` (tree `27e4edd886a8cadf802db1359bef45fe265c920c); exact-head Actions run [36278984106](https://github.com/KayzenRoot/forge/actions/runs/36278984106) passed all four configured jobs. A ten-pair performance rerun on that implementation passed all 16 comparable workloads and three scaling checks; the earlier ten-pair failure is preserved and explained in the current overlay below.

Status: historical correction matrix with a current C03 correction overlay below. The older rows
record the findings and fixes at the `fda8969` snapshot; the overlay is authoritative for later
corrections and current binding. This matrix does not claim audit approval, merge, checkpoint
promotion, or M01 admission.

## High findings

| Finding | Previous behavior | Correction in this candidate | Regression evidence |
|---|---|---|---|
| H1 — Host authorization could be caller-asserted | A caller supplied principal and permissions directly to a public context constructor. | The command bus accepts a host-keyed decision bound to decision ID, subject, action, scope, resource, run, permissions, and bounded validity. The durable store consumes/revokes decision IDs; tampering, expiry, replay, revocation, and scope mismatch fail closed. | `commands::tests::host_authorization_rejects_forgery_expiry_replay_revocation_and_scope_mismatch`; persistent consume/revoke paths in S08. |
| H2 — ProofGraph accepted unauthenticated `Passed` nodes and rejected this repository's SHA-1 commits | Public node claims and invented session IDs could complete certification; Git IDs were assumed 64-hex. | `add` rejects `Passed`; `add_verified` requires a backend-signed receipt bound to artifact, object format, exact head, change set, dependencies, obligations, inputs, environment, backend run, and session IDs. Review sessions must differ. Git object IDs are validated against the actual SHA-1/SHA-256 repository format. | `proof::tests::backend_receipt_rejects_stale_or_mutated_proof_bindings`; `proof::tests::independent_review_proof_requires_distinct_sessions`; `proof::tests::git_assessment_derives_exact_paths_and_checks_sha1_and_sha256_references`. |
| H3 — Canonical state, outbox, receipt, and recovery ownership were not atomic | A crash could persist CAS state without its durable event or receipt; opening another store could mark a live intent unknown. | One SQLite transaction commits CAS, all durable outbox rows, and the owner-bound command receipt. Events fan out only after commit. Instance heartbeats and leases distinguish live owners from stale intents. | `store::tests::command_state_outbox_and_receipt_commit_atomically`; `store::tests::command_commit_process_crashes_recover_atomically_at_each_write_boundary`; `store::tests::opening_a_second_store_preserves_live_intents_and_recovers_stale_owners`; concurrent-owner test. |
| H4 — Obligations and Change Cone could omit changed paths | Callers supplied risk/obligations and missing seeds could be discarded without broadening impact. | Assessment derives changed paths from the exact Git base-to-current-HEAD diff; caller cannot construct a partial assessment. Missing/empty seeds, absent or unverified graph coverage, missing source fingerprints, and low-confidence edges broaden to conservative obligations. Risk and obligations are compiler-derived. | `proof::tests::omitted_git_change_range_broadens_to_the_full_proof_universe`; `causality::tests::missing_or_empty_seeds_fail_closed_to_the_full_graph`; high-risk and missing-source obligation tests. |
| H5 — CI did not execute all required offline cases | Hosted doctor coverage omitted the complete six-ID ZDRP suite. | Both Linux and Windows jobs activate and verify outbound-block rules, check TCP egress probes for IPv4 and IPv6, then run the six named IDs and native diagnostics. | `.engineering/scripts/test_zdrp_scenarios.py`; `repository-validation.yml` Linux/Windows network-blocked steps. Local Windows run passed the six readiness groups; local egress is explicitly not asserted. Hosted proof passed on historical `fda8969` run `36270900434`; current implementation proof is bound by run `36278984106` and the current overlay below. |

## Medium and Low finding groups

| Finding | Previous behavior | Correction in this candidate | Regression evidence |
|---|---|---|---|
| ML1 — Secret values could persist in ordinary payloads | Evidence payloads and other state/cache/replay paths serialized arbitrary JSON. | A bounded recursive filter rejects credential-shaped values and secret-bearing field suffixes before state, CAS, receipt, outbox, evidence, usage, or cache persistence. Errors do not echo values; emergency records accept stable codes only. | `store::tests::credential_canary_never_reaches_canonical_state_receipts_outbox_cache_or_replay`; runtime emergency-ring canary test. |
| ML2 — Source verifier had unbounded repository inputs | Git stdout, tree entries, manifests, blobs, path depth, and aggregate object batches could consume unbounded resources. | Git subprocess time/output, tree/manifest/path/blob/batch sizes, traversal, symlinks (including self-referential loops), unsupported object formats, and ambiguous case aliases are bounded or rejected. | `test_source_fingerprints.py`: valid/deterministic cases plus path, tree, blob, symlink and self-referential symlink-loop, fabricated-commit, malformed-manifest, SHA-1/SHA-256, missing-object, and Git-failure fixtures. |
| ML3 — Capability evidence and readiness could be self-asserted | Public registry operations accepted `Proven` and `Ready`; a caller could also construct a forged public snapshot for the resolver. | Public registration is limited to `Declared`/`Unknown`, public promotion cannot claim verified levels, trusted runtime code alone records readiness/provenance, and snapshot fields cannot be constructed or mutated externally. | `capabilities::tests::callers_cannot_self_assert_provenance_or_ready_health`; `CapabilitySnapshot` compile-fail doctest; resolver freshness/abstention test. |
| ML4 — The 100,000-record ceiling was local to one governor | Multiple stores/processes could each admit a local allowance against one durable ledger. | Schema v3 stores shared per-pool totals and performs the total record-count and resource ceiling check in the same SQLite write transaction. | `store::tests::resource_ledger_limit_is_shared_across_store_instances`; concurrent durable-pool ceiling tests; v1-to-v3 migration and integrity check. |
| ML5 — History scans and removal made hot paths scale poorly | Each ledger write aggregated prior rows; Change Cone scanned all edges for each visited node; scheduler owner removal shifted a vector under lock. | Ledger writes update constant-size aggregates; Change Cone uses outgoing adjacency; scheduler rotates owner IDs with a deque and removes exhausted owners without scanning/shift-removing other owners. | Repeated ledger cardinality intervals and Change Cone/scheduler benchmarks at 100, 1,000, and 10,000 entries; ten-pair report records Change Cone ratio 2.296× (limit 3×), scheduler ratio 2.108× (limit 3×), and durable-ledger write ratio 1.079× (limit 2×), all PASS. |
| ML6 — Performance data had no executable budgets | A five-sample snapshot was descriptive only and did not set regression thresholds or show scale curves. | The performance runner compares repeated baseline/candidate outer medians (five inner samples each), freezes median/MAD budgets, verifies the baseline archive against the declared Git commit, and bounds graph/scheduler/ledger scaling ratios. The exact C03 candidate ran ten paired outer comparisons and passed all 16 comparable checks plus all three scaling curves. | `.engineering/scripts/check_m00_performance.py`; exact baseline `2f95efd4a05ade63dd3e44f3a202c0afe0a46bc4`; ten-pair candidate report, thresholds, initial five-pair variance record, and measured verdict are recorded in `docs/PERFORMANCE-M00-CANDIDATE.md` and the external evidence package. |
| ML7 — CEC and Evidence Bundle carried stale candidate/CI bindings | Current documents continued to present prior candidate SHA, prior workflow run, and older assurance state as the evidence binding. | Current correction identity, test matrix, source inventory, performance measurements, current-head CI pointer, and explicit assurance boundary are added while historical snapshots remain identified as historical. | Updated CEC, Evidence Bundle, CD3 evidence delta, proposed checkpoint delta, exact source-fingerprint result, PR #9 exact-head checks, and frozen assurance package digest. Candidate run `36270900434` is bound to `fda8969097320cdad9b8f92dffb4fddd8fc2424e`. A later documentation-only head requires its own exact-head CI result; four fresh reviewer-session identities and verdicts are recorded only after reviewing the frozen package digest. |

## Boundaries

- The local Windows egress check is not evidence that a firewall blocked network traffic. Only the
  exact-head hosted Linux and Windows jobs can establish that gate.
- Backend receipt signatures authenticate a trusted backend assertion. The backend must resolve
  its session IDs to real executions before signing; string shape alone is not independent-review
  evidence.
- Process-kill tests cover SQLite transaction recovery, not power loss or storage-device fsync
  behavior.
- This correction does not merge PR #9, modify canonical checkpoint files, promote M00, or begin
  M01. A failed hosted or independent-review gate remains a stop condition.

## C03 reviewer correction overlay (2026-09-26)

This overlay supersedes earlier C03 candidate, performance, and pending-hosted-proof statements.
The current implementation candidate is `0ce565eb217ea3dd1d38e31bf16bdecf772c4175`, tree
`27e4edd886a8cadf802db1359bef45fe265c920c`, based on `2f95efd4a05ade63dd3e44f3a202c0afe0a46bc4`.
Its exact-head Actions run [36278984106](https://github.com/KayzenRoot/forge/actions/runs/36278984106)
passed governance, security/supply-chain, Ubuntu native runtime, and Windows native runtime; both
hosted outbound-network-blocked doctor steps passed. This run applies to `0ce565e` only. A later
documentation-only PR head requires its own exact-head CI binding.

| Correction group | Current implementation/evidence | Status |
|---|---|---|
| H1-H4 and ML3 | Host-signed scoped decisions and durable single-use resolution, authenticated proof receipts bound to exact repository changes, atomic command/state/outbox/receipt persistence, conservative obligations, and non-self-asserted capability provenance remain in the implementation candidate. | Corrected; regression coverage is in the Rust suites and S21/CI evidence. |
| H5 | Linux and Windows hosted jobs execute all six C03 ZDRP cases while outbound egress is blocked and verified. | PASS on implementation run `36278984106`; local Windows does not claim egress isolation. |
| ML1 | Secret filtering also rejects `aws_secret_access_key` and credential-shaped idempotency error codes; durable published event IDs match their staged outbox IDs, and the unauthenticated public `publish_committed` path is crate-private. | Corrected; canary, event identity, and API-boundary tests pass. |
| ML2 | Source inventory contains 72 raw-Git-blob fingerprints and includes build/verification inputs, including `crates/forge-kernel/build.rs`; an isolated unlisted-build-input fixture fails closed. | Verifier PASS on `0ce565e`, 72 entries, zero mismatches. |
| H3/S08/S10/S12/S20 | Schema v4 records host-signed UOR decisions append-only and preserves them through backup/restore; only a signed `EffectNotCommitted` result enables a retry with fresh authorization. Resource leases use bounded monotonic TTL, inherited child expiry, expiry reclamation, and fail-closed stale handles. | Implemented and locally tested; provider billing integration remains unavailable and budgets stay fail-closed at zero. |
| ML6 | The readiness workload now serializes the same canonical digest without allocating a dynamic JSON value. Ten-pair reports retain the first `0ce565e` FAIL and the rerun PASS; all three scaling checks pass. | Runner gate PASS on rerun; measured lease overhead and variance are disclosed in `docs/PERFORMANCE-M00-CANDIDATE.md`. This is not an external SLO approval. |
| ML7 | The implementation head, source inventory, current Actions run, performance reports, and pending fresh review state are reconciled across the current evidence overlay. | Final documentation head and frozen package must be bound externally after exact-head CI. |

The fresh four-review gate remains pending: security, performance, reliability, and certification-systems
reviewers must inspect one frozen package digest. The package and reviewer results are not an approval
of M00. PR #9 remains open and draft; the checkpoint files remain unchanged; M00 is not promoted and
M01 has not started.

## C03 final correction executor overlay — implementation head f03da4a (2026-09-26)

This overlay supersedes the earlier implementation/performance state above. Earlier rows remain
historical for their named commits. The current implementation commit is
`f03da4aa02e619b5affcff09baf6fd56337a286e`; final evidence-head CI, source verification, package
digest and fresh reviewer identities are bound externally after packaging.

| Correction area | Final correction | Evidence on the corrected implementation | Boundary |
|---|---|---|---|
| Confirmed-effect recovery (H3/S08/S10/S12) | Handler-confirmed effects enter `effect_committed_pending_finalization`; an outcome-resolution claim cannot make them retryable. A narrowly scoped host-signed revision changes only the local transition under the exact staged fingerprint. Final state, outbox, receipt and marker commit atomically. | Schema-v7 migrations; signed-decision rejection/replay/expiry tests; crash termination at each finalization write boundary; full workspace suite. | Local process-kill tests do not prove power-loss or storage-device fsync behavior. |
| Outbox consistency (S09/H3) | Per-producer sequence allocation reserves the SQLite writer before reading the next sequence. Delivery/acknowledgement is tracked per consumer, and malformed-event quarantine is bounded to three failures while other replay entries continue. | Regression with 96 concurrent producer writes proves contiguous unique sequence values; ordering, per-consumer acknowledgement and poison-event tests pass. The exact candidate benchmark also completes durable outbox and command dispatch. | Does not claim cross-region delivery semantics. |
| Secret and backup integrity (ML1/S08) | Credential-shaped field/value filtering covers camelCase, separators, compound credential names and plural forms. Backup v2 includes CAS inventory; restore checks object content and an expected digest supplied through a separately trusted channel. | Credential canaries, CAS tamper/recalculated-adjacent-hash test, restore integrity tests and documented unkeyed-hash limit pass. | The expected fingerprint must remain protected outside the backup. |
| Performance acceptance (ML6) | Common workloads use a predeclared fixed 20% allowance over the paired baseline median; the observed baseline maximum cannot widen it. Workload definitions, iteration counts and sample counts determine comparability. | On `f03da4a`: 10 paired outer runs x 5 inner samples; 13/13 comparable workloads and 3/3 scaling checks passed. `change_cone_100_node_chain` and `resource_usage_durable` are explicitly `not_comparable`. Full JSON report SHA-256 is `4b068854c60575b0a975b8495c956005194070ea381a1be52a44070fbf523b7f`. | Candidate-only/change-semantic baselines are not approved SLOs. The historical `fda8969` observed-maximum rule is superseded. |
| Local native/S21 evidence (H5) | Release doctor and all six named S21 cases execute against schema 7. Local execution leaves egress blocking unasserted. | Doctor: `native_ready`, SQLite integrity `ok`, offline ready. Local six-case mapping passes. Logs are frozen externally. | Hosted Linux and Windows firewall jobs alone prove blocked egress and must pass on the final evidence head. |

The final complete local suite passed: 1 CLI, 6 contract, 85 kernel, 26 state, and one compile-fail
doctest. `cargo fmt`, workspace check, clippy, release build, `cargo deny check`, and
`cargo audit --deny warnings` passed. `cargo deny` emitted duplicate-version warnings for
`cpufeatures`, `hashbrown`, and `syn`; its advisories, bans, licenses and sources checks passed.

Intermediate performance attempts first found a trailing-comma parser issue and then SQLite writer
contention in outbox/finalization paths. Both corrections are included before the complete `f03da4a`
measurement. Only that complete report is a benchmark verdict. Fresh security, performance,
reliability and certification-systems review of one identical frozen package SHA remains mandatory.
PR #9 remains open/draft/unmerged; checkpoint files are unchanged; M00 is not promoted and M01 has
not started.

## C03 round-1 reviewer correction overlay — implementation commit e1da083 (2026-09-27)

This overlay maps every actionable round-1 finding from the immutable 5e4fd5d package to the current
implementation and regression evidence. The original review result remains historical; it is not
reused as approval for this candidate. The new code commit/tree are
`e1da083893d9335118519392c1cdb36021cbfc74` /
`c5a9d5950af40e43f978d47b89186803121f3615`.

| Finding | Correction | Evidence | Current boundary |
|---|---|---|---|
| R1-COMMIT-CONTENTION (P2) | `commit_command` reserves the SQLite writer before outbox identity/sequence reads. Primary and extended BUSY/LOCKED codes are retryable only on pure-command commit failure, using `FORGE.COMMAND.COMMIT_CONTENTION_RETRYABLE`; failures are not mislabeled as unknown external effects. | 96 concurrent eventful store commits and 48 concurrent pure eventful command commits; SQLite extended-code and retry-state regressions; full workspace suite. | Exact evidence-head hosted CI and a fresh reliability review remain pending. |
| C1-STALE-CONTEXT-LOCK (MEDIUM) | The stale embedded lock from the old package is not copied forward. The replacement package will contain a newly generated context lock matching its exact final head/tree, PR #9, base, and exact-head Actions run; the outer ZIP/manifest digest remains in the adjacent external lock to avoid recursive hashing. | Old package remains immutable; final package assembly and lock construction use newly fetched exact-head GitHub state. | Cannot freeze the final package until the evidence commit is pushed and its exact-head run completes. |
| P1-IDLE-MEMORY-REPEATABILITY (MEDIUM) | Repeated paired baseline/candidate Windows working-set sampling now uses ten outer pairs and five inner samples, with a predeclared baseline-median + 20% cap. | Final report: baseline median 7,667,712 B; candidate median 8,198,144 B; budget 9,201,255 B; PASS. | Local Windows process evidence only; not a portable memory SLO. |
| P2-CANDIDATE-ONLY-BUDGET-GATE (MEDIUM) | Candidate-only and changed-definition workloads first produce capture-only references and then must pass a final exact-identity repeated gate against those frozen references. Identity and sample-shape validation is fail-closed. | 19/19 final reference gates PASS; capture verdict is not counted; nine unit regressions reject stale, malformed, or mutated references. | Independent performance review remains pending. |

The exact-head source verifier passed 73/73 entries on implementation commit `e1da083`; the full
Rust/Python local validation is recorded with its raw logs in
`C03-CORRECTION/`. Final exact-head CI and fresh security, performance, reliability, and
certification-systems review of one identical immutable package digest are still required. This
overlay records correction evidence, not approval or M00 certification.
