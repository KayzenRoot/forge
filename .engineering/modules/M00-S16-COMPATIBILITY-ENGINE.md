# M00-S16 — Compatibility Engine

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S15

## Mission
Determine whether modules, contracts, capabilities, adapters, state schemas, protocols, artifacts and environments can safely interoperate or upgrade, using semantic evidence rather than version numbers alone.

## Core laws
1. SemVer is metadata, not proof.
2. Compatibility is directional: producer->consumer, old->new and new->old may differ.
3. Syntactic compatibility does not imply semantic/behavioral compatibility.
4. Unknown compatibility is not compatible.
5. State/migration/security compatibility can veto otherwise valid API compatibility.
6. Compatibility decisions are reproducible and evidence-backed.
7. Optional ecosystem integration cannot become a boot dependency through compatibility checks.

## Compatibility dimensions
- CONTRACT_SHAPE
- SEMANTIC
- BEHAVIORAL
- CAPABILITY
- STATE_SCHEMA
- MIGRATION
- PROTOCOL/WIRE
- SDK/FEP
- CONFIGURATION
- PLATFORM/ARCH
- TOOLCHAIN
- SECURITY/PERMISSION
- PERFORMANCE_BUDGET
- EVIDENCE/PROVENANCE

## Verdicts
COMPATIBLE
COMPATIBLE_WITH_ADAPTER
COMPATIBLE_WITH_MIGRATION
CONDITIONALLY_COMPATIBLE
INCOMPATIBLE
UNKNOWN

Conditions are machine-readable and must be satisfied before activation.

## Proprietary technologies

### Forge Compatibility Engine (FCE)
Deterministic compatibility evaluator over S03 Contract IR, S04 capability descriptors, S06 graph, S07 config, S08 state metadata and S15 extension manifests.

### Semantic Compatibility Vector (SCV)
Represents compatibility as a vector across dimensions instead of one boolean/version comparison. A release may be wire-compatible but state-incompatible, for example.

### Compatibility Proof Ledger (CPL)
Stores proof identity, compared fingerprints, rule-engine version, test evidence, migration evidence and verdict. Reuse requires unchanged relevant fingerprints.

### Compatibility Delta Analyzer (CDA)
Compares old/new CIR/manifests/schemas and classifies changes as additive-safe, conditionally-safe, behavior-sensitive, migration-required, security-sensitive or breaking.

### Bidirectional Contract Probe (BCP)
Where relevant, verifies both new-producer->old-consumer and old-producer->new-consumer paths rather than assuming symmetry.

### Shadow Compatibility Arena (SCA)
Runs candidate module/adapter/protocol versions against recorded/synthetic traffic in isolated/shadow mode before promotion when static proof is insufficient.

### Compatibility Membrane Adapter (CMA)
Explicit translation shim admitted only when it can preserve declared semantics. It has its own contract, tests, lifecycle and deprecation horizon; shims cannot silently hide permanent architectural divergence.

### Upgrade Safety Gate (USG)
Combines compatibility, migration, rollback, health and evidence requirements before S02 hot swap/upgrade can commit.

### Compatibility Debt Ledger (CDL)
Tracks temporary adapters/version exceptions/deprecations with owner, reason and removal condition so compatibility shims do not become immortal.

## Contract change classes
ADDITIVE_OPTIONAL
ADDITIVE_REQUIRED
CONSTRAINT_TIGHTENED
CONSTRAINT_RELAXED
FIELD_REMOVED
FIELD_RENAMED
TYPE_CHANGED
ENUM_EXPANDED
ENUM_RESTRICTED
DEFAULT_CHANGED
SEMANTIC_CHANGED
SIDE_EFFECT_CHANGED
PERMISSION_CHANGED
ORDERING_CHANGED
ERROR_MODEL_CHANGED
PERFORMANCE_CONTRACT_CHANGED

Classification is interpreted directionally.

## Compatibility pipeline
IDENTIFY OLD/NEW
-> LOAD FINGERPRINTS/CIR/MANIFESTS
-> STATIC DELTA
-> DIMENSION RULES
-> GRAPH IMPACT
-> STATE/MIGRATION CHECK
-> SECURITY/PERMISSION CHECK
-> PLATFORM/TOOLCHAIN CHECK
-> REUSE VALID CPL OR RUN CONTRACT/SHADOW TESTS
-> SCV
-> VERDICT + CONDITIONS
-> RECORD PROOF

## State/migrations
State compatibility requires schema/version ownership, forward migration, rollback/restore strategy and crash-interruption behavior. Destructive/irreversible migrations cannot be marked compatible solely because application code compiles.

## Behavioral compatibility
When semantics cannot be proven statically, use contract tests, golden fixtures, property tests, recorded safe traces and SCA. LLM judgment may assist triage/explanation but cannot be sole authority for hard compatibility.

## Security compatibility
Permission expansion, secret handling changes, trust-tier changes, weaker validation or new network/process authority are security-sensitive deltas even if API shape is unchanged. Such changes require explicit review/evidence.

## Performance compatibility
For contracts with declared latency/memory/throughput budgets, a candidate may be functionally compatible but fail promotion due to budget regression. Performance is a separate SCV dimension, not disguised as functional incompatibility.

## Platform/toolchain
Compatibility captures OS, architecture, libc/runtime where relevant, GPU/driver capability, compiler/runtime and external tool ranges. Forge avoids false universal compatibility claims from a single development machine.

## Version negotiation
Runtime negotiation uses supported ranges + capability/contract fingerprints + proof. Highest version is not automatically preferred; S05 selects among eligible compatible providers according to constraints.

## Caching
CPL entries integrate S14 PCCE. A compatibility proof is reusable only when compared contract/state/config/platform/security fingerprints and rule-engine version remain compatible.

## HIVE/Core/IRIS
Adapters publish supported public contract/capability ranges. FCE validates integration without importing private internals. HIVE may index historical proofs; Core/IRIS outages do not block Forge standalone compatibility for native paths.

## Extension updates
S15 extension update flow:
STAGE -> VERIFY PACKAGE -> CDA -> ECK TESTS -> FCE -> PERMISSION DELTA -> SCA if needed -> USG -> ACTIVATE/REJECT.
Permission expansion always requires explicit policy path.

## Evidence
Compatibility proof records:
- old/new identities and fingerprints;
- dimensions evaluated;
- rule-engine version;
- static deltas;
- tests/shadow/migration evidence;
- conditions/exceptions;
- verdict;
- expiration/invalidation triggers;
- reviewer/policy evidence where required.

## Test strategy
- directional schema compatibility fixtures;
- semantic change classification;
- enum/default/constraint edge cases;
- state migration forward/rollback/crash;
- FEP/protocol version skew;
- platform/toolchain matrices;
- permission expansion detection;
- behavioral shadow divergence;
- adapter translation property tests;
- CPL reuse/invalidation;
- UNKNOWN conservative behavior;
- performance budget regression;
- malformed/malicious compatibility metadata.

## Performance
- normalized CIR/schema fingerprints avoid repeated full comparisons;
- incremental CDA over semantic deltas;
- graph-scoped checks instead of ecosystem-wide scans;
- parallel independent dimension checks;
- cache proof reuse;
- shadow tests only when static proof insufficient/risk demands it;
- benchmark large contract/schema graphs and upgrade batches.

## Anti-pattern gates
Forbidden:
- "same major version = compatible";
- boolean compatibility without dimension evidence;
- assuming backward implies forward compatibility;
- auto-accepting unknown metadata;
- hiding breaking changes forever behind silent shims;
- ignoring migration/permission/platform changes;
- using LLM opinion as compatibility proof;
- hot swap before USG;
- claiming universal platform compatibility from one environment.

## Acceptance criteria
S16 is accepted when Forge can produce directional, multidimensional, reproducible compatibility verdicts with proof, migration/security/platform awareness, explicit adapter conditions and conservative UNKNOWN handling before modules/extensions/contracts are activated or upgraded.
