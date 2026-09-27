# M00-S18 — Telemetry & Hot Path Registry

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S17

## Mission
Make Forge observable from kernel boot through capability execution while keeping telemetry bounded, privacy-safe and cheap enough that instrumentation does not become a hot path itself.

## Core laws
1. Telemetry is evidence, not canonical business state.
2. Every important execution can be correlated without parsing log strings.
3. High-cardinality data belongs in traces/logs/evidence, not metric labels.
4. Telemetry failure cannot break core execution except where audit evidence is explicitly mandatory.
5. Sensitive/secret data is classified before export.
6. Sampling never silently removes mandatory security/audit evidence.
7. Optimization decisions require measured baselines.

## Signal families
- structured logs;
- metrics;
- distributed/local traces;
- profiles;
- health/readiness transitions;
- command/event/error/evidence correlations;
- resource/queue/scheduler observations;
- cache/proof reuse;
- token/model/provider/cost accounting;
- build/test/deploy timing hooks for later modules.

## Telemetry envelope
Common correlation dimensions:
- trace/span identity;
- execution/work-order/project/module;
- command/event/error IDs;
- capability/provider;
- contract/version;
- config/state/graph/capability snapshot refs;
- resource lane;
- risk/side-effect class;
- safe environment/toolchain identity;
- evidence refs.
Only low-cardinality stable subsets become metric labels.

## Proprietary technologies

### Forge Telemetry Spine (FTS)
Unified typed instrumentation substrate projecting to OpenTelemetry-compatible traces/metrics/logs while keeping Forge semantic events/contracts independent of any telemetry vendor.

### Hot Path Registry (HPR)
Evidence-backed registry of operations/paths that dominate latency, CPU, memory, I/O, GPU, network, tokens or monetary cost for a workload/hardware class.

### Resource Attribution Graph (RAG-F)
Links resource consumption to Work Order -> command -> capability -> provider/module -> source/build/test operation. Distinct from retrieval RAG.

### Telemetry Budget Governor (TBG)
Controls collection, sampling, buffering and export budgets per signal/risk class. Under pressure it reduces optional telemetry before consuming recovery headroom.

### Adaptive Evidence Sampling (AES)
Sampling based on novelty, severity, latency tail, error fingerprint, risk and debugging state. Known healthy high-volume paths can sample aggressively; novel/slow/failing/high-assurance paths retain richer evidence.

### Optimization Opportunity Ledger (OOL)
Records measured hotspot, baseline, hypothesis, proposed optimization, expected metric, experiment result and accepted/rejected outcome. Prevents folklore optimization.

### Token & Cost Attribution Ledger (TCAL)
Attributes model/provider tokens, cache reuse, avoided tokens, calls, latency and monetary cost to project/work-order/capability/decision class.

### Telemetry Integrity Marker (TIM)
Marks telemetry gaps/drops/export failures so absence of data is never silently interpreted as healthy zero activity.

## Hot Path dimensions
HPR may classify:
LATENCY
CPU
MEMORY
ALLOCATIONS
DISK_IO
NETWORK_IO
GPU/VRAM
LOCK/CONTENTION
QUEUE_WAIT
SERIALIZATION
HASHING
CACHE_MISS
LLM_TOKENS
LLM_LATENCY
MONETARY_COST
ERROR/RETRY_AMPLIFICATION

A path can be hot for one hardware/workload profile and cold for another.

## Hot Path lifecycle
DISCOVER
-> BASELINE
-> ATTRIBUTE
-> HYPOTHESIS
-> BENCHMARK/PROFILE
-> OPTIMIZE
-> VERIFY
-> RECORD PROOF
-> WATCH REGRESSION

No optimization is accepted solely because code "looks faster."

## OpenTelemetry
OpenTelemetry is the preferred interoperability candidate for traces/metrics/logs/exporters. Forge semantic contracts remain internal source truth so changing exporter/backend does not rewrite kernel semantics.

The M00 local registry exposes an optional exporter boundary over a snapshot. Export failure is explicit and leaves locally collected metrics intact; no remote exporter is configured by native boot.

## Structured logging
Logs use stable event/error/action codes and structured fields. Human text is supplementary. Raw source code, prompts, secrets and payloads are not logged by default.

## Metrics/cardinality
Metric labels use bounded dimensions such as module, capability class, lane, outcome, error class. IDs, paths, raw URLs, commit hashes and arbitrary user strings are not metric labels.

## Tracing
Spans follow command/execution boundaries and can cross FEP adapters with sanitized context propagation. Trace context does not grant authority. Sampling state is independent from permission state.

## Profiling
Continuous profiling is optional/budgeted. On-demand profiles require permission and duration limits. Profiles are treated as potentially sensitive artifacts and may include symbol/path metadata.

## Token/model telemetry
For AI/model operations:
- input/output/cached/avoided tokens where provider data permits;
- context capsule/prompt fingerprint;
- model/provider/decision level;
- retries/fallbacks;
- latency;
- estimated/actual cost;
- quality/eval result when later available.
No prompt body is required for normal cost attribution.

## Cache/test proof telemetry
S14 exposes hit/miss/invalidation/recompute avoided. Green Proof reuse records tests preserved vs rerun and validation time saved. Savings are reported alongside later correction/regression cost to avoid fake efficiency.

## Backpressure
Telemetry uses bounded buffers/queues. TBG can sample/drop optional data with counters/TIM. Critical audit/evidence paths use dedicated durable handling when required, not the normal telemetry queue.

## Privacy/security
Field classification: PUBLIC/SAFE_INTERNAL/SENSITIVE/SECRET. Exporters receive only fields allowed by destination policy. Secret values never enter telemetry. Sensitive identifiers may use scoped pseudonymous fingerprints.

## Local-first/standalone
Forge provides local telemetry snapshots/files/ring buffers without requiring a remote collector. External collectors/backends are adapters. Internet outage does not affect core readiness.

## Retention
Different signal classes have explicit retention/size policies. High-volume traces/logs rotate. Evidence required by governance is promoted to S08 evidence storage rather than relying on telemetry retention.

## Performance targets
Instrumentation overhead budgets are defined per path class and benchmarked. Fast paths support disabled/low-cost branches. Expensive formatting/serialization is deferred until signal is retained/exported where possible.

## Integration
S01 KFR supplies boot/runtime events.
S05 records decisions/escalations.
S06 provides causality.
S08 evidence/state refs.
S09 event metrics.
S10 command traces.
S11 error fingerprints.
S13 queues/resources.
S14 cache/proof savings.
S17 health.
M19 later consumes HPR/OOL for deeper optimization.

## Test strategy
- trace/correlation propagation;
- metric cardinality guards;
- secret/sensitive redaction;
- sampling policy and mandatory evidence preservation;
- telemetry buffer overload/drop markers;
- exporter outage;
- standalone local telemetry;
- HPR attribution accuracy;
- TCAL token/cost accounting;
- cache/test savings accounting;
- profile permission/retention;
- instrumentation overhead benchmarks;
- malformed external trace context;
- Windows/Linux path/symbol sanitization.

## Anti-pattern gates
Forbidden:
- raw IDs/paths/messages as metric labels;
- synchronous remote export on hot path;
- telemetry queue without bounds;
- logging secrets/prompts/source indiscriminately;
- treating missing telemetry as zero/healthy;
- optimizing from intuition without baseline;
- remote observability service as boot dependency;
- sampling away mandatory audit evidence;
- claiming token/time savings without attribution.

## Acceptance criteria
S18 is accepted when Forge can correlate and measure kernel/module/capability execution, identify evidence-backed hot paths and attribute resource/token/cost usage with bounded overhead, safe cardinality/privacy and complete standalone operation even when every external telemetry backend is unavailable.
