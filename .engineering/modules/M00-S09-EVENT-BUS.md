# M00-S09 — Event Bus

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S08

## Mission
Provide a typed, bounded and observable asynchronous event fabric for facts that have already occurred, supporting local standalone operation, selective durability/replay and safe fan-out without turning Forge into an event-sourcing system by default.

## Semantic law
COMMAND = request/intention for work.
EVENT = immutable fact describing something that already occurred.
QUERY = request for information without semantic side effect.

Consumers MUST NOT disguise required synchronous invariants as eventual events.

## Event classes
1. EPHEMERAL_LOCAL — in-process notification; loss on crash acceptable.
2. DURABLE_LOCAL — persisted before/with publication; recovery/replay required.
3. INTEGRATION — event crossing an adapter boundary; explicit external contract.
4. AUDIT/EVIDENCE — security/governance fact with stronger retention/integrity.
5. TELEMETRY — observational stream; separate loss/backpressure policy.

Each contract declares its class; durability is not inferred at runtime.

## Event envelope
- event_id;
- event_contract_id/version;
- occurred_at logical/monotonic metadata plus wall clock where useful;
- producer module/provider;
- causation_id;
- correlation_id;
- execution/work-order identity;
- payload;
- payload/contract fingerprint;
- ordering key if defined;
- sensitivity classification;
- durability class;
- schema/provenance metadata.

## Proprietary technologies

### Forge Event Fabric (FEF)
Typed event publication/subscription layer using FCF contracts and S02 lifecycle ownership. It is protocol-neutral and local-first.

### Event Lane Architecture (ELA)
Separates traffic into bounded lanes by semantics/risk instead of one giant queue: critical lifecycle, durable domain, normal internal, integration and telemetry. A telemetry flood cannot starve lifecycle/recovery events.

### Causal Event Chain (CEC)
Uses causation/correlation/execution IDs to reconstruct why an event exists without requiring full event sourcing.

### Event Delivery Proof (EDP)
For durable events, records append/publication/consumer checkpoint identities sufficient to prove delivery state and recovery decisions without logging payload secrets.

### Replay Safety Token (RST)
Fingerprint combining event identity, consumer contract/version and replay policy. Prevents accidental replay under incompatible consumer semantics.

### Adaptive Fanout Scheduler (AFS)
Groups subscribers by lane/resource class and dispatches with bounded concurrency/backpressure. Optimization remains deterministic within policy and cannot reorder an ordered partition.

### Event Storm Shield (ESS)
Detects repeated/recursive/high-rate event patterns, applies per-contract budgets/coalescing where semantics permit and opens protective isolation before an event storm destabilizes the kernel.

### Semantic Event Coalescing (SEC)
Only event contracts explicitly declaring coalescing semantics may collapse superseded notifications (e.g. repeated refresh hints). Immutable business/audit facts are never silently coalesced.

## Delivery semantics
Default local durable goal: at-least-once delivery + idempotent consumers where durability is required.
Exactly-once claims are forbidden unless a narrowly scoped mechanism can actually prove them.
Ephemeral events are best-effort within their bounded lane.

## Ordering
Global ordering is avoided. Event contracts may declare:
- UNORDERED;
- PER_KEY ordering;
- PRODUCER ordering;
- strict local sequence where justified.
Parallelism is allowed across independent ordering keys.

## Durable publication
For state-derived durable events, use transactional outbox/commit-intent patterns with S08 so Forge cannot commit canonical state while silently losing the corresponding required event. Exact schema is implementation-stage work.

## Consumer model
Subscriptions declare:
- accepted event contract/range;
- lane;
- ordering requirement;
- idempotency/replay support;
- concurrency limit;
- timeout/deadline;
- retry policy;
- failure/dead-letter policy where applicable;
- sensitivity permission.
Consumer readiness participates in S02 lifecycle.

## Backpressure
All production lanes are bounded. When saturated, policy can:
- block within a deadline;
- shed/coalesce only contracts permitting loss/coalescing;
- spill durable work to S08;
- degrade optional consumers;
- reject publication with explicit error.
Unbounded memory growth is not a strategy.

## Retry/failure
Retry applies only to failures classified retryable and uses bounded backoff/jitter/budget. Poison events are quarantined after policy threshold with evidence. A broken optional subscriber cannot indefinitely block unrelated consumers.

## Replay
Replay is explicit, scoped and evidence-backed:
- select event contract/time/sequence range;
- verify consumer compatibility and RST;
- run dry/shadow mode where side effects are risky;
- enforce idempotency/side-effect policy;
- record replay session evidence.
No "replay everything" button bypasses safety.

## Integration boundaries
HIVE/Core/IRIS/MCP/external brokers connect through adapters. Internal FEF does not require Kafka/NATS/Redis or any remote broker for standalone operation. A broker may later be added when a measured distributed use case requires it.

## Performance
Candidate implementation uses typed in-process channels for ephemeral lanes plus S08-backed durable queues/outbox for durable lanes.
- bounded channels;
- batch durable reads/acks where semantics allow;
- compact internal IDs;
- immutable subscriber snapshots;
- avoid serializing/deserializing typed in-process events unnecessarily;
- lazy external projection;
- single write of durable payload/evidence where possible;
- benchmark throughput, p50/p95/p99 latency, fanout, backpressure and recovery.

No heavyweight broker is admitted into M00 without evidence.

## Security
- FCF validation at trust boundaries;
- subscriber permission checks for sensitive contracts;
- payload size/depth limits;
- redaction-safe diagnostics;
- external events are untrusted until validated;
- no event may expand module permissions;
- audit/evidence events have stronger integrity/retention policy.

## Integration with causality/tests
S06 records PRODUCES/SUBSCRIBES event edges. Event contract changes propagate through Change Cone. Green Proof reuse keys include semantic event contract and consumer fingerprints where relevant.

## Test strategy
- publish/subscribe contract tests;
- ordering property tests;
- bounded queue/backpressure tests;
- at-least-once + idempotent recovery;
- crash between state commit/outbox delivery;
- poison event quarantine;
- consumer timeout/retry budget;
- event storm/recursive loop tests;
- coalescing correctness;
- replay compatibility/RST;
- sensitive event permission/redaction;
- ecosystem adapter outage;
- high fanout and sustained throughput benchmarks;
- telemetry flood isolation from critical lanes;
- deterministic event envelope/fingerprint tests.

## Anti-pattern gates
Forbidden by default:
- untyped string topic soup;
- events used as hidden RPC;
- global total ordering;
- unbounded channels;
- blanket exactly-once claims;
- event sourcing every state mutation;
- network broker required for local boot;
- silent dropping of durable/audit events;
- retry forever.

## Acceptance criteria
S09 is accepted when Forge has a typed local-first event fabric with explicit event semantics, bounded lanes, selective durability, safe recovery/replay, causality, isolation and backpressure, while keeping commands and canonical state ownership distinct.

## C03 outbox transaction rule

When a command changes canonical state and emits durable events, S08 commits the compare-and-set,
outbox rows, and idempotency receipt together. In-process publication happens after that transaction
commits. A crash before commit leaves none of those records; a crash after commit leaves durable
pending outbox rows for idempotent redelivery. Conflicting event identity aborts the entire command
transaction instead of leaving state or a receipt partially committed.
