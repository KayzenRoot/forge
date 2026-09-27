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
| ML2 — Source verifier had unbounded repository inputs | Git stdout, tree entries, manifests, blobs, path depth, and aggregate object batches could consume unbounded resources. | Git subprocess time/output, tree/manifest/path/blob/batch sizes, traversal, symlinks, unsupported object formats, and ambiguous case aliases are bounded or rejected. | `test_source_fingerprints.py`: valid/deterministic cases plus path, tree, blob, symlink, fabricated-commit, malformed-manifest, SHA-1/SHA-256, and Git-failure fixtures. |
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
