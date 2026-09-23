# M00-S17 — Health, Readiness & Degradation Model

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S16

## Mission
Give Forge a truthful, typed and low-cost model of whether the kernel, modules and capabilities are alive, ready, healthy, degraded or unsafe, without conflating optional integration failures with core availability.

## Core laws
1. Liveness != readiness != health != capability availability.
2. A successful process/HTTP response is not proof of readiness.
3. Optional dependency loss degrades only the capabilities that actually depend on it.
4. Readiness is workload/capability aware, not one global boolean.
5. Unknown/stale health is not healthy.
6. Probes must not create harmful side effects or overload dependencies.
7. Health evidence has freshness and provenance.

## Health dimensions

### Liveness
Can the owned process/runtime make progress enough to participate in supervision?

### Readiness
May this component accept a defined class of new work safely now?

### Health
Is the component operating within expected correctness/resource/error bounds?

### Capability availability
Is a specific capability currently VERIFIED and eligible under S04/S05?

### Degradation
Can useful service continue with reduced capability/quality/performance while remaining correct?

## Component states
STARTING
READY
DEGRADED
NOT_READY
QUIESCING
STOPPING
FAILED
QUARANTINED
UNKNOWN

State transitions integrate S02 lifecycle and carry reason codes/evidence.

## Proprietary technologies

### Forge Vitality Matrix (FVM)
Multi-dimensional status model for kernel/module/capability/resource/integration health instead of a single red/green flag.

### Readiness Contract (RC)
Each module/capability declares what conditions are required for accepting each workload class. A module may be ready for READ_ONLY while not ready for MUTATING work.

### Capability Health Projection (CHP)
Projects low-level module/provider/resource health into S04 capability eligibility. One broken adapter only invalidates capabilities that depend on it.

### Degradation Topology Map (DTM)
Uses S06 graph to calculate the exact capability/workload blast radius of a failure and identify surviving fallback paths.

### Health Evidence Lease (HEL)
A health observation is valid only for a bounded freshness interval appropriate to its source. Expired evidence becomes STALE/UNKNOWN rather than silently healthy.

### Probe Cost Governor (PCG)
Budgets active health checks, applies jitter/coalescing and prevents a fleet of probes from becoming its own outage.

### Recovery Confidence Gate (RCG-H)
After failure, readiness is restored only after required recovery evidence/hold-down/stability conditions, preventing rapid green/red flapping.

### Degradation Contract Ledger (DCL)
Records declared fallback/degraded modes, quality reductions, disabled capabilities and user-visible implications. Degradation must be designed, not improvised.

## Readiness scopes
Readiness can be queried by:
- kernel;
- module;
- capability;
- command/work class;
- project/work-order;
- integration;
- resource class.
Global summary is derived from scoped truth, never the reverse.

## Probe types
PASSIVE — telemetry/error/resource observations.
INTERNAL — cheap invariant/local checks.
DEPENDENCY — bounded external/provider check.
SYNTHETIC — safe end-to-end probe with explicit budget.
DEEP — expensive diagnostic, not normal readiness path.

Prefer passive/internal evidence; active probes are rate/budget controlled.

## Dependency semantics
Dependencies are REQUIRED, OPTIONAL or CONDITIONAL for a given workload/capability.
- REQUIRED unavailable -> workload NOT_READY unless valid fallback exists.
- OPTIONAL unavailable -> affected enhancement DEGRADED/UNAVAILABLE, core path remains ready.
- CONDITIONAL evaluated against requested capability/workload.

## Sovereign standalone behavior
HIVE, Core/Hades, IRIS, remote providers and Internet are optional for universal native Forge operation. Their loss cannot fail kernel liveness/readiness unless a requested workload explicitly requires that capability and no native fallback exists.

## Resource health
S13 pressure feeds readiness:
- normal pressure -> READY;
- sustained high pressure -> DEGRADED/admission reduction;
- unsafe pressure -> NOT_READY for selected work;
- recovery headroom remains reserved.
A machine under overload can remain alive while rejecting new expensive work.

## State/integrity health
S08 canonical corruption, unrecoverable migration or integrity failure can make affected stateful capabilities NOT_READY/FATAL even when process liveness is fine. Cache corruption alone should normally quarantine/rebuild cache, not fail canonical readiness.

## Health aggregation
Aggregation is dependency-aware:
- no naive "worst child wins" globally;
- critical path determines workload readiness;
- optional failures are represented explicitly;
- multiple redundant providers can preserve capability readiness;
- stale required evidence yields UNKNOWN/NOT_READY according to risk policy.

## Flapping/hysteresis
State changes may use bounded consecutive evidence, minimum hold time and exponential probe backoff. CRITICAL/FATAL integrity/security signals bypass optimistic hysteresis where immediate isolation is safer.

## Health API/model
Machine response includes:
- scope/identity;
- state;
- readiness classes;
- unavailable/degraded capabilities;
- reason codes;
- evidence timestamps/freshness;
- dependency/fallback summary;
- resource pressure;
- recovery state;
- correlation/evidence refs.
Human summary is a projection, not the source of truth.

## Observability
S18 consumes structured health transitions, durations, probe latency/cost and degradation causes. Metrics use stable reason codes, not raw messages.

## Security
Health endpoints must not leak secrets, internal paths, tokens or sensitive topology to unauthorized callers. Deep diagnostics require stronger permission than coarse status. External probes are untrusted inputs.

## Performance
- immutable/atomic health snapshots for reads;
- event-driven updates where possible;
- coalesced dependency probes;
- cached HEL observations with freshness;
- graph-scoped recomputation on dependency changes;
- no synchronous remote probe on every readiness request;
- benchmark large capability/provider topology recomputation.

## HIVE/Core/IRIS
Adapters expose health through FEP/FCF and map provider-native states to Forge semantics. HIVE project READY is evidence for HIVE capabilities only, not global Forge readiness. Core/IRIS follow the same rule.

## Test strategy
- liveness vs readiness separation;
- optional integration outage;
- required dependency outage;
- fallback provider preservation;
- stale HEL evidence;
- capability-specific readiness;
- resource overload/degradation/recovery;
- state corruption vs cache corruption;
- probe storm prevention;
- flapping/hysteresis;
- recovery confidence gate;
- startup/quiesce/shutdown transitions;
- security/redaction of health output;
- large dependency graph aggregation benchmarks;
- standalone boot with all integrations disabled.

## Anti-pattern gates
Forbidden:
- one global boolean health flag;
- HTTP 200 = healthy semantics;
- synchronous remote dependency check on every probe;
- optional integration failure taking down native Forge;
- stale probe result treated as permanently healthy;
- probes with uncontrolled side effects;
- health endpoints leaking secrets/topology;
- instant READY after a transient recovery without required evidence.

## Acceptance criteria
S17 is accepted when Forge can truthfully answer whether each workload/capability is alive, ready, healthy or degraded using fresh dependency-aware evidence, preserve standalone operation during optional outages and reject unsafe new work without confusing process liveness with service correctness.
