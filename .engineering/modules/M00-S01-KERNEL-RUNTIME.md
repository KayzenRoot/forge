# M00-S01 — Kernel Runtime

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime

## Mission
Provide the smallest deterministic, high-performance and standalone-capable runtime on which every Forge module executes. Kernel boot MUST NOT require HIVE, Core/Hades, IRIS, Internet or an LLM.

## Design laws
1. Minimal trusted computing base.
2. Fail-fast for invalid local invariants; degrade explicitly for optional ecosystem capabilities.
3. No hidden global mutable state.
4. Bounded resources and concurrency by default.
5. Deterministic startup ordering.
6. Cancellation, deadlines and backpressure are first-class.
7. External systems are adapters/capabilities, never boot prerequisites.
8. Hot paths are measurable and budgeted.
9. Blocking work never silently occupies async workers.
10. Every lifecycle transition is observable and evidence-capable.

## Proposed Rust workspace boundary
- forge-kernel: lifecycle/runtime facade and invariant enforcement.
- forge-contracts: stable types, IDs, schemas and compatibility primitives.
- forge-capabilities: capability descriptors/resolution primitives.
- forge-config: layered configuration and validated snapshots.
- forge-runtime: task supervision, deadlines, cancellation and resource admission.
- forge-events: typed internal event/command primitives.
- forge-state: standalone durable/local state abstraction.
- forge-telemetry: tracing/metrics/health primitives.
- forge-adapter-sdk: stable adapter boundary, kept outside trusted kernel internals.
- forge-cli: thin operator/developer entrypoint.

Dependency direction is inward toward contracts/kernel primitives. Circular crate dependencies are forbidden.

## Boot pipeline
BOOTSTRAP -> PLATFORM PROBE -> CONFIG SNAPSHOT -> STATE OPEN/RECOVERY -> RESOURCE BUDGET -> NATIVE CAPABILITIES -> OPTIONAL ADAPTER DISCOVERY -> CAPABILITY SNAPSHOT -> HEALTH/READINESS -> READY.

Optional adapters initialize concurrently only after native invariants are valid. A failed optional adapter produces DEGRADED capability state rather than kernel death.

## Runtime model
Candidate: Tokio multi-thread runtime with explicitly bounded worker/blocking pools. Runtime topology is selected from measured hardware and policy, not raw core count alone.
- Structured task supervision: no detached orphan work.
- Hierarchical cancellation tokens.
- Deadlines propagated through commands/adapters.
- Bounded channels only on production paths unless a documented exception exists.
- Backpressure before queue growth.
- Dedicated blocking/CPU execution lanes.
- Panic boundary at module/adapter edge; kernel invariants remain fail-fast.
- Graceful shutdown drains bounded critical work and emits terminal evidence.

## Proprietary technologies

### Forge Pulse Kernel (FPK)
A thin supervisory layer over the runtime. Tracks task ownership, deadlines, cancellation lineage, resource lease and lifecycle state. Goal: make orphan work and uncontrolled concurrency structurally difficult.

### Adaptive Runtime Topology (ART)
Builds a runtime topology from hardware class, workload profile and configured budgets. It may alter worker counts, blocking lanes and queue budgets only within safe deterministic policy bounds. Benchmark against fixed Tokio defaults.

### Resource Lease Protocol (RLP)
Work requests CPU/memory/I/O/concurrency leases before expensive execution. Admission can execute, queue, degrade or reject. This becomes the primitive used later by Resource Governor.

### Capability Boot Snapshot (CBS)
Immutable fingerprinted snapshot of capabilities/providers available at READY. Later changes create a new snapshot/version rather than mutating invisible global state.

### Kernel Flight Recorder (KFR)
Bounded low-overhead ring buffer for critical lifecycle/error/resource events. Designed to preserve the last useful diagnostic window even when full telemetry is unavailable.

### Fast Restart Capsule (FRC)
Persist only safe, fingerprinted startup facts that can shorten subsequent boots. Every fact has an invalidation key; uncertain facts are recomputed.

## Performance policy
Performance is a contract, not an adjective. M00 implementation must establish reproducible baselines for:
- cold standalone boot;
- warm standalone boot;
- idle memory;
- capability registration/resolution;
- command dispatch;
- event dispatch;
- cancellation propagation;
- controlled shutdown;
- telemetry-disabled and telemetry-enabled overhead.

Numeric budgets are frozen only after baseline measurements on reference hardware/CI. CI detects statistically meaningful regressions rather than relying on one noisy timing sample.

## Memory/allocation policy
Prefer owned immutable snapshots and borrowing where it materially reduces copies. Avoid unsafe code by default. Any unsafe block requires a documented invariant, focused tests and benchmark evidence showing why safe code is insufficient. Arena/slab/small-vector/string interning techniques are candidates only after profiling proves allocation pressure.

## Determinism
Stable IDs and canonical serialization for fingerprinted structures. Startup phases have explicit order. Parallel discovery results are normalized before hashing. Wall-clock time, randomness, machine paths and secrets must not contaminate deterministic fingerprints unless intentionally part of the contract.

## Failure model
Kernel errors carry stable code, class, retryability, scope, causal chain and safe diagnostic metadata. Optional integration failure cannot masquerade as READY for that capability. Fatal local invariant failures stop readiness.

## Standalone acceptance
With HIVE/Core/IRIS endpoints absent and network denied, Forge must boot, expose native capability inventory, open local state, execute a deterministic local command, produce health/diagnostics and shut down cleanly.

## Security
No dynamic arbitrary code loading inside the trusted kernel. Adapter/plugin execution model is capability- and permission-bounded. Secrets never enter fingerprints/logs. External inputs are validated at contract boundaries.

## Test strategy
During implementation use impact-selected tests and Green Proof reuse. S01 certification includes unit, property, concurrency/race-oriented, cancellation, fault-injection, deterministic replay, standalone outage, compatibility and benchmark suites. Full M00 certification remains a later module gate.

## Jev/Laya/FDP boundary
System-1/LLM decision providers do NOT participate in kernel boot correctness. S01 exposes deterministic extension points that S05 may use for Forge Decision Plane. Kernel policy/security invariants are never delegated to probabilistic providers without deterministic guardrails.

## Open decisions for later M00 sections
- Exact SQLite/SQLx state schema belongs to S08.
- Event semantics belong to S09.
- Command semantics belong to S10.
- Full error taxonomy belongs to S11.
- Cache/CAS algorithms belong to S14.
- Adapter ABI/process isolation belongs to S15.
- Health contract belongs to S17.
- Telemetry exporters belong to S18.
- Full Resource Governor belongs to S20.

## Acceptance criteria
S01 planning is accepted when boot lifecycle, runtime ownership, standalone guarantees, performance measurement policy, resource primitives, deterministic boundaries and downstream ownership are unambiguous and do not prematurely implement later sections.
