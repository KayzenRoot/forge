# M00-S10 — Command Bus

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S09

## Mission
Provide Forge with a typed, authorized, cancellable, deadline-aware and evidence-producing command execution fabric for requests that may change state or cause side effects.

## Semantic law
A command expresses intent to perform work. It is not proof that work occurred. Successful completion may emit events/evidence after state/side-effect semantics are satisfied.

## Command envelope
Every command carries:
- command_id;
- command_contract_id/version;
- caller/principal identity;
- target capability constraints;
- correlation/causation/work-order identity;
- idempotency key/strategy when applicable;
- deadline/time budget;
- cancellation lineage;
- risk/side-effect class;
- permission scope;
- resource budget/lease hints;
- config/state/capability snapshot fingerprints;
- payload;
- sensitivity classification;
- evidence requirements.

## Side-effect classes
PURE — deterministic/no externally visible mutation.
LOCAL_MUTATION — Forge-owned local state.
REVERSIBLE_EXTERNAL — external effect with reliable compensation/rollback.
IDEMPOTENT_EXTERNAL — repeat-safe external operation under explicit key/contract.
IRREVERSIBLE_EXTERNAL — cannot reliably undo/repeat.
HIGH_ASSURANCE — money, signing, privileged auth, destructive/security-critical operations.

Retry/failover policy is derived from class, never guessed.

## Proprietary technologies

### Forge Command Fabric (FCmdF)
Typed command dispatch/execution layer integrated with FCF, Capability Resolver, Resource Lease Protocol and lifecycle ownership.

### Command Intent Ledger (CIL)
Durable identity/status record for commands requiring crash-safe deduplication or recovery. Distinguishes RECEIVED, ADMITTED, RUNNING, COMMITTED, FAILED, CANCELLED, UNKNOWN_OUTCOME and COMPENSATED.

### Effect Safety Envelope (ESE)
Binds side-effect class, idempotency, retry/failover/compensation policy and required evidence into one machine-enforced execution policy.

### Execution Lease Token (ELT)
A bounded authorization/resource/lifecycle lease attached to execution. Expiry/cancellation/revocation prevents abandoned work from silently continuing beyond policy.

### Command Admission Gate (CAG)
Before dispatch: validates contract, permissions, deadline, resources, capability eligibility, risk, state/config snapshots and side-effect policy. Rejection occurs before expensive work.

### Unknown Outcome Resolver (UOR)
Handles the dangerous case where Forge loses contact after sending a nontrivial external side effect and cannot know whether it committed. It queries provider status/idempotency evidence or escalates; it MUST NOT blindly retry.

### Command Result Proof (CRP)
Evidence record binding command/input fingerprints, provider, snapshots, attempts, timing, result/error, state/effect commit references and emitted event IDs.

### Hedged Execution Guard (HEG)
Allows speculative/hedged execution only for PURE or explicitly safe idempotent operations. Irreversible/unsafe side effects can never be raced across providers for latency.

## Dispatch pipeline
RECEIVE
-> FCF VALIDATE
-> AUTH/PERMISSION
-> SIDE-EFFECT/RISK CLASSIFY
-> IDEMPOTENCY/DEADLINE/CANCELLATION CHECK
-> S05 CAPABILITY RESOLVE
-> RESOURCE LEASE
-> ADMIT
-> EXECUTE
-> COMMIT/CONFIRM EFFECT
-> RECORD CRP
-> EMIT EVENTS
-> RELEASE LEASE
-> RESULT

Any failure produces a stable S11 error path, not an untyped exception leak.

## Idempotency
Idempotency is contract-defined:
- NONE_ALLOWED: duplicate must be rejected or separately authorized.
- CALLER_KEYED: caller supplies stable semantic key.
- FORGE_DERIVED: deterministic key can safely derive from semantic input.
- PROVIDER_NATIVE: external provider guarantees idempotency under a key.
- STATE_GUARDED: local state transition proves duplicate effect is impossible.
Keys include semantic contract/version scope and expire only under explicit retention policy.

## Deadlines
Deadlines propagate downward and shrink as time is consumed. A child operation cannot claim more time than its parent unless explicitly detached by a separate authorized command. Timeout does not automatically mean external side effect failed; UOR handles ambiguous outcomes.

## Cancellation
Cancellation is cooperative but enforced at owned boundaries:
- cancellation token lineage;
- stop admitting new child work;
- release/cancel leases;
- wait bounded grace period;
- mark non-responsive adapters unhealthy/quarantined if appropriate;
- preserve UNKNOWN_OUTCOME when effect state cannot be proven.
Cancellation is not rollback.

## Authorization
Permission checks occur before capability resolution where possible to avoid leaking provider/resource information. Command handlers declare required permissions via FCF. Modules cannot invoke privileged commands merely because they share a process.

The caller supplies no authority boolean or self-asserted permission list. The trusted host issues a
keyed decision binding decision ID, subject, action/contract, scope, resource, run, permissions,
and validity interval. The command bus verifies the decision before dispatch, limits its lifetime
to five minutes, and atomically consumes its ID in S08; a durable revocation marker rejects a
revoked decision. Missing verifier configuration, malformed/tampered claims, future or expired
validity, replay, revocation, or any subject/action/scope/resource/run mismatch fails closed.
Signing keys remain in the trusted host configuration and are not exposed to request handlers.

Secret-class requests are rejected before durable idempotency recording. Sensitive results retain
their audit fingerprint and commit metadata but persist no raw result payload for replay.

## Local/remote execution
Consumers request capability, not transport. S05 may resolve native module, local worker, ecosystem adapter or remote provider. Command semantics remain stable across transports. Transport-specific failures map to S11 taxonomy.

## Transactions/external effects
No database transaction remains open across uncontrolled external calls. For workflows combining local state + external effects, use intent/outbox/saga/compensation patterns appropriate to the side-effect contract.

## Retries
Retries require:
1. error classified retryable;
2. remaining deadline/budget;
3. safe idempotency/effect policy;
4. healthy eligible provider;
5. retry budget not exhausted.
Backoff uses bounded exponential strategy with jitter where appropriate. No retry forever.

## Parallel/hedged execution
Parallel fanout is allowed for independent PURE work. Hedged requests can reduce tail latency for safe operations after benchmark evidence. Side-effecting commands require explicit semantics; HIGH_ASSURANCE/IRREVERSIBLE work is single-authority by default.

## Performance
- immutable handler/capability snapshots;
- precompiled contract/auth predicates;
- avoid serialization for typed in-process dispatch;
- bounded queues;
- compact command IDs/internal indexes;
- batch safe independent commands where contracts allow;
- cache PURE deterministic results by semantic fingerprint when S14 permits;
- benchmark dispatch overhead, queue saturation, cancellation latency, deadline propagation and durable-intent overhead.

## Standalone behavior
Native commands execute without network/HIVE/Core/IRIS/LLM. External capability absence produces explicit resolution error/degradation. Local durable commands recover through S08 Command Intent Ledger semantics.

## HIVE/Core/IRIS
They are command providers/adapters under FCF and S05. Forge never assumes successful effect merely because request transmission succeeded. Provider-specific operation IDs/status checks may support UOR.

## Test strategy
- command contract validation;
- auth/permission denial;
- idempotency duplicate/property tests;
- crash before/during/after local commit;
- unknown external outcome;
- deadline propagation;
- cancellation races;
- retry budget and non-retryable errors;
- provider failover safety;
- hedged execution guard;
- resource lease exhaustion;
- irreversible/high-assurance no-blind-retry;
- event emission only after proper commit semantics;
- CRP deterministic evidence;
- throughput/p95/p99 dispatch benchmarks;
- malicious/malformed command inputs.

## Anti-pattern gates
Forbidden:
- fire-and-forget side-effect commands without ownership/evidence;
- retrying timeout blindly;
- global mutable command handler map;
- permission by process co-location;
- holding DB transactions over network/LLM calls;
- using events as commands;
- unlimited queues/retries;
- treating cancellation as proof of rollback;
- racing irreversible effects for speed.

## Acceptance criteria
S10 is accepted when Forge commands have explicit contracts, authority, idempotency, deadlines, cancellation, resource admission, side-effect safety, crash/unknown-outcome handling, evidence and provider-neutral dispatch while preserving standalone operation.
