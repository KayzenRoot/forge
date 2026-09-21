# M00-S04 — Capability Registry

Status: PLANNED / NOT IMPLEMENTED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01 Kernel Runtime; S02 Module Lifecycle; S03 Forge Contract Fabric

## Mission
Maintain the deterministic, queryable and evidence-backed inventory of capabilities that Forge can actually provide at a point in time, independent of which concrete provider supplies them.

## Core distinction
Installed/configured/discovered is NOT equivalent to usable.
Capability availability progresses through evidence states:
DECLARED -> DISCOVERED -> CONTRACT_VALID -> PROBED -> VERIFIED
and may become DEGRADED, UNAVAILABLE, QUARANTINED or STALE.

Only policy-approved states may satisfy a capability request.

## Capability descriptor
Each capability declares:
- stable capability_id and semantic version;
- contract_id/version range;
- provider_id/provider kind;
- operations/features;
- quality/performance class;
- required permissions;
- resource requirements;
- locality: native/local-process/LAN/remote;
- determinism class;
- side-effect class;
- privacy/data-egress class;
- cost/token model where relevant;
- health/probe policy;
- dependencies;
- fallback relationship;
- evidence freshness;
- implementation fingerprint.

## Capability taxonomy
Examples:
- forge.source.git.inspect
- forge.contract.validate
- forge.test.run.targeted
- hive.context.search
- hive.memory.search
- core.orchestration.execute
- iris.asset.image.generate
- decision.system1.local
- decision.system1.remote
- llm.reasoning
- gpu.cuda
- executor.rust
Names are hierarchical but identity is a stable canonical ID, not free text.

## Proprietary technologies

### Capability DNA
Canonical fingerprint over semantic capability descriptor, provider implementation identity, contract compatibility and relevant policy/resource traits. Used for cache/test/evidence invalidation.

### Capability Evidence Ladder (CEL)
Separates claims from proof. VERIFIED requires successful probe/contract evidence within freshness policy. A stale proof cannot silently remain VERIFIED.

### Capability Snapshot Graph (CSG)
Immutable versioned graph of currently eligible capabilities, providers, dependencies and fallbacks. Commands bind to a snapshot so mid-flight discovery changes cannot silently alter execution semantics.

### Provider Equivalence Classes (PEC)
Groups providers that satisfy the same minimum contract while retaining quality/cost/locality/security differences. Enables safe substitution without pretending providers are identical.

### Capability Freshness Clock (CFC)
Per-capability evidence TTL/event invalidation rather than one global polling interval. Local deterministic capabilities may be event-invalidated; remote volatile providers can require shorter probes.

### Capability Quarantine
A provider producing contract violations, repeated health failures or suspicious output can be removed from eligibility without deleting its registration. Recovery requires explicit successful evidence.

### Negative Capability Cache (NCC)
Short-lived proof that an unavailable provider/capability was recently checked. Prevents repeated expensive probes/reconnect storms while allowing bounded recovery.

## Registry architecture
Two layers:
1. Definition Registry: durable capability/provider definitions and contracts.
2. Runtime Snapshot: immutable eligible-state view built from definitions + current evidence.

Reads on hot paths use immutable snapshots and avoid global mutation locks. Updates build a new snapshot then atomically publish it.

## Discovery sources
- Forge native capabilities;
- module manifests;
- local executors/toolchains/hardware probes;
- adapter manifests;
- HIVE/Core/IRIS integration adapters;
- MCP servers;
- Skills/capability packages;
- explicitly configured remote providers.

Discovery never grants authority. Contract, permission, health and policy checks remain separate.

## Standalone priority
Every capability classified FORGE_CORE has a native/local path or explicit boot-safe substitute. External ecosystem providers can outrank native implementations only through S05 resolution policy; their absence never erases native registration.

## HIVE/Core/IRIS behavior
HIVE, Core and IRIS register through adapter providers. Their capabilities carry contract version, health, locality, freshness and evidence. Provider outage changes eligibility/snapshot and triggers dependent lifecycle recalculation without crashing the kernel.

## Hardware/toolchain capabilities
Platform probes may register CPU architecture, logical/physical resource class, memory budget, filesystem traits, Git, Rust toolchain, containers, GPU/accelerator and other execution capabilities. Sensitive hardware identifiers are not required for normal capability identity.

## Decision providers
Jev/Laya-like providers register as typed-decision capabilities with explicit model/provider fingerprint, locality, supported output cardinality/schema, calibration evidence, latency class, privacy/egress policy and confidence semantics. They never receive generic authority merely because they are available.

## Query model
Consumers request constraints, not provider names:
- capability contract/range;
- required features;
- maximum side-effect class;
- locality/privacy requirement;
- determinism requirement;
- latency/cost/resource bounds;
- minimum evidence state/freshness.
S05 resolves the best eligible provider.

## Event model
Registry emits typed events for definition/evidence/snapshot changes. Events contain IDs/fingerprints/reasons, not secrets. Change coalescing prevents discovery storms.

## Performance
- immutable read-optimized snapshots;
- BLAKE3 descriptor/snapshot fingerprints;
- incremental graph rebuild for localized changes;
- probe deduplication/single-flight;
- negative caching for outages;
- batched discovery;
- no network calls on ordinary registry read path;
- bounded provider/probe concurrency;
- benchmark lookup, snapshot rebuild, discovery and large-registry scaling.

## Security
Capability registration is not permission. Provider manifests are validated/signed or provenance-bound according to trust class. External metadata is untrusted input. Registry prevents privilege expansion through capability aliases/version confusion. Sensitive configuration is referenced, never copied into descriptors/fingerprints.

## Test economics
Capability DNA and CSG fingerprints become Green Proof keys. If an unrelated provider changes, proofs that do not depend on it remain valid. Changes in provider equivalence or capability contract invalidate only intersecting proof sets unless confidence is insufficient.

## Required tests
- deterministic descriptor/snapshot fingerprints;
- evidence-state transition/property tests;
- stale evidence expiration;
- provider outage/recovery;
- quarantine/recovery;
- negative-cache behavior;
- equivalent-provider registration;
- contract/version mismatch;
- privacy/locality constraint queries;
- concurrent discovery/snapshot publication;
- large registry lookup/rebuild benchmarks;
- HIVE/Core/IRIS absent-at-boot and disappearing-at-runtime;
- Jev/Laya provider unavailable/fallback;
- malformed/untrusted manifest security tests.

## Acceptance criteria
S04 is accepted when Forge can represent, prove, snapshot and query available capabilities without coupling callers to providers, while preserving standalone operation, evidence freshness, fast reads, security boundaries and precise test invalidation.
