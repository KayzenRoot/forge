# M00-S06 — Dependency & Causality Graph

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S05

## Mission
Maintain a deterministic multi-layer graph describing what can causally affect what across Forge. Use it to minimize rebuilds, retests, cache invalidation and operational blast radius without sacrificing correctness.

## Graph principle
A dependency edge is typed and directional. "A depends on B" is insufficient. Forge records WHY and HOW changes propagate.

## Node families
- module
- contract
- capability/provider
- source package/crate/file/symbol (progressively enriched by M05)
- command/event
- configuration key/schema
- persistent-state schema/migration
- permission/policy
- resource class
- test/suite/fixture
- build artifact
- evidence/proof
- external integration
- runtime environment/toolchain

M00 owns graph primitives and module/contract/capability-level nodes. Fine source-symbol intelligence is added by M05; test coverage edges by M06.

## Edge families
Examples:
- REQUIRES_CAPABILITY
- IMPLEMENTS_CONTRACT
- CALLS_COMMAND
- SUBSCRIBES_EVENT
- READS_CONFIG
- OWNS_STATE
- MIGRATES_STATE
- REQUIRES_PERMISSION
- CONSUMES_RESOURCE
- TESTS
- COVERS
- PRODUCES_ARTIFACT
- PROVES
- INVALIDATES
- FALLS_BACK_TO
- COMPATIBLE_WITH
- GENERATED_FROM

Each edge can carry semantic scope, version/range, strength (required/optional), propagation rule and provenance.

## Proprietary technologies

### Forge Causality Graph (FCG)
Unified typed graph joining lifecycle, contracts, capabilities, tests, builds and evidence while allowing later modules to add richer node/edge families.

### Semantic Impact Propagator (SIP)
Propagates a semantic change delta only across edge types whose propagation rules intersect that delta. Prevents a README edit from behaving like an API break.

### Proof Preservation Engine (PPE)
Computes which existing green tests/build/evidence proofs remain valid after a change. Default is conservative: uncertainty invalidates proof reuse.

### Change Cone
Compact deterministic subgraph representing all potentially affected downstream nodes from a specific change set. Used by test selection, build planning, review and release evidence.

### Reverse Causality Index (RCI)
Maintains fast reverse edges so Forge can answer "who depends on this?" without graph-wide scans.

### Graph Epoch Snapshot (GES)
Immutable fingerprinted graph snapshot used by a Work Order/execution. Mid-run graph updates create a new epoch and do not silently alter the active execution.

### Dependency Confidence Score (DCS)
Edges have provenance/confidence: DECLARED, GENERATED, OBSERVED, PROVEN. Low-confidence impact paths cause broader validation rather than aggressive reuse.

### Graph Compaction Layer (GCL)
Stores repeated dependency patterns structurally and materializes detailed subgraphs on demand, reducing memory/storage for very large workspaces.

## Multi-resolution graph
L0 ecosystem/project
L1 module
L2 package/crate/service
L3 file
L4 symbol/contract operation
L5 test/evidence/runtime observation
Consumers request only the resolution needed. M00 begins L0-L2 plus contract/capability primitives; M05/M06 deepen L3-L5.

## Change classification
Changes are classified before propagation:
- NON_SEMANTIC: formatting/comments/prose without governed semantic metadata.
- IMPLEMENTATION_LOCAL: internal behavior with frozen public contract.
- CONTRACT_COMPATIBLE: public semantic change classified compatible by S03.
- CONTRACT_CONDITIONAL.
- BREAKING.
- CONFIG/POLICY.
- STATE/MIGRATION.
- SECURITY/PERMISSION.
- TOOLCHAIN/ENVIRONMENT.
Unknown classification is conservative.

## Impact algorithm
1. Normalize changed inputs.
2. Compute semantic deltas/fingerprints.
3. Locate seed nodes.
4. Traverse only applicable typed propagation edges.
5. Apply edge confidence and risk policy.
6. Produce Change Cone.
7. Derive invalidated proof/cache/build/test sets.
8. Record Impact Proof with graph epoch and reasons.

## Performance architecture
- immutable read snapshots;
- adjacency + reverse adjacency indexes;
- compact integer/internal IDs mapped from stable IDs;
- incremental edge/node updates;
- SCC detection for allowed graph domains;
- topological caches for DAG portions;
- memoized impact queries by graph epoch + semantic delta fingerprint;
- persisted snapshots/deltas where worthwhile;
- parallel traversal only above measured thresholds;
- no database/network call required for hot impact queries once snapshot loaded.

Exact graph storage implementation remains benchmark-driven. Do not prematurely adopt a heavyweight graph database.

## Cycle policy
Required module startup graph: cycles forbidden.
Other graph domains may contain legitimate cycles (events, source calls, fallback relationships) and are modeled explicitly. Cycle semantics are edge-family-specific, not globally banned.

## Build/test/cache integration
- M03 uses FCG for Build Graph scheduling.
- M05 adds AST/symbol/source edges.
- M06 adds test/coverage/failure edges.
- Green Proof Cache asks PPE before reuse.
- M09 uses Change Cone for incremental builds.
- M12 uses Impact Proof in release evidence.
- M20 uses graph for failure blast-radius/recovery.

## HIVE integration
HIVE may store/retrieve graph-derived context and historical impact evidence. Forge owns current execution graph truth. HIVE can enrich retrieval but is not required for impact analysis.

## Core/IRIS/external integration
External systems appear as integration/capability/contract nodes, not internal implementation graphs unless explicit versioned graph exchange contracts exist. This preserves sovereignty and avoids false dependency precision.

## Security
Graph data is security-sensitive because it reveals architecture. Persisted/exported views obey project permissions. Secret values never become graph labels. Untrusted manifests cannot inject arbitrary trusted edges without validation/provenance.

## Correctness rule
Optimization is allowed only when proof preservation is sound. If Forge cannot prove an edge set complete enough for a risk class, it broadens test/build scope. False-positive impact costs time; false-negative impact can ship defects, so policy is intentionally asymmetric.

## Test strategy
- deterministic graph/fingerprint tests;
- typed propagation property tests;
- reverse-index equivalence;
- cycle/SCC tests;
- graph epoch isolation;
- semantic vs non-semantic delta tests;
- low-confidence escalation;
- proof-preservation soundness fixtures;
- synthetic 1k/10k/100k/1M-node scalability benchmarks;
- incremental update benchmarks;
- Windows/Linux deterministic serialization;
- malformed/untrusted graph input tests.

## Metrics
- impact-query p50/p95/p99;
- graph rebuild/update latency;
- memory per node/edge class;
- proof reuse rate;
- false-positive impact rate;
- detected false-negative rate target zero;
- percentage of edges by confidence class;
- tests/build work avoided with preserved correctness.

## Acceptance criteria
S06 is accepted when Forge has a typed, multi-resolution, snapshot-based causality graph with conservative semantic propagation, reverse dependency queries, proof-preservation hooks and scalable incremental architecture that future M05/M06/M09 modules can enrich without replacing.
