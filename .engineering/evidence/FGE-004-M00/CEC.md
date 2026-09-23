# FGE-004-M00 Certification Evidence Capsule — candidate

Status: NOT CERTIFIED — mandatory gaps and hosted/independent gates remain open.

## S01-S22 candidate coverage

| Section | Candidate authority | Local evidence | Status / open proof |
|---|---|---|---|
| S01 Kernel Runtime | `forge-kernel::boot`, `runtime` | runtime bound, native boot and cancellation tests | Candidate; inspect current-head hosted platform jobs in PR checks after each commit |
| S02 Module Lifecycle | `lifecycle` | lifecycle transition/readiness tests | Candidate; full rollback rehearsal open |
| S03 Contract Fabric | `forge-contracts::contract` | positive/negative schema tests and CLI validator | Candidate; broad contract property corpus open |
| S04 Capability Registry | `capabilities` | immutable snapshots, evidence monotonicity, quarantine tests | Candidate; forged-evidence host provenance needs review |
| S05 Resolver / Decision Plane | `resolver` | privacy, locality, freshness and deterministic-abstention tests | Candidate; multi-provider scaling baseline open |
| S06 Causality Graph | `causality` | change-cone and cycle regression tests; 100-node benchmark | Candidate; large graph scalability open |
| S07 Configuration | `config` | precedence, schema, secret-reference and approval tests | Candidate; filesystem/environment fault suite open |
| S08 State / Recovery | `forge-state` | transactional state, CAS, backup/restore, tamper tests | Candidate; process-kill and partial-migration rehearsal open |
| S09 Event Bus | `events` | bounded ephemeral lane, durable outbox and replay tests | Candidate; crash-at-ack fault suite open |
| S10 Command Bus | `commands` | authorization, idempotency, state guard, event and unknown-outcome tests | Candidate; external-effect reconciliation backend out of scope |
| S11 Error Taxonomy | `forge-contracts::error` | stable error fingerprint tests | Candidate; failure-neighborhood integration open |
| S12 Idempotency / Cancellation | `commands`, `runtime`, `forge-state` | retryability, unknown outcome, cancellation/deadline tests | Candidate; race/model schedule corpus open |
| S13 Concurrency / Backpressure | `scheduler`, event lanes | capacity, owner share, bounded queue tests | Candidate; saturation curves and platform stress open |
| S14 Cache / Fingerprints | `cache`, `fingerprint`, state cache | identity, TTL, corruption/privacy tests; cache benchmark | Candidate; durable savings ledger/open performance horizon |
| S15 Plugin / Adapter SDK | `extensions` | scopes, permission intersection, revocation and dormant-state tests | Candidate; no OS sandbox or dynamic execution; adapters remain dormant |
| S16 Compatibility Engine | `compatibility` | 14-dimension directional evidence matrix tests | Candidate; fixture/version corpus expansion open |
| S17 Health / Readiness | `health` | freshness, failure streak, required/optional degradation tests | Candidate; external probe outage integration open |
| S18 Telemetry / HPR | `telemetry` | bounded names/cardinality and aggregate tests | Candidate; exporter-failure and statistically bounded overhead open |
| S19 Deterministic Envelope | `determinism` | fingerprint/replay boundary tests | Candidate; full clock/RNG/filesystem replay normalization open |
| S20 Resource Governor | `resources`, resolver | hierarchical ceilings, non-borrowable reserve, token/cost charges and owner/category attribution tests | Candidate; durable ledger, live runtime wiring and measured cache savings remain open |
| S21 Zero-Dependency Boot | `boot`, `forge-cli doctor` | Windows offline build and native smoke; Linux and Windows blocked-network jobs are required for each current PR head | Candidate; hosted results are linked from the PR |
| S22 Test-Proof Foundation | `proof` | exact-head proof graph, obligation compiler, high-risk expansion, gap detection, bounded exact MSPS selection | Candidate; full FPG/PPE/GPC/CEV/FNE/VEP/PFH and exact-head CEC execution are partial |

## Invariant evidence map

Legend: `LOCAL` means named executable local evidence exists; `PARTIAL` means evidence exists but a required scenario/gate remains; `GAP` means the invariant is not adequately implemented/proven by this candidate. This table is not a PASS certificate.

| Invariant | Status | Evidence / gap |
|---|---|---|
| I01 Native boot has no network/model dependency | PARTIAL | Unit + Windows doctor smoke; hosted firewall/namespace proof pending |
| I02 Cache deletion cannot break correctness | LOCAL | State test separates canonical state from disposable cache |
| I03 Optional integration failure cannot globally fail Forge | PARTIAL | HIVE not configured boot works; configured-unreachable adapter fault test pending |
| I04 Hard contract/security/integrity constraints are not traded | LOCAL | Schema, privacy, state integrity and resolver negative tests |
| I05 Production queues are bounded | PARTIAL | Scheduler and event lane caps tested; all-path saturation proof pending |
| I06 Child resource/token/cost authority cannot exceed parent | LOCAL | Resource property and nested lease tests |
| I07 Unknown/stale evidence is not healthy/compatible/proven | LOCAL | Health/resolver/compatibility/proof exact-head negative tests |
| I08 Commands are intent; events are facts | LOCAL | Distinct command/event API and post-handler event test |
| I09 Unknown external effect is not blindly retried | LOCAL | Unknown-outcome command terminal-state test |
| I10 Replay does not repeat irreversible effects | PARTIAL | Idempotency/outbox safety tests; irreversible external reconciliation simulator absent |
| I11 Extension registration does not grant permission | LOCAL | Permission-intersection and dormant-adapter tests |
| I12 Telemetry is not canonical state | LOCAL | Separate telemetry registry and state ownership tests |
| I13 Health/readiness is scoped | LOCAL | Required/optional readiness report tests |
| I14 SemVer alone is not compatibility proof | LOCAL | Compatibility matrix tests reject metadata-only compatibility |
| I15 Proof evidence is obligation-scoped | LOCAL | S22 compiler, proof-to-obligation binding and gap tests |
| I16 Reuse binds dependency/environment fingerprints | LOCAL | Cache identity and exact-head proof dependency checks |
| I17 High assurance escalates proof/authority | PARTIAL | High-risk compiler requires security/fault/performance/replay/review; real independent assurance pending |
| I18 Security controls are not weakened for determinism | PARTIAL | Strict/semantic fingerprint split exists; comprehensive security randomness review pending |
| I19 Survival Reserve cannot be borrowed by ordinary work | LOCAL | Separate ordinary/survival pools and boundary tests prove ordinary leases cannot consume reserve |
| I20 LLM output is not sole hard-invariant authority | LOCAL | Native runtime does not call a model; deterministic code/tests decide local gates |
| I21 External trace/context IDs do not authorize | LOCAL | Auth context comes from trusted host permission decision; trace IDs are not auth inputs |
| I22 Exact-head certification precedes completion | PARTIAL | Compiler and graph bind exact head; final hosted CI/review/checkpoint not complete |

## Mandatory certification gaps

1. Re-establish the UADS path scope in a new plan and dispatch, then run its gates against the final candidate; the current run stopped and cannot certify this tree.
2. Persist token/cost attribution, wire the governor into the live runtime and measure cache-reuse savings if the Work Order cost criteria are to be marked complete.
3. Complete missing fault/concurrency/property/performance scenarios listed in the work order, including idle-memory and repeatability measurements.
4. Produce a trusted, genuinely independent assurance decision; the implementer cannot generate or attest to it.
5. Bind the final CEC and Evidence Bundle to the pushed final head, green current-head CI and a current UADS digest before any certification claim.

## Verdict

`CANDIDATE / NOT CERTIFIED`. Do not promote the checkpoint, merge the PR or start M01.
