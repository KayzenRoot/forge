# M00-S08 — State Store & Recovery Foundation

Status: CANDIDATE IMPLEMENTATION PRESENT / NOT CERTIFIED
Module: M00 Forge Kernel & Contract Runtime
Depends on: S01-S07

## Mission
Provide Forge with a local-first, transactional, crash-recoverable and auditable operational state foundation that remains usable without HIVE, Core, IRIS, Internet or remote databases.

## State classes
Forge MUST distinguish:
1. CANONICAL_OPERATIONAL — authoritative local runtime/project operational state.
2. EVIDENCE — append-oriented proof/audit records tied to execution fingerprints.
3. CACHE — disposable/rebuildable acceleration state.
4. EPHEMERAL — process/session state that must not be relied upon after restart.
5. EXTERNAL_REFERENCE — identifiers/pointers to state owned by another system, never silently copied into Forge authority.
6. RESOURCE_USAGE — immutable canonical S20 records with stable replay identity and project/work-order/execution/capability/provider attribution.

A value cannot change state class implicitly.

## Baseline technology
SQLite is the primary M00 candidate for local durable structured state because it supports embedded standalone operation, transactions, mature recovery semantics and cross-platform deployment. WAL mode, synchronous policy, checkpoint strategy and SQLx integration are candidates to validate experimentally.

SQLite is not assumed to be the forever solution for every future distributed workload. S08 defines a State Port so later storage engines can be introduced without rewriting domain logic.

## Storage boundaries
- Structured canonical state: relational transactional store.
- Large immutable blobs/artifacts: content-addressed object/file layer when appropriate.
- Cache: separate namespace/tables/files with explicit eviction/rebuild semantics.
- Evidence: append-oriented records plus content fingerprints; immutable artifacts may live in CAS.
- Resource usage: append-only SQLite ledger owned by S08; exact replay is a no-op while the live governor is healthy, and identity/content drift is rejected. The shared pool ceiling is enforced atomically with the durable insert. Native boot applies the live lease charge only after insertion while holding the per-lease accounting gate; an interrupted or ambiguous transition marks the in-memory governor as requiring recovery and blocks new authority until boot replays the ledger.
- Secrets: never ordinary state rows; only secure references/metadata.

## Proprietary technologies

### Forge State Fabric (FSF)
Typed state-access boundary combining ownership, transactions, schema versions, state class and evidence provenance. Modules never open arbitrary shared tables as an informal integration mechanism.

### State Ownership Ledger (SOL)
Machine-readable map of which module owns each schema/entity and which contracts permit other modules to read/change it. Cross-module direct mutation is a conformance failure.

### Crash Consistency Envelope (CCE)
Defines atomic boundaries between state mutation, command identity, evidence emission and recovery marker. After crash, Forge can determine whether work committed, must resume, compensate or be retried safely.

### Recovery Journal Spine (RJS)
Compact durable journal of lifecycle/command transaction milestones needed for deterministic recovery. It is not a duplicate full event-sourcing system.

### State Epoch Capsule (SEC)
Immutable fingerprint describing schema versions, migration level and relevant canonical state epoch used by an execution. Supports replay/evidence without hashing the entire database for every command.

### Content Addressed Evidence Store (CAES)
Immutable evidence/artifact storage keyed by cryptographic content identity. Deduplicates identical evidence and makes tampering/corruption detectable.

### Cache Sovereignty Boundary (CSB)
Enforces the rule that deleting all cache must never destroy canonical correctness. Recovery/certification includes a clean-cache boot test.

### Migration Rehearsal Protocol (MRP)
Before destructive/high-risk schema migration, clone/snapshot representative state, execute migration, validate invariants and recovery/rollback strategy, then permit real migration.

## Transaction model
State mutations are scoped to explicit transactions.
- No transaction spans uncontrolled network/LLM calls.
- External side effects use idempotency/outbox/intent patterns rather than holding DB locks.
- Transaction duration is bounded and observable.
- Busy/lock contention has explicit retry/backoff policy.
- Cancellation cannot leave partially committed domain state.

## Command consistency
For durable commands:
1. validate command/idempotency identity;
2. record intent when required;
3. mutate canonical state atomically;
4. record commit/recovery metadata;
5. dispatch external/event work through safe post-commit mechanism;
6. attach Evidence Contract references.

Exact outbox semantics are refined in S09/S10.

## SQLite operating policy candidates
To benchmark/freeze during implementation:
- WAL for normal local concurrency;
- foreign keys enabled;
- busy timeout/backoff policy;
- integrity checks at defined lifecycle gates, not every request;
- bounded WAL/checkpoint policy;
- explicit synchronous durability profile;
- prepared queries through SQLx;
- migration table/version checks;
- backup API or safe snapshot procedure.
Durability cannot be silently lowered for benchmark vanity.

## Filesystem/CAS
Large evidence/build artifacts should not inflate relational rows unnecessarily. Candidate CAS identity: BLAKE3 digest + size + media/type metadata, with optional Zstd compression where benchmark shows value. Atomic temp-write + fsync/rename strategy must be platform-tested.

## Corruption/recovery
On startup:
- validate store identity/schema compatibility;
- inspect recovery journal/incomplete intents;
- verify critical integrity markers;
- recover/compensate bounded interrupted operations;
- quarantine corrupt optional cache;
- refuse READY if canonical state integrity cannot be established.

Forge never reports READY over known canonical corruption.

## Backup/restore
M00 provides primitives, not full M20 disaster recovery:
- consistent local snapshot;
- manifest with schema/state epoch/fingerprints;
- integrity verification;
- restore into isolated candidate;
- validate before activation.
Backups exclude or separately govern secrets.

## Performance
- prepared/typed queries;
- bounded connection pool appropriate to SQLite;
- batch writes where semantics allow;
- indexes justified by measured query paths;
- avoid N+1 access;
- cache derived read models only when profiling warrants;
- incremental fingerprints/epochs instead of whole-store hashing;
- CAS dedup/compression benchmarks;
- measure p50/p95/p99 read/write/transaction/recovery startup and DB growth.

## Security
- parameterized queries only;
- path traversal/symlink protections for state/CAS roots;
- restrictive filesystem permissions where supported;
- secret values excluded;
- sensitive state classification/redaction;
- evidence tamper detection;
- untrusted imported state validated through S03 contracts;
- database file replacement/rollback attacks considered in threat model.

## Standalone guarantee
With all ecosystem/network services disabled, Forge can create/open state, migrate from supported version, execute a durable local command, crash/restart recovery test, query evidence, clear/rebuild cache and shut down cleanly.

## HIVE/Core/IRIS boundaries
HIVE remains owner of its own knowledge/memory stores. Core and IRIS own their domain state. Forge stores only its canonical operational state plus external references/cache allowed by explicit contracts. Synchronization never creates ambiguous dual authority.

## Integration with previous sections
- S02 State Ownership Ledger maps module ownership.
- S03 schemas/contracts validate persisted records and evidence.
- S04 capability state can persist definitions/evidence while runtime snapshots remain immutable.
- S05 Resolution Proofs can be evidence records.
- S06 graph snapshots/deltas can be cached/persisted by fingerprint.
- S07 ICC/config fingerprints bind migrations and executions.

## Test strategy
- transaction atomicity/property tests;
- abrupt process termination/fault injection;
- WAL/recovery tests;
- idempotent resume tests;
- concurrent reader/writer contention;
- migration forward/failed/rollback-recovery tests;
- canonical-vs-cache deletion tests;
- CAS dedup/corruption tests;
- disk-full/permission/read-only filesystem tests;
- schema downgrade/version-skew tests;
- backup/restore validation;
- Windows/Linux filesystem behavior;
- large-state/long-run growth benchmarks.

## Acceptance criteria
S08 is accepted when Forge has an explicit state taxonomy, local transactional authority, module ownership, crash-consistency/recovery semantics, cache disposability, evidence integrity, migration discipline and a storage abstraction that preserves Sovereign Standalone Mode without pretending SQLite must solve every future distributed problem.
