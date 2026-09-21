# M00 — Consolidated Architecture & Module Certification Plan

Status: CONSOLIDATED PLANNING / NOT IMPLEMENTED
Module: M00 Forge Kernel & Contract Runtime
Planning sections: S01-S22
Parent increment: FGE-003

## 1. Mission
M00 establishes the sovereign kernel and contract runtime that every later Forge module depends on. It must be local-first, deterministic where declared, bounded under load, evidence-producing, contract-driven, recoverable and operational without HIVE, Core/Hades, IRIS, Internet, cloud or an LLM.

M00 is infrastructure, not a thin bootstrap. Its purpose is to make unsafe/ambiguous execution structurally difficult while preserving high performance.

## 2. Consolidated authority map

| Authority | Owner | Non-owner relationship |
|---|---|---|
| Kernel supervision/boot | S01 | consumes all kernel primitives |
| Module lifecycle | S02 | S17 projects health, S16 gates upgrades |
| Contracts/invariants | S03 | all sections consume FCF/CIR |
| Capability truth/registry | S04 | S05 resolves from immutable snapshots |
| Provider/path decision | S05 | cannot override S03/security/resource hard constraints |
| Dependency/causality/impact | S06 | S14/S16/S22 consume graph |
| Configuration truth | S07 | execution consumes immutable config capsules |
| Canonical operational state/evidence storage | S08 | telemetry/cache are not canonical |
| Facts/events | S09 | S10 commands may emit them post-commit |
| Intent/commands/effects | S10 | S12 protects idempotency/cancellation |
| Failure semantics | S11 | later self-healing consumes, does not redefine |
| Idempotency/cancellation/deadlines | S12 | S10/S13 use these primitives |
| Scheduling/concurrency/backpressure | S13 | S20 grants resources; S13 schedules within them |
| Cache/fingerprints/proof storage | S14 | S22 owns proof meaning/obligations |
| Extension boundary/isolation | S15 | integrations/providers implement FEP |
| Compatibility/upgrade safety | S16 | S02 lifecycle executes admitted transition |
| Health/readiness/degradation truth | S17 | S18 observes; S04 capability eligibility consumes |
| Telemetry/hot-path attribution | S18 | never canonical business state |
| Reproducibility/replay boundary | S19 | consumes fingerprints/contracts/evidence |
| Resource ownership/budgets/leases | S20 | S13 admission uses available leases |
| Sovereign boot certification | S21 | S22 treats ZDRP as mandatory proof |
| Proof semantics/obligations/certification | S22 | S14 stores reusable proof artifacts |

This table is normative when section wording overlaps.

## 3. Consolidated dependency spine
S03 Contract Fabric is the semantic spine.
S06 Causality Graph is the impact spine.
S08 State Fabric is the durable operational spine.
S11 Error Genome is the failure spine.
S14 Fingerprint Fabric is the reuse identity spine.
S18 Telemetry Spine is the observation spine.
S22 Proof Graph is the correctness/certification spine.

No other subsystem may create a parallel informal authority for these concerns.

## 4. Internal build waves
The one primary M00 construction Work Order/prompt may execute internally in waves without user re-prompt.

### Wave A — Trusted primitives
S03 contract primitives/CIR subset
S11 typed errors
S07 configuration primitives
S14 fingerprint primitives
S19 deterministic identity primitives

### Wave B — Sovereign state/runtime
S08 local state/recovery
S12 cancellation/idempotency/deadlines
S20 resource leases/budgets
S13 bounded runtime/scheduling
S01 kernel supervision/boot skeleton

### Wave C — Modules/capabilities
S02 lifecycle
S04 registry
S06 dependency graph
S16 compatibility
S05 resolver/decision deterministic levels

### Wave D — Messaging/extensions
S09 event bus
S10 command bus
S15 adapter/plugin SDK

### Wave E — Operational truth
S17 health/readiness
S18 telemetry/HPR
S21 zero-dependency path

### Wave F — Proof/certification
S22 proof graph/compiler/cache integration/certification
full M00 certification and evidence capsule

Waves are dependency-oriented implementation order, not separate user prompts or partial product releases.

## 5. Technology ownership consolidation
The following names are canonical:
- Forge Contract Fabric / Contract IR — S03.
- Forge Causality Graph — S06.
- Forge State Fabric — S08.
- Forge Error Genome — S11.
- Forge Semantic Fingerprint Fabric — S14.
- Forge Compatibility Engine — S16.
- Forge Telemetry Spine / Hot Path Registry — S18.
- Deterministic Execution Envelope — S19.
- Forge Resource Governor — S20.
- Sovereign Boot Capsule / ZDRP — S21.
- Forge Proof Graph / Proof Obligation Compiler — S22.

### Resolved overlaps
- Resource Lease Protocol is defined semantically/authoritatively by S20; S01 uses a minimal boot/runtime facade over it.
- Green Proof Cache storage/fingerprint mechanics belong to S14; proof validity/obligation semantics belong to S22.
- Capability health evidence originates through S17/provider probes; S04 stores/project eligibility state.
- Compatibility verdict belongs to S16; lifecycle activation/rollback belongs to S02.
- KFR is S01 emergency bounded recorder; FTS S18 is normal telemetry. KFR must work when FTS/exporters fail.
- Decision memoization uses S14 cache primitives; S05 owns decision semantics.
- State evidence artifacts live through S08/CAES; S18 telemetry is not substituted for governance evidence.

## 6. Cross-section invariants
I01 Native boot requires no external service/network/model.
I02 Cache deletion cannot break canonical correctness.
I03 Optional integration failure cannot globally fail native Forge.
I04 Hard contract/security/integrity constraints cannot be traded for cost/latency.
I05 All production queues are bounded unless a reviewed exception exists.
I06 Child resource/token/cost authority cannot exceed parent authority.
I07 Unknown/stale required evidence is never silently treated as healthy/compatible/proven.
I08 Commands express intent; events express facts.
I09 Unknown external effect outcome is never blindly retried.
I10 Replay never repeats irreversible side effects without new authorization.
I11 Extension registration does not imply permission.
I12 Telemetry is evidence/observation, not canonical state.
I13 Health/readiness is scoped, not one misleading boolean.
I14 SemVer alone is not compatibility proof.
I15 A passing test is not universal proof; evidence is obligation-scoped.
I16 Proof reuse requires dependency/environment fingerprints.
I17 High-assurance work escalates proof and authority requirements.
I18 Security randomness/controls are never weakened to manufacture determinism.
I19 Survival Reserve cannot be borrowed by ordinary work.
I20 LLM output cannot be sole authority for hard invariants or correctness certification.
I21 External trace/context identifiers never grant authorization.
I22 Exact-head certification is required before M00 completion.

## 7. Required implementation experiments before freezing candidates
The Work Order must benchmark/validate rather than blindly freeze:
- Tokio runtime topology vs conservative fixed baseline.
- SQLite WAL/synchronous/checkpoint/SQLx policy.
- BLAKE3 fingerprint throughput and identity versioning.
- OpenTelemetry-compatible instrumentation overhead.
- JSON Schema/CIR projection strategy.
- Protobuf/gRPC only where IPC evidence justifies them.
- cargo-nextest vs baseline test execution economics.
- Criterion benchmark stability.
- local System-1/Jev/Laya-compatible decision adapters only after kernel deterministic levels exist.

A candidate failing maintenance/licensing/security/performance/portability criteria is replaceable behind its contract.

## 8. Required M00 proof obligations

### Contract
CIR canonicalization/versioning, boundary validation, contract delta/compatibility fixtures.

### State/recovery
crash consistency, migration rehearsal, cache wipe, corrupted cache, corrupted canonical state, recovery journal.

### Runtime/concurrency
bounded queues, cancellation lineage, deadline propagation, backpressure, no orphan work, race/model tests.

### Resource
budget conservation, Survival Reserve, noisy-neighbor, token/cost exhaustion, GPU/VRAM admission where hardware capability exists.

### Effects
idempotency, unknown-outcome reconciliation, replay firewall, irreversible-effect safety.

### Extensions
permission isolation, adapter conformance, crash/hang containment, capability translation.

### Compatibility
directional semantic/state/security/toolchain compatibility and rollback gates.

### Health
scoped readiness, stale evidence, optional outage, overload, recovery hysteresis.

### Telemetry
cardinality, redaction, exporter outage, bounded overhead, integrity gap markers.

### Determinism
strict/semantic replay, clock/RNG/filesystem normalization, external fixture replay, drift detection.

### Sovereign
all S21 ZDRP scenarios under enforced network isolation on Windows and Linux.

### Proof system
POC/FPG/MSPS/PPE/GPC/PGD/CEC soundness including seeded regression corpus and cache poisoning/corruption cases.

## 9. Performance baseline families
No arbitrary final thresholds are frozen during planning. Implementation establishes reproducible baseline and then records budgets for:
- cold/warm offline boot;
- idle memory;
- contract validation;
- capability registration/resolution;
- command/event dispatch;
- state transaction/recovery;
- fingerprint/cache lookup;
- cancellation propagation;
- graph impact computation;
- readiness query;
- telemetry overhead;
- proof selection/reuse;
- controlled shutdown.

Hardware/workload/toolchain profile is part of every baseline.

## 10. Security/threat boundaries
Mandatory threat analysis covers:
- untrusted repo/input/schema;
- malicious/corrupt plugin/adapter;
- path traversal/symlink/project boundary;
- secret leakage through logs/cache/replay/evidence;
- dependency/supply-chain artifact;
- denial of service/resource exhaustion;
- forged capability/health/proof evidence;
- cache poisoning;
- replay of side effects;
- external trace/context spoofing;
- state/evidence tampering;
- model/provider prompt/tool output as untrusted data.

M08 later expands security; M00 cannot defer kernel-critical trust boundaries.

## 11. M00 Definition of Done
M00 is COMPLETE only when:
1. one admitted complete-module Work Order exists;
2. implementation matches the consolidated authority map;
3. public contracts are versioned/documented;
4. standalone native boot/recovery path works;
5. mandatory proof obligations have executable evidence;
6. no mandatory PGD proof gaps remain;
7. required security/threat review passes;
8. performance baselines/budgets are recorded;
9. health/telemetry/evidence are operational;
10. Windows and Linux sovereign certification passes;
11. integrations fail/degrade safely;
12. documentation/ADRs/migrations/rollback are current;
13. exact PR head has green required checks;
14. Certification Evidence Capsule is complete;
15. reviewer audit returns APPROVED;
16. checkpoint is promoted only after governed merge.

Partial waves are not called M00 complete.

## 12. Work Order admission gate
Product implementation remains blocked until FGE-003 planning is reviewed/promoted and the complete M00 Work Order is admitted from this consolidated architecture.

The primary M00 executor prompt must cover the entire module and all build waves. Additional M00 prompts are correction deltas only.

## 13. Stop condition
STOP after consolidation/review/admission artifacts are complete. Do not implement M00 product code during FGE-003 planning.
