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
| Resource exhaustion | Bounded runtime settings, queue caps, metric cardinality, input sizes and hierarchical resource leases | Scheduler/resource/telemetry property and limit tests |
| Side-effect replay or unknown outcome | Explicit idempotency modes, commit evidence, unknown-outcome terminal state, state CAS, stable outbox event IDs | Command, event and state-transition tests |
| External trace spoofing | Trace and causal identifiers are not authorization inputs | Authorization derives from trusted host context in the command API |
| Untrusted model/provider output | No remote model/provider is used by native boot or proof verdicts; adapters require explicit contracts and stay dormant | Boot test and extension contract tests |

## Residual risks and proof gaps

- The M00 adapter layer is not an OS sandbox; adapters are not dynamically executed in this candidate.
- Local `cargo-deny 0.20.2` and `cargo-audit 0.22.2` checks passed on 2026-09-23; cargo-deny reports duplicate transitive versions of `cpufeatures`, `hashbrown` and `syn` as warnings. The exact pushed head still needs hosted checks and independent security review.
- Process-kill/disk-full/network fault injection, property/fuzz coverage for every public parser, and exact-head Windows/Linux network-isolation checks remain open until their commands complete on CI.
- A caller that serializes secrets into arbitrary user-provided evidence metadata must be prevented by the host's evidence contract; this candidate does not infer secrecy from arbitrary JSON text.
- No security or certification PASS is claimed by this document.
