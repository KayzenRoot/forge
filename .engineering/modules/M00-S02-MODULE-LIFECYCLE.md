# M00-S02 — Module Lifecycle

Status: PLANNED / NOT IMPLEMENTED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01 Kernel Runtime primitives

## Mission
Define the deterministic lifecycle, isolation, dependency and evolution model for every Forge module so modules can be planned, built, certified and upgraded independently behind frozen public contracts.

## Core rule
A module is a governed capability unit, not merely a folder or crate. It owns an explicit manifest, contracts, capability exports/imports, state ownership, lifecycle hooks, health, resource requirements, compatibility range and evidence identity.

## Module manifest
Every module declares at minimum:
- stable module_id and human name;
- module contract version;
- implementation/build identity;
- exported capabilities and versions;
- required and optional imported capabilities;
- configuration schema/version;
- persistent-state ownership/migration version when applicable;
- lifecycle hooks;
- resource class/budgets;
- permissions;
- health/readiness contract;
- compatibility constraints;
- feature/capability flags;
- evidence/attestation identity.

Manifests are canonicalized and fingerprinted. Undeclared runtime dependency access is a conformance defect.

## Lifecycle state machine
DISCOVERED -> VALIDATED -> RESOLVED -> PREPARED -> STARTING -> READY
                                                   |         |
                                                   v         v
                                                FAILED    DEGRADED
                                                              |
                                                              v
                                                           READY
READY/DEGRADED -> QUIESCING -> STOPPING -> STOPPED

Upgrade path:
READY -> QUIESCING -> SNAPSHOT/MIGRATE -> CANDIDATE_START -> VERIFY -> COMMIT -> READY
                                                        \-> ROLLBACK -> previous READY

Invalid transitions are rejected and recorded.

## Dependency model
Modules depend on capabilities/contracts, not concrete module internals.
- Required dependency unavailable: dependent module cannot become READY.
- Optional dependency unavailable: module may become DEGRADED with explicit missing capability.
- Cycles in required startup dependencies are forbidden.
- Optional cycles require explicit event-safe design and validation.
- Dependency graph is resolved before side-effectful start.
- Capability version ranges and compatibility proofs are checked before activation.

## Proprietary technologies

### Module Genome Manifest (MGM)
Canonical machine-readable identity for a module: contracts, capabilities, state, permissions, resources and compatibility. Fingerprinted for evidence and cache invalidation.

### Lifecycle Transaction Protocol (LTP)
Treats start/stop/upgrade as bounded transactions with prepare, commit and compensating rollback phases. Goal: prevent half-started or half-upgraded module states.

### Dependency Blast-Radius Graph (DBG)
Precomputes which modules/capabilities can be affected by a lifecycle or contract change. Feeds later test selection, rollout, review and incident diagnosis.

### Compatibility Proof Ledger (CPL)
Stores machine-verifiable evidence that a module implementation satisfies the contract/version combinations it claims. Compatibility is proven by contract tests rather than assumed from version strings.

### Quiescence Barrier (QB)
Before stop/upgrade, blocks new work, tracks in-flight leases and drains/cancels according to deadline policy. Persistent state is not migrated while uncontrolled work is still mutating it.

### Shadow Activation
A candidate implementation may initialize and validate against read-only/synthetic traffic without becoming authoritative. Promotion occurs only after health/contract checks pass. Use only where side effects can be safely suppressed.

### Module Circuit Isolation (MCI)
Repeated module failure opens an isolation circuit. Kernel remains alive, dependents are recalculated, optional paths degrade and restart storms are prevented.

## Hot upgrade policy
Hot replacement is not a universal promise. It is allowed only when contracts, state model, resource ownership and platform support prove it safe. Otherwise Forge uses controlled quiesce/restart. Correctness beats theatrical zero-downtime.

## State ownership
A persistent datum has one authoritative owning module. Other modules access it through contracts. Cross-module direct database/table mutation is forbidden. State migrations are owned/versioned by the owner and must provide forward validation plus rollback/recovery strategy where technically possible.

## Failure containment
Module panic/crash/failure is contained at its boundary when process/runtime safety allows. Kernel records causal evidence, marks capability health, recalculates dependents and applies restart policy. Restart uses bounded exponential backoff + jitter and a retry budget; no infinite restart loop.

## Standalone/ecosystem behavior
HIVE/Core/IRIS adapters are modules/providers under the same lifecycle law. Their absence cannot prevent native CORE Forge modules from booting. Their capabilities appear/disappear through explicit snapshots and lifecycle events, never hidden global checks.

## Build-by-module enforcement
A module under construction cannot silently consume future module internals. It may depend on:
1. already-certified contracts/capabilities;
2. frozen interface stubs explicitly admitted by ADR;
3. standard/external dependencies approved by policy.
This enforces the plan-complete -> build-complete -> certify -> next-module strategy.

## Test economics integration
DBG supplies affected module/capability edges to Green Proof Cache/Test Impact Graph. A change invalidates only proofs whose dependency fingerprint intersects the blast radius, unless confidence is insufficient. Lifecycle contract changes trigger broader compatibility tests automatically.

## Performance
Lifecycle orchestration must avoid global locks on normal command/event hot paths. Manifest parsing/validation and graph resolution are startup/change-path work and may be cached by fingerprints. State transitions use small immutable snapshots and bounded synchronization.

## Security
Module permissions/capabilities are least-privilege. Lifecycle activation validates permission manifest before side effects. A module cannot self-expand permissions at runtime. Untrusted/dynamic third-party code is outside the trusted in-process kernel and will be addressed by adapter isolation in S15.

## Observability
Every transition emits structured event: module_id, previous/new state, reason code, contract/implementation fingerprint, duration, affected capabilities and correlation ID. Secrets and sensitive payloads are excluded.

## Required tests
- property/state-machine tests for valid/invalid transitions;
- dependency DAG and cycle tests;
- required vs optional dependency outage tests;
- quiescence/deadline/cancellation tests;
- failed-start compensation tests;
- restart-budget/storm tests;
- compatibility proof tests;
- state migration rollback/recovery tests where applicable;
- ecosystem adapter disappearance/recovery tests;
- deterministic graph/fingerprint tests;
- blast-radius correctness tests;
- performance baselines for graph resolution and transitions.

## Acceptance criteria
S02 is accepted when module identity, lifecycle states, dependency resolution, state ownership, upgrade/rollback, failure isolation, compatibility evidence, build-order enforcement and test-impact hooks are explicit and compatible with S01.
