# M00 — Technology & Ecosystem Research Register

Status: PLANNING / NOT IMPLEMENTATION
Purpose: preserve M00 technology decisions and research inputs before the complete-module Work Order is admitted.

## Ecosystem source study
Forge planning MUST inspect the current canonical sources and public contracts of HIVE, Core/Hades and IRIS before freezing integration contracts.

### HIVE observed capabilities
HIVE v1.0.0 is local-first and provides project registry, task/prompt CAS, Git-aware incremental indexing, Python AST metadata, lexical/semantic/hybrid retrieval, checkpoint-first Context Manager, progressive disclosure, adaptive token budgets, context fingerprints/deltas/cache adapters, governed execution foundation, read-only MCP core, telemetry/event bus and local durable storage. Forge should consume these through adapters while preserving native fallback.

### Core/Hades observed direction
Core is Rust-based and organized as crates including CLI, config, contracts, health, identity, IPC, journal, registry, runtime and workspace. Forge should align contract/runtime concepts where useful, but not import Core internals or require Core online. Shared concepts should become explicit versioned packages/protocols when justified.

### IRIS observed direction
IRIS is the visual/multimodal engine with GEF/HIVE integration and planned Quality Kernel and Project OS Kernel. Forge should treat creative production as a capability provider through contracts and keep asset-build integration optional.

## M00 candidate base stack
- Rust: kernel/runtime and high-integrity native capabilities.
- Tokio: bounded async I/O/concurrency where measurements justify async.
- Serde: typed serialization.
- JSON Schema: language-neutral external contract validation.
- SQLite + SQLx: standalone local durable state where required.
- OpenTelemetry + tracing: vendor-neutral observability.
- MCP: interoperable AI/tool surface, not the sole architecture.
- Protobuf/gRPC: candidate for high-throughput typed IPC where benchmarked superior.
- Git: canonical engineering state.
- BLAKE3: fast content fingerprints/CAS keys.
- Zstandard: compact evidence/cache/snapshot payloads.
- cargo-nextest: efficient Rust test execution.
- Criterion: performance regression benchmarks.
- Clippy/rustfmt/cargo-deny/cargo-audit: quality and supply-chain gates.

No candidate library is frozen until M00 section planning and benchmark/maintenance/licensing review are complete.

## M00 planned proprietary technologies
1. Forge Contract Fabric (FCF): versioned contracts, capabilities, errors and compatibility.
2. Capability DNA: deterministic capability identity/fingerprint.
3. Deterministic Execution Envelope: reproducible execution inputs/environment/contracts/results.
4. Forge Capability Resolver: choose integrated or native providers by capability, health, policy, cost and confidence.
5. Zero-Dependency Boot Path: essential Forge operation without HIVE/Core/IRIS/Internet/LLM.
6. Hot Path Registry: explicit latency/throughput budgets and regression gates.
7. Resource Governor: CPU/RAM/I/O/concurrency budgets and backpressure.
8. Green Proof Cache foundation: reusable green-test proofs keyed by dependency/environment fingerprints.

## Jev/Laya research input
Jev represents a non-generative System-1 decision layer: typed choice/score/boolean-like judgments with calibrated probabilities can cheaply handle routing, filtering, triage and bounded evaluation instead of invoking a generative LLM for every decision.

Laya provides an open/local Jev-compatible decision pattern and can run locally, making it especially relevant to Sovereign Standalone Mode.

Forge will NOT hard-depend on Jev or Laya in M00. Instead M00 should define a provider-neutral Typed Decision Port:
- deterministic/code rules first when sufficient;
- local decision provider candidate (e.g. Laya-compatible) for bounded semantic decisions;
- remote decision provider candidate (e.g. Jev-compatible) when policy permits;
- generative LLM escalation only for open-ended reasoning;
- confidence threshold, calibration evidence, abstain/escalate behavior;
- batch multiple independent typed questions over the same state;
- cache decisions by state/question/provider/model fingerprint;
- benchmark quality, latency, tokens/cost and calibration before routing production decisions.

Candidate proprietary evolution: Forge Decision Plane (FDP), combining deterministic rules, local System-1 models, remote System-1 providers and LLM escalation behind one measured policy.

## M00 planning sections
S01 Kernel Runtime
S02 Module Lifecycle
S03 Forge Contract Fabric
S04 Capability Registry
S05 Capability Resolver / Forge Decision Plane boundary
S06 Dependency Graph
S07 Configuration
S08 State Store
S09 Event Bus
S10 Command Bus
S11 Error Taxonomy
S12 Idempotency & Cancellation
S13 Concurrency & Backpressure
S14 Cache/Fingerprints
S15 Plugin/Adapter SDK
S16 Compatibility Engine
S17 Health/Readiness
S18 Telemetry & Hot Path Registry
S19 Deterministic Execution Envelope
S20 Resource Governor
S21 Zero-Dependency Boot Path
S22 Test-Proof Foundation

## Research rule
External ideas are inputs, not authority. Forge adopts only mechanisms that survive reproducible internal benchmarks against native baselines. Marketing claims never become acceptance criteria without local evidence.
