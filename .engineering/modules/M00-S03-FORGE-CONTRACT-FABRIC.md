# M00-S03 — Forge Contract Fabric (FCF)

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01 Kernel Runtime; S02 Module Lifecycle

## Mission
Provide one deterministic, language-neutral and machine-verifiable contract system for Forge modules, capabilities, commands, events, adapters and ecosystem boundaries. Contracts are executable engineering truth, not prose-only documentation.

## Contract families
FCF defines distinct typed families:
1. Data Contract — payload/schema/value invariants.
2. Capability Contract — what a provider promises and requires.
3. Command Contract — request/result/errors/idempotency/deadline semantics.
4. Event Contract — immutable fact, ordering/replay/delivery semantics.
5. Lifecycle Contract — module lifecycle obligations/hooks.
6. Resource Contract — CPU/RAM/I/O/concurrency/resource-class expectations.
7. Health Contract — readiness/degradation/failure semantics.
8. Permission Contract — explicit authority and side-effect classes.
9. Evidence Contract — proof/evidence shape and provenance.
10. Integration Contract — external HIVE/Core/IRIS/MCP/Skill/CLI/API boundary.

## Contract identity
Every contract has:
- stable contract_id;
- family;
- semantic contract version;
- canonical schema representation;
- input/output schemas;
- invariants/preconditions/postconditions;
- declared error set;
- compatibility policy;
- capability/permission/resource references;
- deprecation metadata;
- provenance/owner;
- deterministic fingerprint.

Implementation versions are separate from contract versions.

## Proprietary technologies

### Forge Contract Fabric (FCF)
The full contract registry/compiler/validator model. Source contracts compile to runtime validators, canonical fingerprints, documentation and test fixtures/adapters where safe.

### Contract IR (CIR)
A minimal language-neutral intermediate representation. JSON Schema/Protobuf/MCP/tool schemas are adapters/views, not the canonical internal truth. CIR prevents Forge architecture from being captured by one serialization technology.

### Contract Proof Pack (CPP)
Machine-verifiable bundle containing canonical contract fingerprint, compatibility result, generated/curated contract tests, implementation identity and evidence references.

### Semantic Compatibility Engine (SCE)
Compatibility is evaluated from structural + semantic policy, not SemVer string comparison alone. Classifies changes as COMPATIBLE, CONDITIONALLY_COMPATIBLE, MIGRATION_REQUIRED or BREAKING.

### Boundary Guard Compiler (BGC)
Generates validation/normalization guards for trust boundaries from CIR. Internal hot paths may use already-validated typed values to avoid repeated parsing.

### Contract Test Synthesizer (CTS)
Generates baseline positive/negative/boundary/unknown-field/version-skew tests from contract schemas and invariants. Generated tests complement, never replace, hand-authored semantic tests.

### Contract Delta Fingerprint (CDF)
Fingerprints the semantic portions changed between contract revisions and feeds Dependency Blast-Radius Graph/Test Impact Graph. Cosmetic documentation changes do not invalidate unrelated test proofs.

### Ecosystem Compatibility Membrane (ECM)
Adapter boundary for HIVE/Core/IRIS/external protocols. Maps external contract versions into FCF without leaking external representation into kernel internals.

## Canonical representation
Candidate source format: human-reviewable YAML/JSON or typed Rust builder compiled into CIR. Final authoring format is frozen only after prototype/benchmark ergonomics review.
Canonical hashing uses normalized CIR, stable ordering and BLAKE3. Non-semantic metadata is separated from semantic fingerprint material.

## Schema technologies
- JSON Schema: preferred language-neutral data validation/export.
- Protobuf: optional high-throughput IPC/wire projection where benchmark justified.
- MCP schemas: generated/mapped projection for AI/tool interoperability.
- Rust types + Serde: native compiled representation.
No projection becomes the source of truth.

## Compatibility rules
Examples:
- adding truly optional field with defined default/absence semantics: potentially compatible;
- removing/renaming required field: breaking;
- narrowing accepted enum/range: breaking for callers using removed values;
- widening output without tolerant-reader guarantee: conditional;
- changing error/retry/idempotency semantics: semantic change even if schema identical;
- permission/resource side-effect expansion: requires explicit review and may be breaking.
SCE must understand these policy dimensions.

## Unknown-field/version-skew strategy
Contracts explicitly declare tolerant vs strict readers. Unknown fields are never silently accepted merely for convenience. Version negotiation uses supported contract ranges/capabilities rather than guessing from implementation version.

## Invariants
Schema validation alone is insufficient. CIR supports declarative invariants where feasible and references deterministic validators for complex rules. Probabilistic LLM/System-1 judgment is not valid for hard contract enforcement.

## Error contracts
Commands/capabilities declare stable machine-readable error codes/classes and retryability. Undeclared error leakage across a public boundary is a conformance defect. Internal causal detail may be retained in evidence while public errors remain safe.

## Security and trust boundaries
Untrusted external payloads validate once at the boundary. Validation includes size/depth/count limits to resist resource-exhaustion payloads. Secret/sensitive fields carry classification metadata controlling logs, fingerprints, evidence and serialization.

## Performance
- Compile/cache validators by contract fingerprint.
- Avoid reparsing schemas per request.
- Validate once on ingress; pass typed validated values internally.
- Fast-path compatible contract fingerprints.
- Lazy-load rarely used contract projections.
- Benchmark JSON Schema vs generated/native validators on representative payloads before choosing hot-path strategy.
- No reflection-heavy design on proven hot paths without evidence.

## Standalone behavior
FCF registry/compiler/validators operate locally with no HIVE/Core/IRIS/network/LLM dependency. External ecosystem contracts can be imported as cached/versioned adapter descriptors but their providers need not be online.

## HIVE integration
HIVE may index/retrieve contract source, decisions, evidence and deltas. Forge remains canonical for runtime contract enforcement. HIVE outage never disables local FCF.

## Core/Hades integration
Core orchestration may consume capability/command/permission contracts. Shared contracts should be published as explicit versioned artifacts/protocol packages rather than importing private crate internals.

## IRIS integration
IRIS asset/job/provenance capabilities cross the Ecosystem Compatibility Membrane. Forge validates and fingerprints the contract while IRIS retains ownership of creative-domain semantics.

## MCP/Skills integration
MCP tool schemas and Skill manifests are projections/adapters into FCF. Forge can expose FCF capabilities over MCP without making MCP the internal bus.

## Test strategy
- canonicalization/fingerprint golden tests;
- property tests for serialization round trips;
- generated positive/negative/boundary tests;
- semantic compatibility matrix tests;
- version-skew/tolerant-reader tests;
- malformed/oversized/deep payload tests;
- sensitive-field redaction tests;
- cross-language/projection equivalence tests;
- deterministic output tests across Windows/Linux;
- mutation tests against validators/compatibility rules;
- benchmarks for compile, validate, negotiate and fingerprint paths.

Green Proof reuse keys include contract semantic fingerprints. CDF invalidates only proofs intersecting changed semantics unless compatibility confidence is insufficient.

## Quality gates
A public contract cannot be promoted without:
- canonical CIR;
- stable ID/version;
- compatibility classification;
- declared errors;
- security/resource limits;
- contract tests;
- fingerprint;
- owner/provenance;
- deprecation/migration policy where relevant;
- Contract Proof Pack.

## Acceptance criteria
S03 planning is accepted when Forge has a protocol-neutral canonical contract model, deterministic identity, executable validation, semantic compatibility rules, ecosystem membranes, security/performance strategy and direct integration with lifecycle/test-impact evidence.
