# M00 state ownership, migrations and recovery

## State ownership

| Store | Authority | Recovery behavior |
|---|---|---|
| `canonical_state` | Durable kernel-owned key/value state with monotonic versions and compare-and-set | Preserved in backup; version conflict rejects stale transitions |
| `command_intents` | Idempotency identity and effect outcome | Committed outcomes replay only recorded safe result data; unknown outcomes are not blindly retried |
| `event_outbox` | Durable event envelopes and delivery state | Ordered pending records can be replayed and acknowledged idempotently |
| `evidence` | Immutable proof/evidence records | Fingerprinted records are stored separately from observations |
| `resource_usage` | Immutable S20 token/cost accounting with owner, pool and project/work-order/execution/capability/provider attribution; insert and shared-pool ceiling are one durable transaction | Stable usage identity makes replay a no-op; conflicting identity reuse is rejected. Live boot applies the matching in-memory lease charge after the durable insert under a per-lease gate. If that transition is interrupted or ambiguous, the governor blocks new authority until a fresh boot replays this ledger. |
| `disposable_cache` | Reusable noncanonical values | May be cleared; cache miss must not affect correctness |

## Migration policy

The SQLite database has a monotonic `PRAGMA user_version`. Schema version 1 is applied in one transaction during open. Schema version 2 adds `resource_usage`; schema version 3 adds command-owner leases, authorization-decision replay/revocation records, and shared resource totals. Opening a v1/v2 store migrates it forward while preserving canonical rows. The runtime rejects a database newer than its supported schema. A migration must have a versioned forward step, an upgrade rehearsal from the previous schema, integrity checks and a documented recovery boundary before its schema version is advanced. No destructive downgrade is provided.

## Backup and restore

1. Call `ForgeStateStore::backup_to` with a new isolated directory. Existing database or manifest paths are rejected.
2. The backup uses SQLite `VACUUM INTO`, then records schema version, hash algorithm and BLAKE3 database digest in `manifest.json`.
3. Restore validates the manifest and database digest before writing into a new state root, opens the candidate, and runs SQLite integrity/schema checks.
4. Keep the original root unchanged until the restored candidate is independently checked by the caller. M00 does not perform destructive in-place replacement or automatic rollback.

## Recovery tests present in the candidate

- Version 1 to version 2 migration and canonical state preservation.
- Resource usage survives reopen, exact replay is deduplicated, conflicting reuse is rejected and boot restores consumed token/cost totals before new leases.
- Deterministic CD3 accounting tests cover cloned-lease delegation contention, cancellation after durable insertion, revocation after durable insertion, persistence failure, conflicting replay, and restart reconciliation. These tests exercise the live/durable accounting boundary; they do not certify external provider billing.
- Compare-and-set conflict for competing transitions.
- Immutable evidence and content-addressed integrity checks.
- Cache deletion independence from canonical state.
- Backup and restore into a new root.
- Tampered backup database rejection.
- Outbox ordering and idempotent event identity.

The command consistency test injects a conflicting outbox identity after the state CAS and proves rollback leaves canonical state and receipt unchanged. Process interruption tests terminate a writer with an open SQLite transaction and verify rollback after reopen. The candidate's exact commit boundary is one SQLite transaction; post-commit outbox publication is recoverable and idempotent. Platform power-loss behavior at the storage device/fsync layer is not established by these process tests.

For C03, a crash-recovery intent stays `in_flight` while its owner instance heartbeat and 60-second lease are live. Only an absent/stale owner or expired lease becomes `unknown_outcome`. Opening a second live store preserves the intent; an unknown external effect is never retried without reconciliation. CAS state, outbox events, and idempotency receipt commit atomically.
