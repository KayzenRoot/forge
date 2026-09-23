# M00-S12 — Idempotency, Cancellation & Deadline Control

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S11

## Mission
Provide shared primitives that make retries, duplicate delivery, cancellation, deadlines and abandoned-work control safe across commands, events, adapters and modules.

## Core laws
1. Idempotency means same semantic intent cannot produce an unintended duplicate effect within its declared scope.
2. Cancellation means stop further owned work as safely/quickly as possible; it does NOT mean rollback.
3. Timeout means deadline expired; it does NOT prove whether an external effect occurred.
4. Child work inherits a bounded time/cancellation/resource lineage unless explicitly detached through a new authorized root.
5. No invisible zombie work after caller/lifecycle/resource ownership ends.

## Idempotency identity
An idempotency identity may include:
- command/operation contract ID + semantic version scope;
- canonical semantic input fingerprint;
- caller/tenant/project scope;
- target resource/effect identity;
- explicit caller key where required;
- provider-native key mapping;
- retention/expiry policy.

Non-semantic metadata such as timestamps/tracing IDs is excluded unless contract semantics require it.

## Idempotency modes
NONE
CALLER_KEYED
FORGE_DERIVED
STATE_TRANSITION_GUARDED
PROVIDER_NATIVE
COMPOSITE
Each operation contract declares supported mode and duplicate-result semantics.

## Duplicate states
FIRST_SEEN
IN_FLIGHT
SUCCEEDED
FAILED_RETRYABLE
FAILED_FINAL
UNKNOWN_OUTCOME
EXPIRED
A duplicate arriving during IN_FLIGHT may join/wait/reject according to contract; it must not blindly execute again.

## Proprietary technologies

### Semantic Idempotency Key (SIK)
Canonical fingerprint of semantic intent and declared scope, derived through FCF normalization. Prevents accidental duplicate effects caused by transport retries or superficial request differences.

### Idempotency Proof Record (IPR)
Durable S08 record linking SIK to execution/effect/result/evidence state. A cached result is reusable only when contract/provider/state fingerprints remain compatible.

### In-Flight Coalescer (IFC)
Concurrent identical safe operations can share one owned execution and independently await the result. Cancellation of one waiter does not cancel shared work while another valid owner remains.

### Cancellation Lineage Tree (CLT)
Every owned task belongs to a cancellation tree. Cancellation propagates downward with reason/deadline/resource context. Detached work requires explicit re-root authorization.

### Deadline Budget Tree (DBT)
Parent deadline becomes a consumable budget distributed among children. Scheduling/resolution can reject work before start when remaining budget cannot plausibly satisfy minimum execution requirements.

### Cancellation Safe-Point Protocol (CSP)
Long CPU/blocking/multi-stage operations declare bounded safe points where cancellation is observed and resources/state are left consistent. Maximum cancellation latency becomes measurable.

### Zombie Work Reaper (ZWR)
Detects work whose execution lease/owner/module/cancellation lineage is no longer valid. It requests shutdown, escalates isolation and records a failure if work ignores bounded cancellation.

### Outcome Reconciliation Token (ORT)
Binds timeout/cancellation with known side-effect state and provider operation identity so S10 UOR can reconcile ambiguous outcomes without conflating them with cancelled local work.

## Cancellation reasons
USER_REQUEST
PARENT_CANCELLED
DEADLINE_EXCEEDED
RESOURCE_REVOKED
MODULE_STOP
KERNEL_SHUTDOWN
POLICY_REVOKED
DEPENDENCY_LOST
SUPERSEDED
FAILURE_CASCADE
Reasons are stable typed values and influence evidence/retry policy.

## Deadline model
Use monotonic time for runtime deadline enforcement. Wall-clock timestamps are evidence metadata, not the source of timeout correctness. Nested operations receive the minimum of inherited deadline and their own allowed bound.

## Cancellation behavior
- stop accepting new child work;
- signal children/adapters;
- reach safe point;
- close/release owned resources;
- preserve committed state/effects;
- mark ambiguous external effects UNKNOWN_OUTCOME;
- emit evidence/result;
- complete bounded cleanup.
Hard process termination is last-resort isolation, not ordinary cancellation.

Resource-accounting cancellation follows the same consistency rule: if cancellation or task destruction occurs while a durable usage write may have committed but before its in-memory lease charge is confirmed, the current governor fails closed and requires a fresh boot to reconcile from S08's ledger. A cancellation result alone does not claim that the durable record was rolled back.

## Shared work/coalescing
Only operations whose contract permits semantic sharing can coalesce. Caller-specific permissions, secrets, side effects or result visibility can prohibit sharing even when payloads look identical.

## Retention
Idempotency records are not retained forever by default. Retention derives from side-effect replay risk, provider guarantees, audit policy and operation semantics. Expiration cannot make a still-dangerous duplicate safe without explicit policy.

## Distributed/external considerations
M00 implements local authority. Provider-native/distributed idempotency is represented by contracts and mappings. Forge never claims cross-system exactly-once solely because local dedup exists.

## State consistency
SIK/IPR updates requiring crash safety use S08 transactions. No lock is held across uncontrolled external work. Intent/status transitions and provider operation IDs allow recovery.

## Event integration
Durable event consumers may use event_id + consumer contract/version as idempotency scope. Replay Safety Token and IPR cooperate so at-least-once delivery does not imply duplicate effects.

## Resource integration
Execution Lease Token and S01 resource leases bind into CLT. Cancellation/revocation releases future budget and prevents child admission. Resource starvation may cause deadline-aware rejection rather than queueing doomed work.

## Performance
- BLAKE3 semantic keys;
- memory fast-path for active keys plus durable backing only when needed;
- sharded/concurrent map or equivalent benchmarked structure for in-flight keys;
- single-flight/coalescing;
- monotonic timer wheel/efficient runtime timers rather than one heavyweight thread per deadline;
- bounded cleanup;
- batch expiry/GC of records;
- benchmark key derivation, duplicate lookup, timer scale, cancellation propagation and coalesced fan-in.

## Security
Idempotency keys must not reveal secrets/raw payloads. Cross-tenant/project key collisions cannot share results. A malicious caller cannot cancel another principal's work without authority. Cancellation reason/source is authenticated where crossing trust boundaries.

## Test strategy
- semantic key canonicalization/property tests;
- concurrent duplicate races;
- crash across FIRST_SEEN/IN_FLIGHT/commit/result recording;
- duplicate during UNKNOWN_OUTCOME;
- provider-native idempotency mapping;
- coalesced waiter cancellation;
- cancellation tree propagation;
- detached-work authorization;
- deadline inheritance/budget exhaustion;
- monotonic clock behavior;
- safe-point maximum latency;
- zombie reaper/nonresponsive adapter;
- event replay dedup;
- tenant/permission isolation;
- 10k+ timer/task scale benchmarks;
- cancellation storms and shutdown tests.

## Anti-pattern gates
Forbidden:
- treating timeout as failure proof;
- cancellation equals rollback;
- random UUID alone as semantic idempotency key;
- global dedup without tenant/project/contract scope;
- infinite idempotency retention by accident;
- child tasks detached implicitly;
- unbounded cancellation cleanup;
- retry after UNKNOWN_OUTCOME without reconciliation;
- sharing coalesced results across incompatible security contexts.

## Acceptance criteria
S12 is accepted when Forge can deduplicate declared semantic intent, safely coalesce eligible in-flight work, propagate bounded cancellation/deadlines through owned task trees, detect zombie work and preserve ambiguous external outcomes for reconciliation without claiming impossible cross-system exactly-once semantics.
