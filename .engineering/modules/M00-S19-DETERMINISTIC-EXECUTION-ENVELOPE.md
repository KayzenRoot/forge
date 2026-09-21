# M00-S19 — Deterministic Execution Envelope

Status: PLANNED / NOT IMPLEMENTED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S18

## Mission
Make Forge executions explainable, reproducible and comparable by binding every governed operation to an explicit snapshot of the inputs, contracts, code, configuration, capabilities, environment and nondeterministic boundaries that can affect its result.

## Core laws
1. Determinism is a declared property, not an assumption.
2. Reproducibility does not require pretending external systems are deterministic.
3. Every nondeterministic input is controlled, captured, abstracted or explicitly marked external/unreplayable.
4. Side effects are never replayed accidentally.
5. Reproduction compares semantic outcomes, not only byte-for-byte output when contracts permit nondeterminism.
6. Missing critical provenance downgrades replay confidence.
7. Deterministic evidence must be usable without HIVE/Core/IRIS/Internet.

## Determinism classes
D0_STRICT — same envelope must produce byte-identical result.
D1_SEMANTIC — result may differ in non-semantic metadata but satisfies identical semantic contract.
D2_BOUNDED — controlled variance/range/order is contractually allowed.
D3_RECORDED_EXTERNAL — external response/observation must be recorded/stubbed for replay.
D4_LIVE_EXTERNAL — result depends on current external state and cannot be faithfully replayed without that state.
D5_IRREVERSIBLE — execution includes non-replayable side effect; only simulation/dry replay is allowed.

## Execution envelope
The Deterministic Execution Envelope (DEE) binds:
- execution/work-order/command identity;
- source tree/commit/content fingerprints;
- relevant Contract IR fingerprints;
- module/capability/provider versions;
- immutable config capsule;
- state/schema/graph epochs;
- semantic inputs;
- environment/platform/toolchain;
- dependency/artifact fingerprints;
- resource policy/profile;
- locale/timezone/encoding where semantically relevant;
- random seed/source;
- clock policy;
- filesystem/path normalization policy;
- concurrency/order policy where relevant;
- external interaction policy and recorded exchange references;
- model/provider/prompt/context fingerprints for AI calls;
- side-effect class;
- expected determinism class;
- evidence/result fingerprints.

## Proprietary technologies

### Deterministic Execution Envelope (DEE)
Canonical proof object describing the complete execution boundary and the factors that may affect its outcome.

### Nondeterminism Boundary Map (NBM)
Graph of all known nondeterministic sources touched by an execution: clock, RNG, scheduler, filesystem ordering, network, provider, LLM, GPU kernels, OS/toolchain, external mutable state.

### Determinism Contract (DC)
FCF extension declaring expected determinism class, allowed variance, canonicalization/comparison rules and replay restrictions for an operation.

### Replay Capsule (RCAP)
Portable package of envelope + immutable inputs/artifacts + safe recorded external fixtures + expected semantic assertions needed to reproduce/debug an execution without copying unrelated project data.

### External Interaction Recorder (EIR)
At explicitly allowed boundaries, records sanitized request/response contract identities and replayable fixtures. Secret/auth material is never persisted as raw replay data.

### Side-Effect Replay Firewall (SERF)
Blocks external mutation during replay by default. Replays use mocks/simulators/read-only verification unless a separately authorized command explicitly permits effects.

### Determinism Drift Detector (DDD)
Runs equivalent envelopes and detects unexpected semantic divergence, attributing likely changed dimensions from fingerprints/NBM.

### Reproducibility Confidence Score (RCS)
Evidence-derived level based on provenance completeness and determinism class. It is not a quality score; it states how strongly Forge can reproduce/explain a result.

## Sources of nondeterminism

### Time
Runtime deadlines use monotonic clocks. Business/current-time access goes through a clock port when deterministic replay matters. Wall clock values affecting semantics are captured.

### Randomness
Operations requiring replayable randomness use seeded/recordable RNG through explicit ports. Security cryptography uses secure randomness and may be non-replayable by design; secrets/seeds are not weakened for determinism.

### Concurrency
Do not globally serialize Forge for determinism. Contracts identify whether order matters. Tests may use deterministic schedulers/models for race-sensitive components while production retains safe parallelism.

### Filesystem
Directory iteration/order, path separators, line endings, permissions and case sensitivity are normalized or recorded when relevant. Windows/Linux differences are explicit envelope dimensions.

### Network/external APIs
Live calls are D3/D4/D5 depending on recording/effect semantics. EIR records only what policy permits. Provider operation IDs and response fingerprints support explanation.

### LLM/model calls
Remote generative models are generally not strict deterministic dependencies. Envelope records provider/model/version if available, parameters, prompt/context fingerprints, tool schema fingerprints, seed if supported, cached response/proof and evaluation assertions. Replay can reuse recorded result or re-evaluate semantically; it must not claim byte determinism.

### GPU
GPU/device/driver/library/kernel determinism settings are captured when relevant. Some kernels remain nondeterministic; DC declares tolerated variance.

## Replay modes
VERIFY_ONLY — validate envelope/evidence without execution.
PURE_REPLAY — rerun strict/pure operation.
FIXTURE_REPLAY — replace external interactions with recorded fixtures.
SIMULATION — use mocks/sandboxes for effects.
SHADOW_REPLAY — execute candidate implementation without committing effects.
LIVE_REEXECUTION — new authorized execution, not considered replay of irreversible effects.

## Semantic comparison
DC may define:
- exact bytes/hash;
- normalized structured equality;
- unordered set equality;
- numeric tolerance;
- invariant/property assertions;
- contract-valid output;
- quality/eval threshold for AI outputs.
Comparison logic is versioned and part of proof.

## Evidence integration
Command Result Proof, Compatibility Proof, Green Proof, error fingerprints and telemetry refs can point to DEE. A patch/regression report can state which envelope changed and which dimensions remained identical.

## Cache integration
S14 cache keys may reuse DEE sub-fingerprints. A cache hit never overrides a DC requirement. Replay artifacts are content-addressed and deduplicated.

## Security/privacy
RCAP is least-data by design. Secrets are references/redacted; production credentials are never bundled. Sensitive fixtures require policy/retention/encryption. Replaying untrusted recorded data still passes FCF validation.

## Performance
DEE uses references/fingerprints rather than copying large artifacts. Envelope creation is incremental and reuses S14 hashes. NBM is assembled from S06/runtime evidence. Full RCAP materialization is on-demand, not every execution.

## Test strategy
- strict deterministic golden replay;
- semantic/tolerance replay;
- clock/RNG injection;
- Windows/Linux path/order fixtures;
- concurrent ordering-sensitive tests;
- recorded external API replay;
- SERF side-effect blocking;
- LLM recorded-result/semantic evaluation;
- GPU tolerance metadata;
- missing provenance/RCS downgrade;
- drift attribution;
- RCAP secret-redaction;
- envelope overhead benchmarks.

## Anti-pattern gates
Forbidden:
- claiming all executions deterministic;
- replaying irreversible external effects automatically;
- storing production secrets inside replay capsules;
- using wall clock/RNG directly in replay-sensitive core logic;
- assuming same source commit means same environment;
- byte-comparing outputs whose contracts allow semantic variance;
- disabling concurrency globally merely to manufacture determinism;
- calling a fresh LLM response a deterministic replay.

## Acceptance criteria
S19 is accepted when Forge can declare determinism expectations, capture the execution factors that matter, safely reproduce/simulate eligible operations, block side effects during replay, detect semantic drift and explain limits when an external/nondeterministic result cannot be exactly reproduced.
