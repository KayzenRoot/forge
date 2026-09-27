# M00 threat model

Status: candidate analysis; independent security assurance pending.

## Scope and trust boundaries

The M00 boundary includes the local CLI, caller-supplied JSON contracts/instances, local state root, proof/cache records, capability and health evidence, and adapter manifests. HIVE, remote models/providers, Core/Hades and IRIS are optional external systems and are not trusted authorities for native boot. A remote trace/context identifier is data only and never grants permission.

## Threats, controls and evidence

| Threat | Candidate control | Evidence in this change |
|---|---|---|
| Malformed or oversized JSON/schema input | CLI byte limits, typed parse errors, bounded schema compilation, external schema reference rejection | Contract positive/negative tests; CLI boundary checks |
| Path traversal and symlink redirection | Restricted store identifiers, canonical state root checks, symlink component checks for content-addressed storage and backup artifacts | Store path and symlink regression tests |
| Malicious extension gaining authority | Registration does not grant permissions; caller/adapter permissions are intersected; origin scopes are explicit; validated adapters remain dormant | Extension permission, origin and dormant-state tests |
| Secret exposure through cache, event or replay | Sensitive/secret cache entries are denied; event privacy is capped at Internal; sensitive command results are not replay-persisted | Cache, event and command negative tests |
| Cache/proof poisoning or stale reuse | Semantic identity binds scope, input, dependencies and environment; proof nodes bind exact head and dependencies; unknown impact expands obligations | Cache fingerprint tests; proof gap and exact-head tests |
| Forged capability or stale health evidence | Monotonic evidence, immutable snapshots, freshness windows, quarantine and stale-evidence abstention | Capability, health and resolver tests |
| State/evidence tampering | SQLite integrity checks, immutable evidence identity, BLAKE3 backup manifest verification and content-addressed hashes | State integrity and backup-tamper tests |
| Durable usage replay or cross-process budget race | Append-only usage IDs, payload conflict detection, schema v1→v2 migration, and one atomic SQLite insert that checks cumulative token/cost ceilings per pool | Migration, replay, concurrent-write, two-boot shared-budget and child-process rollback tests |
| Resource exhaustion | Bounded runtime settings, queue caps, metric cardinality, input sizes and hierarchical resource leases | Scheduler/resource/telemetry property and limit tests |
| Side-effect replay or unknown outcome | Explicit idempotency modes, commit evidence, unknown-outcome terminal state, state CAS, stable outbox event IDs | Command, event and state-transition tests |
| External trace spoofing | Trace and causal identifiers are not authorization inputs | Authorization derives from trusted host context in the command API |
| Untrusted model/provider output | No remote model/provider is used by native boot or proof verdicts; adapters require explicit contracts and stay dormant | Boot test and extension contract tests |

## Residual risks and proof gaps

- The M00 adapter layer is not an OS sandbox; adapters are not dynamically executed in this candidate.
- Local `cargo-deny 0.20.2` and `cargo-audit 0.22.2` checks passed on 2026-09-23; cargo-deny reports duplicate transitive versions of `cpufeatures`, `hashbrown` and `syn` as warnings. The exact pushed head still needs hosted checks and independent security review.
- The crash test kills a child during an uncommitted canonical write and verifies rollback; disk-full injection, crash scheduling across every transition, and property/fuzz coverage for every public parser remain open.
- Local Windows firewall rule creation was denied, so exact-head Windows/Linux network-isolation checks remain pending hosted CI. A normal local doctor run is not proof of blocked egress.
- Durable token/cost accounting has no provider receipt integration. Budgets default to zero until explicitly configured, and cache counters do not establish tokens or money saved.
- A caller that serializes secrets into arbitrary user-provided evidence metadata must be prevented by the host's evidence contract; this candidate does not infer secrecy from arbitrary JSON text.
- No security or certification PASS is claimed by this document.

## C03 correction overlay — SQLite contention and performance-reference integrity (2026-09-27)

This overlay describes the implementation correction at `e1da083893d9335118519392c1cdb36021cbfc74`
(tree `c5a9d5950af40e43f978d47b89186803121f3615)). It is candidate evidence; fresh package-bound
security assurance remains pending.

- Concurrent pure local command commits reserve SQLite's writer before outbox reads. Primary and
  extended `BUSY/LOCKED` results are surfaced as retryable contention only for pure commands, with a
  stable error code. The transaction rollback boundary keeps state, outbox, and receipt atomic; the
  regression suite covers concurrent eventful store and command commits.
- The performance gate now checks trust-sensitive reference fields before acceptance: candidate commit
  and tree, baseline commit, environment, reference kind, run count, workload definition, iteration and
  inner-sample shape, captured repeated measurements, recomputed median/MAD and fixed budget. Stale
  or mutated identities, truncated measurements, changed definitions, malformed values, and boolean
  schema versions are rejected by nine dedicated tests. Candidate-reference capture is explicitly
  `CANDIDATE_REFERENCE_CAPTURE_ONLY`; only a later repeated final gate can return PASS.
- The baseline archive was checked against its declared raw Git source before the run. The final local
  comparison passed 13/13 comparable workloads, 19/19 candidate/non-comparable reference gates, all
  three scaling checks, and the repeated Windows idle-memory budget. Measurements remain local candidate
  evidence and are not external SLO approval.
- Local S21 cases passed, but this Windows host did not enforce outbound blocking. The final exact-head
  hosted Linux and Windows gates are required for egress evidence.

No security PASS is inferred from the earlier 5e4fd5d review: that review is historical and the
corrected package needs fresh, independent, package-bound assurance.
