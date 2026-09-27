# M00-S14 — Cache, Fingerprints & Proof Reuse

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S13

## Mission
Accelerate Forge by reusing computation, context, decisions, tests, builds and evidence only when semantic equivalence can be established with sufficient confidence. Cache is never canonical truth.

## Core laws
1. Cache may disappear at any time without breaking correctness.
2. Reuse requires a semantic identity plus dependency/environment compatibility.
3. Invalidation is primarily causal/fingerprint-driven; TTL is freshness policy, not the main correctness mechanism.
4. Unknown dependency coverage causes conservative miss/revalidation.
5. Secrets/raw sensitive values do not enter reusable cache keys.
6. Cached failure/absence has bounded semantics.
7. Every cache layer exposes why a hit/miss/invalidation occurred.

## Fingerprint families
- CONTENT: exact bytes/artifact identity.
- SEMANTIC: normalized behaviorally relevant input.
- CONTRACT: S03 CIR semantic identity.
- CAPABILITY: S04 Capability DNA/provider identity.
- CONFIG: S07 Semantic Config Fingerprint.
- STATE_EPOCH: S08 relevant state/schema epoch.
- GRAPH_EPOCH/DELTA: S06 dependency/impact identity.
- TOOLCHAIN/ENVIRONMENT: compiler/runtime/OS/architecture relevant traits.
- DECISION: S05 request/provider/model/calibration/policy identity.
- TEST_PROOF: source/dependency/fixture/config/toolchain/test identity.
- BUILD: source/flags/dependencies/toolchain/platform.
Composite keys are versioned and typed.

## Proprietary technologies

### Forge Semantic Fingerprint Fabric (FSFF)
Canonical service/library for normalized, typed and versioned fingerprints. Prevents every module inventing incompatible cache-key logic.

### Proof-Carrying Cache Entry (PCCE)
A cache entry contains value reference plus proof metadata: semantic key, dependency fingerprints, producer/version, environment constraints, evidence state, freshness and invalidation policy.

### Causal Invalidation Engine (CIE)
Uses S06 Change Cone/semantic deltas to invalidate only entries whose declared dependencies intersect changed semantics. Unknown edges broaden invalidation.

### Green Proof Cache (GPC)
Stores reusable successful test/verification proofs keyed by test identity/version, relevant source/contract/config/fixture/toolchain/environment fingerprints and evidence. It never treats "passed once" as universally valid.

### Multi-Tier Cache Fabric (MTCF)
Logical tiers:
L0 execution-local memoization
L1 process memory
L2 local persistent cache/CAS
L3 optional shared/project cache
L4 optional remote/provider-native cache
Correctness does not require L3/L4.

### Cache Explainability Record (CER)
For every meaningful lookup, can report HIT, MISS, STALE, INVALIDATED, BYPASSED or QUARANTINED with machine reason codes and relevant fingerprint deltas.

### Reuse Confidence Gate (RCG)
Classifies reuse confidence from dependency coverage/provenance/risk. LOW confidence broadens validation or forces recomputation; HIGH_ASSURANCE operations demand stronger proof.

### Cache Stampede Shield (CSS)
Single-flight/coalescing + jittered refresh + bounded stale policies prevent many workers recomputing the same expensive item simultaneously.

### Negative Evidence Cache (NEC)
Caches bounded evidence of absence/failure only when semantics permit, with shorter/event-invalidated lifetime and no suppression of safety-critical recovery checks.

### Token Reuse Ledger (TRL)
Records context/prompt/decision reuse and estimates tokens/calls avoided versus spent. It measures real savings without allowing token minimization to weaken correctness.

Current M00 cache observations count exact-identity lookups and their outcomes. Cached values are provider-agnostic, so hit counts do not establish avoided model tokens, calls or money; those savings remain unmeasured until a provider supplies attributable usage data.

## Hashing
BLAKE3 remains primary candidate for high-throughput local fingerprints/CAS. Cryptographic/security-signature use cases are separate and choose algorithms appropriate to their threat model. Hash algorithm/version is encoded in identity.

## Canonicalization
Semantic hashing occurs after FCF normalization:
- stable field ordering;
- normalized enums/paths where contract permits;
- explicit absent/default semantics;
- exclusion of non-semantic metadata;
- canonical line-ending/encoding rules where relevant.
A normalization rule change versions the fingerprint schema and invalidates incompatible entries.

## Cache namespaces
Caches are namespaced by project/module/contract/cache-schema and security scope. Cross-project/tenant reuse is forbidden unless content is explicitly share-safe and policy permits it.

## Dependency manifests
Every reusable entry declares dependencies explicitly or derives them from proven graph edges. Examples:
- decision cache depends on state/context/provider/model/policy fingerprints;
- test proof depends on covered source/contracts/fixtures/toolchain;
- build cache depends on sources/deps/flags/toolchain/platform;
- context capsule depends on source versions/query/retrieval policy.
Hidden dependencies are cache correctness defects.

## Freshness
Freshness may be:
- IMMUTABLE_CONTENT;
- EVENT_INVALIDATED;
- DEPENDENCY_INVALIDATED;
- EPOCH_BOUND;
- TTL_BOUND;
- PROVIDER_DEFINED.
TTL-only caching is reserved for genuinely time-sensitive/externally unknowable freshness.

## Stale-while-revalidate
Allowed only for contracts explicitly tolerating stale data and never for security, permission, state integrity, migration or high-assurance correctness decisions. Staleness is surfaced in result metadata.

## Cache corruption
PCCE content fingerprints are verified on read according to risk/cost policy. Corrupt entries are quarantined/deleted and recomputed. Canonical state is never reconstructed from untrusted cache without independent validation.

## Eviction
Eviction uses size/age/cost/recompute value and pressure, not one universal LRU. S13 memory/disk pressure can trigger eviction. Pinned short-lived execution proofs are bounded. Cache quotas prevent one project from consuming the machine.

## HIVE integration
HIVE can supply context/retrieval caches and historical evidence through explicit adapters. Forge may reuse HIVE artifacts only with compatible source/policy fingerprints. HIVE outage falls back to Forge-local cache/native paths.

## Core/IRIS/providers
Provider-native caches are treated as capability properties with opaque/provider-defined semantics unless verifiable. Forge does not assume an external cache key equals its own semantic identity.

## Token/context optimization
- cache deterministic context capsules/deltas;
- reuse stable prompt prefixes when provider supports it;
- memoize typed decisions;
- avoid resending unchanged context;
- batch shared-context decisions;
- record source/context fingerprint so stale context cannot masquerade as savings;
- use HIVE progressive disclosure where available.

## Green Proof Cache flow
CHANGE
-> S06 semantic Change Cone
-> identify candidate proofs
-> compare proof dependency fingerprints
-> RCG risk/confidence
-> PRESERVE or INVALIDATE
-> execute only invalidated/required tests
-> store new proof/evidence.
Full/module certification gates may still require broader suites by policy.

## Performance
- compact typed fingerprint IDs;
- streaming/incremental hashing for large artifacts;
- avoid rehashing immutable content already proven by CAS identity;
- memory indexes for hot metadata, persistent payloads in L2/CAS;
- batch metadata reads/writes;
- single-flight expensive fills;
- compression only when CPU-vs-I/O benchmark favors it;
- benchmark hit latency, key construction, invalidation, CAS throughput, eviction and large-cache startup.

## Security/privacy
- no secret bytes in keys/logs;
- cache poisoning prevention via producer/provenance/contract checks;
- permission/security scope included in reuse eligibility;
- remote cache artifacts are untrusted until verified;
- sensitive cached values encrypted/protected where required or not cached;
- no cross-principal result sharing without explicit safe contract.

## Test strategy
- deterministic fingerprint golden/property tests;
- normalization/version migration;
- causal invalidation soundness;
- hidden-dependency conservative fallback;
- Green Proof preservation/invalidation fixtures;
- single-flight stampede tests;
- negative cache recovery;
- stale-while-revalidate policy tests;
- corruption/quarantine;
- eviction under disk/RAM pressure;
- cross-project/security isolation;
- HIVE/shared-cache outage;
- 1M-entry metadata scalability benchmarks;
- CAS dedup/compression benchmarks;
- token reuse accounting correctness.

## Metrics
- hit/miss/stale/invalidation rates by cache family;
- bytes/items and eviction;
- recompute time avoided;
- tests/build work avoided;
- tokens/calls/cost avoided;
- false reuse/correctness incidents target zero;
- invalidation breadth;
- single-flight coalescing rate;
- corruption/poison rejection.

## Anti-pattern gates
Forbidden:
- cache as canonical state;
- correctness based only on TTL;
- one global untyped cache namespace;
- keys containing secrets;
- reuse without dependency/environment identity;
- swallowing cache corruption;
- cross-tenant/project sharing by accident;
- stale security/permission decisions;
- "clear cache" as required normal repair path;
- claiming token savings without tracking correctness/rework.

## Acceptance criteria
S14 is accepted when Forge has versioned semantic fingerprints, proof-carrying multi-tier caches, causal invalidation, safe Green Proof reuse, stampede protection, corruption/security boundaries and measurable token/build/test savings while remaining fully correct with every cache empty.
