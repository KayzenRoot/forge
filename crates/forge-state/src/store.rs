use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use thiserror::Error;
use tokio::sync::watch;

const SCHEMA_VERSION: i64 = 7;
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS forge_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS canonical_state (owner TEXT NOT NULL, key TEXT NOT NULL, value BLOB NOT NULL, version INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (owner, key))",
    "CREATE TABLE IF NOT EXISTS command_intents (identity TEXT PRIMARY KEY, state TEXT NOT NULL, result BLOB, error_code TEXT, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
    "CREATE TABLE IF NOT EXISTS event_outbox (event_id TEXT PRIMARY KEY, contract_id TEXT NOT NULL, payload BLOB NOT NULL, delivery_state TEXT NOT NULL DEFAULT 'pending', created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
    "CREATE INDEX IF NOT EXISTS event_outbox_pending ON event_outbox(delivery_state, created_at)",
    "CREATE TABLE IF NOT EXISTS evidence (evidence_id TEXT PRIMARY KEY, kind TEXT NOT NULL, fingerprint TEXT NOT NULL, payload BLOB NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
    "CREATE TABLE IF NOT EXISTS disposable_cache (namespace TEXT NOT NULL, cache_key TEXT NOT NULL, fingerprint TEXT NOT NULL, value BLOB NOT NULL, PRIMARY KEY (namespace, cache_key))",
];
const RESOURCE_USAGE_MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS resource_usage (usage_id TEXT PRIMARY KEY, pool TEXT NOT NULL CHECK(pool IN ('ordinary', 'survival')), tokens INTEGER NOT NULL CHECK(tokens >= 0), cost_micros INTEGER NOT NULL CHECK(cost_micros >= 0), payload BLOB NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
    "CREATE INDEX IF NOT EXISTS resource_usage_pool ON resource_usage(pool, created_at, usage_id)",
];
const CORRECTION_MIGRATIONS: &[&str] = &[
    "ALTER TABLE command_intents ADD COLUMN owner_instance_id TEXT",
    "ALTER TABLE command_intents ADD COLUMN lease_expires_at_ms INTEGER",
    "CREATE TABLE IF NOT EXISTS store_instances (instance_id TEXT PRIMARY KEY, heartbeat_at_ms INTEGER NOT NULL)",
    "CREATE INDEX IF NOT EXISTS store_instances_heartbeat ON store_instances(heartbeat_at_ms)",
    "CREATE TABLE IF NOT EXISTS resource_usage_totals (pool TEXT PRIMARY KEY CHECK(pool IN ('ordinary', 'survival')), tokens INTEGER NOT NULL CHECK(tokens >= 0), cost_micros INTEGER NOT NULL CHECK(cost_micros >= 0), record_count INTEGER NOT NULL CHECK(record_count >= 0))",
    "INSERT OR IGNORE INTO resource_usage_totals(pool, tokens, cost_micros, record_count) SELECT pool, SUM(tokens), SUM(cost_micros), COUNT(*) FROM resource_usage GROUP BY pool",
    "INSERT OR IGNORE INTO resource_usage_totals(pool, tokens, cost_micros, record_count) VALUES('ordinary', 0, 0, 0), ('survival', 0, 0, 0)",
    "CREATE TABLE IF NOT EXISTS authorization_decisions (decision_id TEXT PRIMARY KEY, claims_fingerprint TEXT NOT NULL, consumed_at_ms INTEGER, revoked_at_ms INTEGER)",
];
const OUTCOME_RESOLUTION_MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS command_outcome_resolutions (resolution_id INTEGER PRIMARY KEY AUTOINCREMENT, identity TEXT NOT NULL REFERENCES command_intents(identity), decision_id TEXT NOT NULL UNIQUE, outcome TEXT NOT NULL CHECK(outcome IN ('effect_committed', 'effect_not_committed', 'inconclusive')), evidence_fingerprint TEXT NOT NULL, decision_fingerprint TEXT NOT NULL, resolved_at_ms INTEGER NOT NULL CHECK(resolved_at_ms >= 0))",
    "CREATE INDEX IF NOT EXISTS command_outcome_resolutions_identity ON command_outcome_resolutions(identity, resolution_id DESC)",
];
const EVENT_DELIVERY_MIGRATIONS: &[&str] = &[
    "ALTER TABLE event_outbox ADD COLUMN producer_id TEXT NOT NULL DEFAULT 'legacy'",
    "ALTER TABLE event_outbox ADD COLUMN producer_sequence INTEGER NOT NULL DEFAULT 0",
    "UPDATE event_outbox SET producer_sequence=rowid WHERE producer_sequence=0",
    "CREATE UNIQUE INDEX IF NOT EXISTS event_outbox_producer_order ON event_outbox(producer_id, producer_sequence)",
    "CREATE TABLE IF NOT EXISTS event_consumer_state (event_id TEXT NOT NULL REFERENCES event_outbox(event_id), consumer_id TEXT NOT NULL, delivery_attempts INTEGER NOT NULL DEFAULT 0 CHECK(delivery_attempts >= 0), acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0, 1)), quarantined INTEGER NOT NULL DEFAULT 0 CHECK(quarantined IN (0, 1)), last_error_code TEXT, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY(event_id, consumer_id))",
    "CREATE INDEX IF NOT EXISTS event_consumer_pending ON event_consumer_state(consumer_id, acknowledged, quarantined, event_id)",
];
const EVENT_POISON_QUARANTINE_THRESHOLD: i64 = 3;
const EFFECT_FINALIZATION_MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS command_finalizations (identity TEXT PRIMARY KEY REFERENCES command_intents(identity), payload BLOB NOT NULL, fingerprint TEXT NOT NULL, staged_at_ms INTEGER NOT NULL CHECK(staged_at_ms >= 0), finalized_at_ms INTEGER)",
    "UPDATE command_intents SET state='effect_committed_pending_finalization', result=NULL, error_code='FORGE.COMMAND.EFFECT_COMMITTED_REQUIRES_FINALIZATION', owner_instance_id=NULL, lease_expires_at_ms=NULL WHERE state<>'committed' AND EXISTS(SELECT 1 FROM command_outcome_resolutions WHERE command_outcome_resolutions.identity=command_intents.identity AND outcome='effect_committed')",
];
const EFFECT_FINALIZATION_REVISION_MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS command_finalization_revisions (revision_id INTEGER PRIMARY KEY AUTOINCREMENT, identity TEXT NOT NULL REFERENCES command_finalizations(identity), prior_fingerprint TEXT NOT NULL, revised_fingerprint TEXT NOT NULL, state_transition BLOB NOT NULL, decision_id TEXT NOT NULL UNIQUE REFERENCES authorization_decisions(decision_id), decision_fingerprint TEXT NOT NULL, revised_at_ms INTEGER NOT NULL CHECK(revised_at_ms >= 0))",
    "CREATE INDEX IF NOT EXISTS command_finalization_revisions_identity ON command_finalization_revisions(identity, revision_id)",
];

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static STORE_INSTANCE_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const OWNER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const OWNER_LEASE_MS: i64 = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateClass {
    CanonicalOperational,
    Evidence,
    Cache,
    Ephemeral,
    ExternalReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyState {
    InFlight,
    Committed,
    FailedRetryable,
    FailedFinal,
    UnknownOutcome,
    EffectCommittedPendingFinalization,
}

impl IdempotencyState {
    fn as_db(self) -> &'static str {
        match self {
            Self::InFlight => "in_flight",
            Self::Committed => "committed",
            Self::FailedRetryable => "failed_retryable",
            Self::FailedFinal => "failed_final",
            Self::UnknownOutcome => "unknown_outcome",
            Self::EffectCommittedPendingFinalization => "effect_committed_pending_finalization",
        }
    }

    fn from_db(value: &str) -> Result<Self, StateError> {
        match value {
            "in_flight" => Ok(Self::InFlight),
            "committed" => Ok(Self::Committed),
            "failed_retryable" => Ok(Self::FailedRetryable),
            "failed_final" => Ok(Self::FailedFinal),
            "unknown_outcome" => Ok(Self::UnknownOutcome),
            "effect_committed_pending_finalization" => Ok(Self::EffectCommittedPendingFinalization),
            _ => Err(StateError::Integrity),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdempotencyRecord {
    pub state: IdempotencyState,
    pub result: Option<Value>,
    pub error_code: Option<String>,
    pub latest_resolution: Option<CommandOutcomeResolutionRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOutcomeResolutionKind {
    EffectCommitted,
    EffectNotCommitted,
    Inconclusive,
}

impl CommandOutcomeResolutionKind {
    fn as_db(self) -> &'static str {
        match self {
            Self::EffectCommitted => "effect_committed",
            Self::EffectNotCommitted => "effect_not_committed",
            Self::Inconclusive => "inconclusive",
        }
    }

    fn from_db(value: &str) -> Result<Self, StateError> {
        match value {
            "effect_committed" => Ok(Self::EffectCommitted),
            "effect_not_committed" => Ok(Self::EffectNotCommitted),
            "inconclusive" => Ok(Self::Inconclusive),
            _ => Err(StateError::Integrity),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommandOutcomeResolutionRecord {
    pub kind: CommandOutcomeResolutionKind,
    pub evidence_fingerprint: String,
    pub decision_fingerprint: String,
    pub decision_id: String,
    pub resolved_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub evidence_id: String,
    pub kind: String,
    pub fingerprint: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoredEvent {
    pub event_id: String,
    pub contract_id: String,
    pub payload: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingEventRecord {
    pub event_id: String,
    pub contract_id: String,
    pub payload_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CanonicalStateTransition {
    pub owner: String,
    pub key: String,
    pub expected_version: Option<u64>,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceUsagePool {
    Ordinary,
    Survival,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedResourceVector {
    pub cpu_millis: u64,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub network_bytes: u64,
    pub tokens: u64,
    pub cost_micros: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceUsageAttribution {
    pub project_id: String,
    pub work_order_id: String,
    pub execution_id: String,
    pub capability_id: String,
    pub provider_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersistedResourceUsageRecord {
    pub usage_id: String,
    pub pool: ResourceUsagePool,
    pub owner: String,
    pub category: String,
    pub used: PersistedResourceVector,
    pub cache_tokens_reused: u64,
    pub attribution: ResourceUsageAttribution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupCasObject {
    digest: String,
    size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupManifestUnsigned {
    schema_version: u16,
    database_schema_version: i64,
    database_fingerprint: String,
    database_hash_algorithm: String,
    cas_objects: Vec<BackupCasObject>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupManifest {
    schema_version: u16,
    database_schema_version: i64,
    database_fingerprint: String,
    database_hash_algorithm: String,
    cas_objects: Vec<BackupCasObject>,
    backup_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct PendingEffectFinalization {
    result: Value,
    state_transition: Option<CanonicalStateTransition>,
    events: Vec<StoredEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectFinalizationRecord {
    pub fingerprint: String,
    pub result: Value,
    pub state_transition: Option<CanonicalStateTransition>,
    pub events: Vec<StoredEvent>,
    pub finalized_at_ms: Option<i64>,
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("state storage operation failed")]
    Storage(#[source] sqlx::Error),
    #[error("filesystem operation failed")]
    Filesystem(#[from] std::io::Error),
    #[error("state value could not be encoded or decoded")]
    Serialization(#[source] serde_json::Error),
    #[error("state owner or key is invalid")]
    InvalidKey,
    #[error("state root could not be resolved")]
    InvalidRoot,
    #[error("state or evidence integrity check failed")]
    Integrity,
    #[error("idempotency identity is invalid")]
    InvalidIdempotencyIdentity,
    #[error("evidence identity conflicts with existing immutable evidence")]
    EvidenceConflict,
    #[error("resource usage identity conflicts with an existing immutable record")]
    ResourceUsageConflict,
    #[error("resource usage would exceed its durable token or cost ceiling")]
    ResourceBudgetExceeded,
    #[error("requested state does not exist")]
    NotFound,
    #[error("state transition is not valid from the stored state")]
    StateConflict,
    #[error("backup target must be new and isolated")]
    BackupTargetExists,
    #[error("payload contains secret material or secret-bearing fields")]
    SensitivePayload,
    #[error("resource usage ledger reached its shared record limit")]
    ResourceLedgerLimit,
    #[error("host authorization decision is expired, revoked, or has already been used")]
    AuthorizationDecisionRejected,
    #[error("this store instance no longer owns a live command lease")]
    OwnerLeaseLost,
}

#[derive(Clone)]
pub struct ForgeStateStore {
    root: PathBuf,
    pool: SqlitePool,
    owner: Arc<StoreOwner>,
}

struct StoreOwner {
    instance_id: String,
    healthy: Arc<AtomicBool>,
    _shutdown: watch::Sender<bool>,
}

impl ForgeStateStore {
    pub async fn open(root: impl AsRef<Path>) -> Result<Self, StateError> {
        fs::create_dir_all(root.as_ref()).map_err(StateError::Filesystem)?;
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|_| StateError::InvalidRoot)?;
        let database = root.join("forge.sqlite3");
        reject_symlink_if_present(&database)?;
        for sidecar in [
            "forge.sqlite3-wal",
            "forge.sqlite3-shm",
            "forge.sqlite3-journal",
        ] {
            reject_symlink_if_present(&root.join(sidecar))?;
        }

        let options = SqliteConnectOptions::new()
            .filename(&database)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(6))
            .connect_with(options)
            .await
            .map_err(StateError::Storage)?;
        let instance_id = new_store_instance_id();
        let (shutdown, receiver) = watch::channel(false);
        let healthy = Arc::new(AtomicBool::new(true));
        let store = Self {
            root,
            pool,
            owner: Arc::new(StoreOwner {
                instance_id,
                healthy: Arc::clone(&healthy),
                _shutdown: shutdown,
            }),
        };
        store.migrate().await?;
        store.integrity_check().await?;
        store.register_store_instance().await?;
        store.mark_interrupted_commands_unknown().await?;
        store.start_owner_heartbeat(receiver, healthy);
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn state_class(&self) -> StateClass {
        StateClass::CanonicalOperational
    }

    pub fn schema_version(&self) -> u16 {
        SCHEMA_VERSION as u16
    }

    fn ensure_owner_healthy(&self) -> Result<(), StateError> {
        if self.owner.healthy.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(StateError::OwnerLeaseLost)
        }
    }

    async fn register_store_instance(&self) -> Result<(), StateError> {
        sqlx::query("INSERT INTO store_instances(instance_id, heartbeat_at_ms) VALUES(?1, ?2)")
            .bind(&self.owner.instance_id)
            .bind(unix_time_ms()?)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(())
    }

    fn start_owner_heartbeat(&self, mut shutdown: watch::Receiver<bool>, healthy: Arc<AtomicBool>) {
        let pool = self.pool.clone();
        let instance_id = self.owner.instance_id.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            break;
                        }
                    }
                    () = tokio::time::sleep(OWNER_HEARTBEAT_INTERVAL) => {
                        let now = match unix_time_ms() {
                            Ok(value) => value,
                            Err(_) => {
                                healthy.store(false, Ordering::Release);
                                break;
                            }
                        };
                        let heartbeat = sqlx::query("UPDATE store_instances SET heartbeat_at_ms=?2 WHERE instance_id=?1")
                            .bind(&instance_id)
                            .bind(now)
                            .execute(&pool)
                            .await;
                        match heartbeat {
                            Ok(result) if result.rows_affected() == 1 => {}
                            _ => {
                                healthy.store(false, Ordering::Release);
                                break;
                            }
                        }
                        let lease_expires_at = now.saturating_add(OWNER_LEASE_MS);
                        if sqlx::query("UPDATE command_intents SET lease_expires_at_ms=?2 WHERE state='in_flight' AND owner_instance_id=?1")
                            .bind(&instance_id)
                            .bind(lease_expires_at)
                            .execute(&pool)
                            .await
                            .is_err()
                        {
                            healthy.store(false, Ordering::Release);
                            break;
                        }
                    }
                }
            }
            let _ = sqlx::query("DELETE FROM store_instances WHERE instance_id=?1")
                .bind(&instance_id)
                .execute(&pool)
                .await;
        });
    }

    pub async fn record_resource_usage(
        &self,
        record: &PersistedResourceUsageRecord,
        pool_limit: &PersistedResourceVector,
    ) -> Result<bool, StateError> {
        validate_resource_usage_record(record)?;
        ensure_safe_payload(&serde_json::to_value(record).map_err(StateError::Serialization)?)?;
        let payload = serde_json::to_vec(record).map_err(StateError::Serialization)?;
        let pool = match record.pool {
            ResourceUsagePool::Ordinary => "ordinary",
            ResourceUsagePool::Survival => "survival",
        };
        let tokens =
            i64::try_from(record.used.tokens).map_err(|_| StateError::ResourceBudgetExceeded)?;
        let cost_micros = i64::try_from(record.used.cost_micros)
            .map_err(|_| StateError::ResourceBudgetExceeded)?;
        let token_limit =
            i64::try_from(pool_limit.tokens).map_err(|_| StateError::ResourceBudgetExceeded)?;
        let cost_limit = i64::try_from(pool_limit.cost_micros)
            .map_err(|_| StateError::ResourceBudgetExceeded)?;
        // Acquire SQLite's writer reservation before reading. A deferred transaction lets two
        // writers both read the same totals and then fail while upgrading their locks.
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(StateError::Storage)?;
        if let Some(existing) = sqlx::query("SELECT payload FROM resource_usage WHERE usage_id=?1")
            .bind(&record.usage_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(StateError::Storage)?
        {
            let existing_payload: Vec<u8> =
                existing.try_get("payload").map_err(StateError::Storage)?;
            transaction.rollback().await.map_err(StateError::Storage)?;
            return if existing_payload == payload {
                Ok(false)
            } else {
                Err(StateError::ResourceUsageConflict)
            };
        }

        let reserved = sqlx::query("UPDATE resource_usage_totals SET tokens=tokens+?2, cost_micros=cost_micros+?3, record_count=record_count+1 WHERE pool=?1 AND record_count < 100000 AND (SELECT COALESCE(SUM(record_count), 0) FROM resource_usage_totals) < 100000 AND tokens <= ?4 - ?2 AND cost_micros <= ?5 - ?3")
            .bind(pool)
            .bind(tokens)
            .bind(cost_micros)
            .bind(token_limit)
            .bind(cost_limit)
            .execute(&mut *transaction)
            .await
            .map_err(StateError::Storage)?
            .rows_affected();
        if reserved != 1 {
            let row = sqlx::query(
                "SELECT COALESCE(SUM(record_count), 0) AS total FROM resource_usage_totals",
            )
            .fetch_one(&mut *transaction)
            .await
            .map_err(StateError::Storage)?;
            let total: i64 = row.try_get("total").map_err(StateError::Storage)?;
            transaction.rollback().await.map_err(StateError::Storage)?;
            return if total >= 100_000 {
                Err(StateError::ResourceLedgerLimit)
            } else {
                Err(StateError::ResourceBudgetExceeded)
            };
        }
        sqlx::query("INSERT INTO resource_usage(usage_id, pool, tokens, cost_micros, payload) VALUES(?1, ?2, ?3, ?4, ?5)")
            .bind(&record.usage_id)
            .bind(pool)
            .bind(tokens)
            .bind(cost_micros)
            .bind(&payload)
            .execute(&mut *transaction)
            .await
            .map_err(StateError::Storage)?;
        transaction.commit().await.map_err(StateError::Storage)?;
        Ok(true)
    }

    pub async fn get_resource_usage_record(
        &self,
        usage_id: &str,
    ) -> Result<Option<PersistedResourceUsageRecord>, StateError> {
        validate_identity(usage_id)?;
        let row = sqlx::query("SELECT payload FROM resource_usage WHERE usage_id=?1")
            .bind(usage_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        row.map(|row| {
            let payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
            let record: PersistedResourceUsageRecord =
                serde_json::from_slice(&payload).map_err(StateError::Serialization)?;
            validate_resource_usage_record(&record)?;
            if record.usage_id != usage_id {
                return Err(StateError::Integrity);
            }
            Ok(record)
        })
        .transpose()
    }

    pub async fn resource_usage_records(
        &self,
    ) -> Result<Vec<PersistedResourceUsageRecord>, StateError> {
        let rows = sqlx::query("SELECT payload FROM resource_usage ORDER BY created_at, usage_id")
            .fetch_all(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        rows.into_iter()
            .map(|row| {
                let payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
                let record: PersistedResourceUsageRecord =
                    serde_json::from_slice(&payload).map_err(StateError::Serialization)?;
                validate_resource_usage_record(&record)?;
                Ok(record)
            })
            .collect()
    }

    pub async fn put_canonical(
        &self,
        owner: &str,
        key: &str,
        value: &Value,
    ) -> Result<(), StateError> {
        validate_key(owner)?;
        validate_key(key)?;
        ensure_safe_payload(value)?;
        let bytes = serde_json::to_vec(value).map_err(StateError::Serialization)?;
        sqlx::query(
            "INSERT INTO canonical_state(owner, key, value, version) VALUES(?1, ?2, ?3, 1) \
             ON CONFLICT(owner, key) DO UPDATE SET value=excluded.value, version=canonical_state.version+1, updated_at=CURRENT_TIMESTAMP",
        )
        .bind(owner)
        .bind(key)
        .bind(bytes)
        .execute(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        Ok(())
    }

    pub async fn get_canonical(&self, owner: &str, key: &str) -> Result<Option<Value>, StateError> {
        validate_key(owner)?;
        validate_key(key)?;
        let row = sqlx::query("SELECT value FROM canonical_state WHERE owner=?1 AND key=?2")
            .bind(owner)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        row.map(|record| {
            let bytes: Vec<u8> = record.try_get("value").map_err(StateError::Storage)?;
            serde_json::from_slice(&bytes).map_err(StateError::Serialization)
        })
        .transpose()
    }

    pub async fn delete_canonical(&self, owner: &str, key: &str) -> Result<bool, StateError> {
        validate_key(owner)?;
        validate_key(key)?;
        let result = sqlx::query("DELETE FROM canonical_state WHERE owner=?1 AND key=?2")
            .bind(owner)
            .bind(key)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn compare_and_set_canonical(
        &self,
        owner: &str,
        key: &str,
        expected_version: Option<u64>,
        value: &Value,
    ) -> Result<bool, StateError> {
        validate_key(owner)?;
        validate_key(key)?;
        ensure_safe_payload(value)?;
        let bytes = serde_json::to_vec(value).map_err(StateError::Serialization)?;
        let result = if let Some(expected_version) = expected_version {
            if expected_version == 0 || expected_version > i64::MAX as u64 {
                return Err(StateError::StateConflict);
            }
            sqlx::query(
                "UPDATE canonical_state SET value=?3, version=version+1, updated_at=CURRENT_TIMESTAMP WHERE owner=?1 AND key=?2 AND version=?4",
            )
            .bind(owner)
            .bind(key)
            .bind(bytes)
            .bind(expected_version as i64)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?
        } else {
            sqlx::query(
                "INSERT INTO canonical_state(owner, key, value, version) VALUES(?1, ?2, ?3, 1) ON CONFLICT(owner, key) DO NOTHING",
            )
            .bind(owner)
            .bind(key)
            .bind(bytes)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?
        };
        Ok(result.rows_affected() == 1)
    }

    async fn insert_outbox_event(
        transaction: &mut Transaction<'_, Sqlite>,
        event: &StoredEvent,
    ) -> Result<(), StateError> {
        validate_key(&event.event_id)?;
        validate_key(&event.contract_id)?;
        ensure_safe_payload(&event.payload)?;
        let producer_id = event
            .payload
            .get("producer")
            .and_then(Value::as_str)
            .unwrap_or("legacy");
        validate_key(producer_id)?;
        let payload = serde_json::to_vec(&event.payload).map_err(StateError::Serialization)?;

        if let Some(row) = sqlx::query(
            "SELECT contract_id, producer_id, payload FROM event_outbox WHERE event_id=?1",
        )
        .bind(&event.event_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(StateError::Storage)?
        {
            let contract_id: String = row.try_get("contract_id").map_err(StateError::Storage)?;
            let existing_producer: String =
                row.try_get("producer_id").map_err(StateError::Storage)?;
            let prior_payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
            if contract_id == event.contract_id
                && existing_producer == producer_id
                && prior_payload == payload
            {
                return Ok(());
            }
            return Err(StateError::StateConflict);
        }

        let sequence_row = sqlx::query(
            "SELECT COALESCE(MAX(producer_sequence), 0) + 1 AS next_sequence FROM event_outbox WHERE producer_id=?1",
        )
        .bind(producer_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(StateError::Storage)?;
        let producer_sequence: i64 = sequence_row
            .try_get("next_sequence")
            .map_err(StateError::Storage)?;
        if producer_sequence <= 0 {
            return Err(StateError::Integrity);
        }
        sqlx::query(
            "INSERT INTO event_outbox(event_id, contract_id, payload, delivery_state, producer_id, producer_sequence) VALUES(?1, ?2, ?3, 'pending', ?4, ?5)",
        )
        .bind(&event.event_id)
        .bind(&event.contract_id)
        .bind(payload)
        .bind(producer_id)
        .bind(producer_sequence)
        .execute(&mut **transaction)
        .await
        .map_err(StateError::Storage)?;
        Ok(())
    }

    /// Atomically commits a command receipt with its optional canonical state transition and
    /// durable events. A crash can therefore leave either the complete commit or none of it.
    pub async fn commit_command(
        &self,
        identity: &str,
        result: &Value,
        state_transition: Option<&CanonicalStateTransition>,
        events: &[StoredEvent],
    ) -> Result<(), StateError> {
        self.ensure_owner_healthy()?;
        validate_identity(identity)?;
        ensure_safe_payload(result)?;
        let result_bytes = serde_json::to_vec(result).map_err(StateError::Serialization)?;
        if result_bytes.len() > 4_194_304 || events.len() > 1_024 {
            return Err(StateError::Integrity);
        }

        let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
        if let Some(transition) = state_transition {
            validate_key(&transition.owner)?;
            validate_key(&transition.key)?;
            ensure_safe_payload(&transition.value)?;
            let value = serde_json::to_vec(&transition.value).map_err(StateError::Serialization)?;
            let updated = if let Some(expected_version) = transition.expected_version {
                if expected_version == 0 || expected_version > i64::MAX as u64 {
                    return Err(StateError::StateConflict);
                }
                sqlx::query("UPDATE canonical_state SET value=?3, version=version+1, updated_at=CURRENT_TIMESTAMP WHERE owner=?1 AND key=?2 AND version=?4")
                    .bind(&transition.owner)
                    .bind(&transition.key)
                    .bind(value)
                    .bind(expected_version as i64)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?
            } else {
                sqlx::query("INSERT INTO canonical_state(owner, key, value, version) VALUES(?1, ?2, ?3, 1) ON CONFLICT(owner, key) DO NOTHING")
                    .bind(&transition.owner)
                    .bind(&transition.key)
                    .bind(value)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?
            };
            if updated.rows_affected() != 1 {
                return Err(StateError::StateConflict);
            }
            #[cfg(test)]
            crash_commit_process_at("state_cas");
        }

        #[cfg(test)]
        let mut event_index = 0usize;
        for event in events {
            Self::insert_outbox_event(&mut transaction, event).await?;
            #[cfg(test)]
            {
                event_index += 1;
                crash_commit_process_at(&format!("outbox_{event_index}"));
            }
        }

        let update = sqlx::query("UPDATE command_intents SET state='committed', result=?2, error_code=NULL, owner_instance_id=NULL, lease_expires_at_ms=NULL, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='in_flight' AND owner_instance_id=?3")
            .bind(identity)
            .bind(result_bytes)
            .bind(&self.owner.instance_id)
            .execute(&mut *transaction)
            .await
            .map_err(StateError::Storage)?;
        if update.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        #[cfg(test)]
        crash_commit_process_at("receipt");
        transaction.commit().await.map_err(StateError::Storage)?;
        #[cfg(test)]
        crash_commit_process_at("after_commit");
        Ok(())
    }

    /// Stages already-confirmed external work for local finalization without making it retryable.
    /// The result, local CAS, durable outbox and receipt are content-bound and persisted before
    /// finalization is attempted, so a crash can resume the local half without re-running the effect.
    pub async fn stage_effect_finalization(
        &self,
        identity: &str,
        result: &Value,
        state_transition: Option<&CanonicalStateTransition>,
        events: &[StoredEvent],
    ) -> Result<(), StateError> {
        self.ensure_owner_healthy()?;
        validate_identity(identity)?;
        ensure_safe_payload(result)?;
        let evidence = result
            .get("_forgeCommandReceipt")
            .and_then(|receipt| receipt.get("commitEvidenceFingerprint"))
            .and_then(Value::as_str)
            .ok_or(StateError::Integrity)?;
        validate_digest(evidence)?;
        if let Some(transition) = state_transition {
            validate_key(&transition.owner)?;
            validate_key(&transition.key)?;
            ensure_safe_payload(&transition.value)?;
        }
        if events.len() > 1_024 {
            return Err(StateError::Integrity);
        }
        for event in events {
            validate_key(&event.event_id)?;
            validate_key(&event.contract_id)?;
            ensure_safe_payload(&event.payload)?;
            validate_key(
                event
                    .payload
                    .get("producer")
                    .and_then(Value::as_str)
                    .unwrap_or("legacy"),
            )?;
        }
        let staged = PendingEffectFinalization {
            result: result.clone(),
            state_transition: state_transition.cloned(),
            events: events.to_vec(),
        };
        let payload = serde_json::to_vec(&staged).map_err(StateError::Serialization)?;
        if payload.len() > 16_777_216 {
            return Err(StateError::Integrity);
        }
        let fingerprint = blake3::hash(&payload).to_hex().to_string();
        let staged_at_ms = unix_time_ms()?;
        let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
        let existing =
            sqlx::query("SELECT payload, fingerprint FROM command_finalizations WHERE identity=?1")
                .bind(identity)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
        if let Some(row) = existing {
            let existing_payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
            let existing_fingerprint: String =
                row.try_get("fingerprint").map_err(StateError::Storage)?;
            transaction.rollback().await.map_err(StateError::Storage)?;
            return if existing_payload == payload && existing_fingerprint == fingerprint {
                Ok(())
            } else {
                Err(StateError::StateConflict)
            };
        }

        let intent =
            sqlx::query("SELECT state, owner_instance_id FROM command_intents WHERE identity=?1")
                .bind(identity)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(StateError::Storage)?
                .ok_or(StateError::StateConflict)?;
        let state: String = intent.try_get("state").map_err(StateError::Storage)?;
        let owner: Option<String> = intent
            .try_get("owner_instance_id")
            .map_err(StateError::Storage)?;
        let owned_in_flight =
            state == "in_flight" && owner.as_deref() == Some(self.owner.instance_id.as_str());
        let resolved_committed = matches!(
            state.as_str(),
            "unknown_outcome" | "effect_committed_pending_finalization"
        ) && sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM command_outcome_resolutions WHERE identity=?1 AND outcome='effect_committed')",
        )
        .bind(identity)
        .fetch_one(&mut *transaction)
        .await
        .map_err(StateError::Storage)?
            == 1;
        if !owned_in_flight && !resolved_committed {
            return Err(StateError::StateConflict);
        }
        if owned_in_flight {
            let updated = sqlx::query(
                "UPDATE command_intents SET state='effect_committed_pending_finalization', result=NULL, error_code='FORGE.COMMAND.EFFECT_COMMITTED_REQUIRES_FINALIZATION', owner_instance_id=NULL, lease_expires_at_ms=NULL, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='in_flight' AND owner_instance_id=?2",
            )
            .bind(identity)
            .bind(&self.owner.instance_id)
            .execute(&mut *transaction)
            .await
            .map_err(StateError::Storage)?;
            if updated.rows_affected() != 1 {
                return Err(StateError::StateConflict);
            }
        }
        sqlx::query(
            "INSERT INTO command_finalizations(identity, payload, fingerprint, staged_at_ms, finalized_at_ms) VALUES(?1, ?2, ?3, ?4, NULL)",
        )
        .bind(identity)
        .bind(payload)
        .bind(fingerprint)
        .bind(staged_at_ms)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        transaction.commit().await.map_err(StateError::Storage)?;
        Ok(())
    }

    /// Loads the durable finalization envelope after checking its content fingerprint.
    pub async fn effect_finalization(
        &self,
        identity: &str,
    ) -> Result<Option<EffectFinalizationRecord>, StateError> {
        validate_identity(identity)?;
        let row = sqlx::query(
            "SELECT payload, fingerprint, finalized_at_ms FROM command_finalizations WHERE identity=?1",
        )
        .bind(identity)
        .fetch_optional(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        row.map(|row| {
            let payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
            let fingerprint: String = row.try_get("fingerprint").map_err(StateError::Storage)?;
            let finalized_at_ms: Option<i64> = row
                .try_get("finalized_at_ms")
                .map_err(StateError::Storage)?;
            if blake3::hash(&payload).to_hex().as_str() != fingerprint {
                return Err(StateError::Integrity);
            }
            let staged: PendingEffectFinalization =
                serde_json::from_slice(&payload).map_err(StateError::Serialization)?;
            ensure_safe_payload(&staged.result)?;
            if let Some(transition) = staged.state_transition.as_ref() {
                validate_key(&transition.owner)?;
                validate_key(&transition.key)?;
                ensure_safe_payload(&transition.value)?;
            }
            for event in &staged.events {
                validate_key(&event.event_id)?;
                validate_key(&event.contract_id)?;
                ensure_safe_payload(&event.payload)?;
            }
            Ok(EffectFinalizationRecord {
                fingerprint,
                result: staged.result,
                state_transition: staged.state_transition,
                events: staged.events,
                finalized_at_ms,
            })
        })
        .transpose()
    }

    /// Replaces only the local CAS transition of a confirmed effect, bound to the exact
    /// prior envelope fingerprint and a host-verified, single-use decision. The caller
    /// must verify the host signature before entering this state-layer method.
    pub async fn revise_effect_finalization(
        &self,
        identity: &str,
        expected_fingerprint: &str,
        state_transition: Option<&CanonicalStateTransition>,
        decision_id: &str,
        decision_fingerprint: &str,
        revised_at_ms: u64,
    ) -> Result<String, StateError> {
        self.ensure_owner_healthy()?;
        validate_identity(identity)?;
        validate_digest(expected_fingerprint)?;
        validate_key(decision_id)?;
        validate_digest(decision_fingerprint)?;
        let revised_at_ms = i64::try_from(revised_at_ms).map_err(|_| StateError::Integrity)?;
        if let Some(transition) = state_transition {
            validate_key(&transition.owner)?;
            validate_key(&transition.key)?;
            ensure_safe_payload(&transition.value)?;
        }

        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(StateError::Storage)?;
        let row = sqlx::query(
            "SELECT payload, fingerprint, finalized_at_ms FROM command_finalizations WHERE identity=?1",
        )
        .bind(identity)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(StateError::Storage)?
        .ok_or(StateError::StateConflict)?;
        let payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
        let prior_fingerprint: String = row.try_get("fingerprint").map_err(StateError::Storage)?;
        let finalized_at_ms: Option<i64> = row
            .try_get("finalized_at_ms")
            .map_err(StateError::Storage)?;
        if blake3::hash(&payload).to_hex().as_str() != prior_fingerprint {
            return Err(StateError::Integrity);
        }
        if finalized_at_ms.is_some() || prior_fingerprint != expected_fingerprint {
            return Err(StateError::StateConflict);
        }
        let state: Option<String> =
            sqlx::query_scalar("SELECT state FROM command_intents WHERE identity=?1")
                .bind(identity)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
        if state.as_deref() != Some("effect_committed_pending_finalization") {
            return Err(StateError::StateConflict);
        }

        let mut staged: PendingEffectFinalization =
            serde_json::from_slice(&payload).map_err(StateError::Serialization)?;
        staged.state_transition = state_transition.cloned();
        let revised_payload = serde_json::to_vec(&staged).map_err(StateError::Serialization)?;
        if revised_payload.len() > 16_777_216 {
            return Err(StateError::Integrity);
        }
        let revised_fingerprint = blake3::hash(&revised_payload).to_hex().to_string();
        if revised_fingerprint == prior_fingerprint {
            return Err(StateError::StateConflict);
        }
        let revision_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM command_finalization_revisions WHERE identity=?1",
        )
        .bind(identity)
        .fetch_one(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        if revision_count >= 32 {
            return Err(StateError::Integrity);
        }
        let transition_payload =
            serde_json::to_vec(&staged.state_transition).map_err(StateError::Serialization)?;
        let consumed = sqlx::query(
            "INSERT INTO authorization_decisions(decision_id, claims_fingerprint, consumed_at_ms, revoked_at_ms) VALUES(?1, ?2, ?3, NULL) ON CONFLICT(decision_id) DO NOTHING",
        )
        .bind(decision_id)
        .bind(decision_fingerprint)
        .bind(revised_at_ms)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        if consumed.rows_affected() != 1 {
            return Err(StateError::AuthorizationDecisionRejected);
        }
        sqlx::query(
            "INSERT INTO command_finalization_revisions(identity, prior_fingerprint, revised_fingerprint, state_transition, decision_id, decision_fingerprint, revised_at_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(identity)
        .bind(&prior_fingerprint)
        .bind(&revised_fingerprint)
        .bind(transition_payload)
        .bind(decision_id)
        .bind(decision_fingerprint)
        .bind(revised_at_ms)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        let updated = sqlx::query(
            "UPDATE command_finalizations SET payload=?3, fingerprint=?4 WHERE identity=?1 AND fingerprint=?2 AND finalized_at_ms IS NULL",
        )
        .bind(identity)
        .bind(&prior_fingerprint)
        .bind(revised_payload)
        .bind(&revised_fingerprint)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        if updated.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        transaction.commit().await.map_err(StateError::Storage)?;
        Ok(revised_fingerprint)
    }

    /// Applies the proof-staged local transition, outbox and receipt in one transaction.
    /// A CAS conflict leaves the payload pending and the confirmed effect non-retryable.
    pub async fn finalize_effect_committed(&self, identity: &str) -> Result<bool, StateError> {
        self.ensure_owner_healthy()?;
        validate_identity(identity)?;
        let mut transaction = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(StateError::Storage)?;
        let row = sqlx::query(
            "SELECT payload, fingerprint, finalized_at_ms FROM command_finalizations WHERE identity=?1",
        )
        .bind(identity)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(StateError::Storage)?
        .ok_or(StateError::StateConflict)?;
        let payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
        let fingerprint: String = row.try_get("fingerprint").map_err(StateError::Storage)?;
        let finalized_at: Option<i64> = row
            .try_get("finalized_at_ms")
            .map_err(StateError::Storage)?;
        if blake3::hash(&payload).to_hex().as_str() != fingerprint {
            return Err(StateError::Integrity);
        }
        let staged: PendingEffectFinalization =
            serde_json::from_slice(&payload).map_err(StateError::Serialization)?;
        if finalized_at.is_some() {
            transaction.rollback().await.map_err(StateError::Storage)?;
            return Ok(false);
        }
        let intent = sqlx::query("SELECT state FROM command_intents WHERE identity=?1")
            .bind(identity)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(StateError::Storage)?
            .ok_or(StateError::Integrity)?;
        let state: String = intent.try_get("state").map_err(StateError::Storage)?;
        if state != "effect_committed_pending_finalization" {
            return Err(StateError::StateConflict);
        }
        ensure_safe_payload(&staged.result)?;
        let result_bytes = serde_json::to_vec(&staged.result).map_err(StateError::Serialization)?;
        if result_bytes.len() > 4_194_304 || staged.events.len() > 1_024 {
            return Err(StateError::Integrity);
        }
        if let Some(transition) = staged.state_transition.as_ref() {
            validate_key(&transition.owner)?;
            validate_key(&transition.key)?;
            ensure_safe_payload(&transition.value)?;
            let value = serde_json::to_vec(&transition.value).map_err(StateError::Serialization)?;
            let updated = if let Some(expected_version) = transition.expected_version {
                if expected_version == 0 || expected_version > i64::MAX as u64 {
                    return Err(StateError::StateConflict);
                }
                sqlx::query(
                    "UPDATE canonical_state SET value=?3, version=version+1, updated_at=CURRENT_TIMESTAMP WHERE owner=?1 AND key=?2 AND version=?4",
                )
                .bind(&transition.owner)
                .bind(&transition.key)
                .bind(value)
                .bind(expected_version as i64)
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?
            } else {
                sqlx::query(
                    "INSERT INTO canonical_state(owner, key, value, version) VALUES(?1, ?2, ?3, 1) ON CONFLICT(owner, key) DO NOTHING",
                )
                .bind(&transition.owner)
                .bind(&transition.key)
                .bind(value)
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?
            };
            if updated.rows_affected() != 1 {
                return Err(StateError::StateConflict);
            }
            #[cfg(test)]
            crash_commit_process_at("finalize_state_cas");
        }
        #[cfg(test)]
        let mut event_index = 0_usize;
        for event in &staged.events {
            Self::insert_outbox_event(&mut transaction, event).await?;
            #[cfg(test)]
            {
                event_index += 1;
                crash_commit_process_at(&format!("finalize_outbox_{event_index}"));
            }
        }
        let updated = sqlx::query(
            "UPDATE command_intents SET state='committed', result=?2, error_code=NULL, owner_instance_id=NULL, lease_expires_at_ms=NULL, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='effect_committed_pending_finalization'",
        )
        .bind(identity)
        .bind(result_bytes)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        if updated.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        #[cfg(test)]
        crash_commit_process_at("finalize_receipt");
        let finalized_at_ms = unix_time_ms()?;
        let marked = sqlx::query(
            "UPDATE command_finalizations SET finalized_at_ms=?2 WHERE identity=?1 AND finalized_at_ms IS NULL",
        )
        .bind(identity)
        .bind(finalized_at_ms)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        if marked.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        #[cfg(test)]
        crash_commit_process_at("finalize_marker");
        transaction.commit().await.map_err(StateError::Storage)?;
        #[cfg(test)]
        crash_commit_process_at("finalize_after_commit");
        Ok(true)
    }

    pub async fn begin_idempotent(
        &self,
        identity: &str,
    ) -> Result<Option<IdempotencyRecord>, StateError> {
        self.ensure_owner_healthy()?;
        validate_identity(identity)?;
        let now = unix_time_ms()?;
        let lease_expires_at = now.saturating_add(OWNER_LEASE_MS);
        let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
        let inserted = sqlx::query(
            "INSERT INTO command_intents(identity, state, owner_instance_id, lease_expires_at_ms) VALUES(?1, 'in_flight', ?2, ?3) ON CONFLICT(identity) DO NOTHING",
        )
        .bind(identity)
        .bind(&self.owner.instance_id)
        .bind(lease_expires_at)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?
        .rows_affected();
        let existing = if inserted == 0 {
            sqlx::query("UPDATE command_intents SET state='unknown_outcome', error_code='FORGE.COMMAND.RECOVERY_UNKNOWN', owner_instance_id=NULL, lease_expires_at_ms=NULL WHERE identity=?1 AND state='in_flight' AND (owner_instance_id IS NULL OR NOT EXISTS (SELECT 1 FROM store_instances AS owner WHERE owner.instance_id=command_intents.owner_instance_id AND owner.heartbeat_at_ms > ?2 - ?3 AND command_intents.lease_expires_at_ms > ?2))")
                .bind(identity)
                .bind(now)
                .bind(OWNER_LEASE_MS)
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            let row = sqlx::query(
                "SELECT state, result, error_code FROM command_intents WHERE identity=?1",
            )
            .bind(identity)
            .fetch_one(&mut *transaction)
            .await
            .map_err(StateError::Storage)?;
            let state: String = row.try_get("state").map_err(StateError::Storage)?;
            let result: Option<Vec<u8>> = row.try_get("result").map_err(StateError::Storage)?;
            let error_code: Option<String> =
                row.try_get("error_code").map_err(StateError::Storage)?;
            let resolution_row = sqlx::query(
                "SELECT outcome, evidence_fingerprint, decision_fingerprint, decision_id, resolved_at_ms FROM command_outcome_resolutions WHERE identity=?1 ORDER BY resolution_id DESC LIMIT 1",
            )
            .bind(identity)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(StateError::Storage)?;
            let latest_resolution = resolution_row
                .map(
                    |resolution| -> Result<CommandOutcomeResolutionRecord, StateError> {
                        let outcome: String =
                            resolution.try_get("outcome").map_err(StateError::Storage)?;
                        let evidence_fingerprint: String = resolution
                            .try_get("evidence_fingerprint")
                            .map_err(StateError::Storage)?;
                        let decision_fingerprint: String = resolution
                            .try_get("decision_fingerprint")
                            .map_err(StateError::Storage)?;
                        let decision_id: String = resolution
                            .try_get("decision_id")
                            .map_err(StateError::Storage)?;
                        let resolved_at_ms: i64 = resolution
                            .try_get("resolved_at_ms")
                            .map_err(StateError::Storage)?;
                        Ok(CommandOutcomeResolutionRecord {
                            kind: CommandOutcomeResolutionKind::from_db(&outcome)?,
                            evidence_fingerprint,
                            decision_fingerprint,
                            decision_id,
                            resolved_at_ms: u64::try_from(resolved_at_ms)
                                .map_err(|_| StateError::Integrity)?,
                        })
                    },
                )
                .transpose()?;
            Some(IdempotencyRecord {
                state: IdempotencyState::from_db(&state)?,
                result: result
                    .map(|bytes| serde_json::from_slice(&bytes).map_err(StateError::Serialization))
                    .transpose()?,
                error_code,
                latest_resolution,
            })
        } else {
            None
        };
        transaction.commit().await.map_err(StateError::Storage)?;
        Ok(existing)
    }

    pub async fn retry_idempotent(&self, identity: &str) -> Result<bool, StateError> {
        self.ensure_owner_healthy()?;
        validate_identity(identity)?;
        let update = sqlx::query(
            "UPDATE command_intents SET state='in_flight', result=NULL, error_code=NULL, owner_instance_id=?2, lease_expires_at_ms=?3, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='failed_retryable'",
        )
        .bind(identity)
        .bind(&self.owner.instance_id)
        .bind(unix_time_ms()?.saturating_add(OWNER_LEASE_MS))
        .execute(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        Ok(update.rows_affected() == 1)
    }

    pub async fn complete_idempotent(
        &self,
        identity: &str,
        result: &Value,
    ) -> Result<(), StateError> {
        validate_identity(identity)?;
        ensure_safe_payload(result)?;
        let bytes = serde_json::to_vec(result).map_err(StateError::Serialization)?;
        self.transition_idempotency(identity, IdempotencyState::Committed, Some(bytes), None)
            .await
    }

    pub async fn fail_idempotent(
        &self,
        identity: &str,
        state: IdempotencyState,
        error_code: &str,
    ) -> Result<(), StateError> {
        if !matches!(
            state,
            IdempotencyState::FailedRetryable
                | IdempotencyState::FailedFinal
                | IdempotencyState::UnknownOutcome
        ) {
            return Err(StateError::StateConflict);
        }
        validate_identity(identity)?;
        validate_error_code(error_code)?;
        self.transition_idempotency(identity, state, None, Some(error_code.to_owned()))
            .await
    }

    /// Applies a host-verified reconciliation result to an ambiguous external command.
    /// The caller must verify the host signature before this method; the decision id,
    /// its fingerprint, the resolution record, and the state transition commit atomically.
    pub async fn reconcile_unknown_outcome(
        &self,
        identity: &str,
        kind: CommandOutcomeResolutionKind,
        evidence_fingerprint: &str,
        decision_id: &str,
        decision_fingerprint: &str,
        resolved_at_ms: u64,
    ) -> Result<(), StateError> {
        self.ensure_owner_healthy()?;
        validate_identity(identity)?;
        validate_key(decision_id)?;
        validate_digest(evidence_fingerprint)?;
        validate_digest(decision_fingerprint)?;
        let resolved_at_ms = i64::try_from(resolved_at_ms).map_err(|_| StateError::Integrity)?;
        let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
        let consumed = sqlx::query(
            "INSERT INTO authorization_decisions(decision_id, claims_fingerprint, consumed_at_ms, revoked_at_ms) VALUES(?1, ?2, ?3, NULL) ON CONFLICT(decision_id) DO NOTHING",
        )
        .bind(decision_id)
        .bind(decision_fingerprint)
        .bind(resolved_at_ms)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        if consumed.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        let (next_state, error_code) = match kind {
            CommandOutcomeResolutionKind::EffectCommitted => (
                "effect_committed_pending_finalization",
                "FORGE.COMMAND.EFFECT_COMMITTED_REQUIRES_FINALIZATION",
            ),
            CommandOutcomeResolutionKind::EffectNotCommitted => {
                ("failed_retryable", "FORGE.COMMAND.RECONCILED_NOT_COMMITTED")
            }
            CommandOutcomeResolutionKind::Inconclusive => (
                "unknown_outcome",
                "FORGE.COMMAND.RECONCILIATION_INCONCLUSIVE",
            ),
        };
        let update = sqlx::query(
            "UPDATE command_intents SET state=?2, result=NULL, error_code=?3, owner_instance_id=NULL, lease_expires_at_ms=NULL, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='unknown_outcome'",
        )
        .bind(identity)
        .bind(next_state)
        .bind(error_code)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        if update.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        sqlx::query(
            "INSERT INTO command_outcome_resolutions(identity, decision_id, outcome, evidence_fingerprint, decision_fingerprint, resolved_at_ms) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(identity)
        .bind(decision_id)
        .bind(kind.as_db())
        .bind(evidence_fingerprint)
        .bind(decision_fingerprint)
        .bind(resolved_at_ms)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        transaction.commit().await.map_err(StateError::Storage)?;
        Ok(())
    }

    pub async fn consume_host_authorization_decision(
        &self,
        decision_id: &str,
        claims_fingerprint: &str,
        consumed_at_ms: u64,
    ) -> Result<bool, StateError> {
        validate_key(decision_id)?;
        validate_digest(claims_fingerprint)?;
        let consumed_at_ms = i64::try_from(consumed_at_ms).map_err(|_| StateError::Integrity)?;
        let result = sqlx::query("INSERT INTO authorization_decisions(decision_id, claims_fingerprint, consumed_at_ms, revoked_at_ms) VALUES(?1, ?2, ?3, NULL) ON CONFLICT(decision_id) DO NOTHING")
            .bind(decision_id)
            .bind(claims_fingerprint)
            .bind(consumed_at_ms)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn revoke_host_authorization_decision(
        &self,
        decision_id: &str,
    ) -> Result<(), StateError> {
        validate_key(decision_id)?;
        let now = unix_time_ms()?;
        let revocation_fingerprint = blake3::hash(format!("revoked:{decision_id}").as_bytes())
            .to_hex()
            .to_string();
        sqlx::query("INSERT INTO authorization_decisions(decision_id, claims_fingerprint, consumed_at_ms, revoked_at_ms) VALUES(?1, ?2, NULL, ?3) ON CONFLICT(decision_id) DO UPDATE SET revoked_at_ms=COALESCE(authorization_decisions.revoked_at_ms, excluded.revoked_at_ms)")
            .bind(decision_id)
            .bind(revocation_fingerprint)
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(())
    }

    pub async fn enqueue_event(&self, event: &StoredEvent) -> Result<(), StateError> {
        let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
        Self::insert_outbox_event(&mut transaction, event).await?;
        transaction.commit().await.map_err(StateError::Storage)?;
        Ok(())
    }

    pub async fn pending_events_for_consumer(
        &self,
        consumer_id: &str,
        limit: u32,
    ) -> Result<Vec<PendingEventRecord>, StateError> {
        validate_key(consumer_id)?;
        let rows = sqlx::query(
            "SELECT event.event_id, event.contract_id, event.payload FROM event_outbox AS event \
             WHERE NOT EXISTS (SELECT 1 FROM event_consumer_state AS state \
             WHERE state.event_id=event.event_id AND state.consumer_id=?1 \
             AND (state.acknowledged=1 OR state.quarantined=1)) \
             ORDER BY event.producer_id, event.producer_sequence LIMIT ?2",
        )
        .bind(consumer_id)
        .bind(i64::from(limit.min(10_000)))
        .fetch_all(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        rows.into_iter()
            .map(|row| {
                Ok(PendingEventRecord {
                    event_id: row.try_get("event_id").map_err(StateError::Storage)?,
                    contract_id: row.try_get("contract_id").map_err(StateError::Storage)?,
                    payload_bytes: row.try_get("payload").map_err(StateError::Storage)?,
                })
            })
            .collect()
    }

    pub async fn acknowledge_event_for_consumer(
        &self,
        event_id: &str,
        consumer_id: &str,
    ) -> Result<bool, StateError> {
        validate_key(event_id)?;
        validate_key(consumer_id)?;
        let result = sqlx::query(
            "INSERT INTO event_consumer_state(event_id, consumer_id, delivery_attempts, acknowledged, quarantined, last_error_code) \
             SELECT event_id, ?2, 0, 1, 0, NULL FROM event_outbox WHERE event_id=?1 \
             ON CONFLICT(event_id, consumer_id) DO UPDATE SET acknowledged=1, updated_at=CURRENT_TIMESTAMP \
             WHERE event_consumer_state.acknowledged=0 AND event_consumer_state.quarantined=0",
        )
            .bind(event_id)
            .bind(consumer_id)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn record_event_decode_failure(
        &self,
        event_id: &str,
        consumer_id: &str,
    ) -> Result<bool, StateError> {
        validate_key(event_id)?;
        validate_key(consumer_id)?;
        let result = sqlx::query(
            "INSERT INTO event_consumer_state(event_id, consumer_id, delivery_attempts, acknowledged, quarantined, last_error_code) \
             SELECT event_id, ?2, 1, 0, 0, 'FORGE.EVENT.POISON_PAYLOAD' FROM event_outbox WHERE event_id=?1 \
             ON CONFLICT(event_id, consumer_id) DO UPDATE SET \
             delivery_attempts=event_consumer_state.delivery_attempts+1, \
             quarantined=CASE WHEN event_consumer_state.delivery_attempts+1 >= ?3 THEN 1 ELSE 0 END, \
             last_error_code='FORGE.EVENT.POISON_PAYLOAD', updated_at=CURRENT_TIMESTAMP \
             WHERE event_consumer_state.acknowledged=0 AND event_consumer_state.quarantined=0",
        )
        .bind(event_id)
        .bind(consumer_id)
        .bind(EVENT_POISON_QUARANTINE_THRESHOLD)
        .execute(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn record_evidence(&self, evidence: &EvidenceRecord) -> Result<(), StateError> {
        validate_key(&evidence.evidence_id)?;
        validate_key(&evidence.kind)?;
        validate_digest(&evidence.fingerprint)?;
        ensure_safe_payload(&evidence.payload)?;
        let payload = serde_json::to_vec(&evidence.payload).map_err(StateError::Serialization)?;
        let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
        let row =
            sqlx::query("SELECT kind, fingerprint, payload FROM evidence WHERE evidence_id=?1")
                .bind(&evidence.evidence_id)
                .fetch_optional(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
        if let Some(row) = row {
            let existing_kind: String = row.try_get("kind").map_err(StateError::Storage)?;
            let existing_fingerprint: String =
                row.try_get("fingerprint").map_err(StateError::Storage)?;
            let existing_payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
            transaction.rollback().await.map_err(StateError::Storage)?;
            return if existing_kind == evidence.kind
                && existing_fingerprint == evidence.fingerprint
                && existing_payload == payload
            {
                Ok(())
            } else {
                Err(StateError::EvidenceConflict)
            };
        }
        sqlx::query(
            "INSERT INTO evidence(evidence_id, kind, fingerprint, payload) VALUES(?1, ?2, ?3, ?4)",
        )
        .bind(&evidence.evidence_id)
        .bind(&evidence.kind)
        .bind(&evidence.fingerprint)
        .bind(payload)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?;
        transaction.commit().await.map_err(StateError::Storage)?;
        Ok(())
    }

    pub async fn put_cache(
        &self,
        namespace: &str,
        key: &str,
        fingerprint: &str,
        value: &Value,
    ) -> Result<(), StateError> {
        validate_key(namespace)?;
        validate_key(key)?;
        validate_digest(fingerprint)?;
        ensure_safe_payload(value)?;
        let bytes = serde_json::to_vec(value).map_err(StateError::Serialization)?;
        sqlx::query(
            "INSERT INTO disposable_cache(namespace, cache_key, fingerprint, value) VALUES(?1, ?2, ?3, ?4) \
             ON CONFLICT(namespace, cache_key) DO UPDATE SET fingerprint=excluded.fingerprint, value=excluded.value",
        )
        .bind(namespace)
        .bind(key)
        .bind(fingerprint)
        .bind(bytes)
        .execute(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        Ok(())
    }

    pub async fn get_cache(
        &self,
        namespace: &str,
        key: &str,
        fingerprint: &str,
    ) -> Result<Option<Value>, StateError> {
        validate_key(namespace)?;
        validate_key(key)?;
        validate_digest(fingerprint)?;
        let row = sqlx::query(
            "SELECT fingerprint, value FROM disposable_cache WHERE namespace=?1 AND cache_key=?2",
        )
        .bind(namespace)
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        let Some(row) = row else { return Ok(None) };
        let stored_fingerprint: String = row.try_get("fingerprint").map_err(StateError::Storage)?;
        if stored_fingerprint != fingerprint {
            return Ok(None);
        }
        let bytes: Vec<u8> = row.try_get("value").map_err(StateError::Storage)?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(StateError::Serialization)
    }

    pub async fn clear_cache(&self) -> Result<u64, StateError> {
        let result = sqlx::query("DELETE FROM disposable_cache")
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(result.rows_affected())
    }

    pub fn put_blob(&self, bytes: &[u8]) -> Result<String, StateError> {
        let digest = blake3::hash(bytes).to_hex().to_string();
        let cas_root = self.root.join("cas");
        ensure_real_directory(&cas_root, true)?;
        let directory = cas_root.join(&digest[..2]);
        ensure_real_directory(&directory, true)?;
        let destination = directory.join(&digest);
        reject_symlink_if_present(&destination)?;
        if destination.exists() {
            verify_blob(&destination, &digest)?;
            return Ok(digest);
        }

        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary =
            directory.join(format!(".{digest}.{}.{}.tmp", std::process::id(), sequence));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            match fs::hard_link(&temporary, &destination) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    verify_blob(&destination, &digest)
                }
                Err(error) => Err(StateError::Filesystem(error)),
            }
        })();
        let cleanup_result = fs::remove_file(&temporary).map_err(StateError::Filesystem);
        write_result?;
        cleanup_result?;
        Ok(digest)
    }

    pub fn get_blob(&self, digest: &str) -> Result<Option<Vec<u8>>, StateError> {
        validate_digest(digest)?;
        let cas_root = self.root.join("cas");
        if !ensure_real_directory(&cas_root, false)? {
            return Ok(None);
        }
        let shard = cas_root.join(&digest[..2]);
        if !ensure_real_directory(&shard, false)? {
            return Ok(None);
        }
        let path = shard.join(digest);
        reject_symlink_if_present(&path)?;
        let mut file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(StateError::Filesystem)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(StateError::Filesystem)?;
        if blake3::hash(&bytes).to_hex().as_str() != digest {
            return Err(StateError::Integrity);
        }
        Ok(Some(bytes))
    }

    pub async fn backup_to(&self, backup_root: impl AsRef<Path>) -> Result<String, StateError> {
        fs::create_dir_all(backup_root.as_ref()).map_err(StateError::Filesystem)?;
        let backup_root = backup_root
            .as_ref()
            .canonicalize()
            .map_err(|_| StateError::InvalidRoot)?;
        let backup_db = backup_root.join("forge.sqlite3");
        let manifest_path = backup_root.join("manifest.json");
        let backup_cas = backup_root.join("cas");
        reject_symlink_if_present(&backup_db)?;
        reject_symlink_if_present(&manifest_path)?;
        reject_symlink_if_present(&backup_cas)?;
        if backup_db.exists() || manifest_path.exists() || backup_cas.exists() {
            return Err(StateError::BackupTargetExists);
        }
        let backup_path = backup_db.to_string_lossy().into_owned();
        sqlx::query("VACUUM INTO ?1")
            .bind(backup_path)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        let bytes = fs::read(&backup_db).map_err(StateError::Filesystem)?;
        let database_fingerprint = blake3::hash(&bytes).to_hex().to_string();
        let cas_objects = snapshot_cas(&self.root, &backup_root)?;
        let unsigned = BackupManifestUnsigned {
            schema_version: 2,
            database_schema_version: SCHEMA_VERSION,
            database_fingerprint,
            database_hash_algorithm: "blake3".into(),
            cas_objects,
        };
        let backup_fingerprint = backup_manifest_fingerprint(&unsigned)?;
        let manifest = BackupManifest {
            schema_version: unsigned.schema_version,
            database_schema_version: unsigned.database_schema_version,
            database_fingerprint: unsigned.database_fingerprint,
            database_hash_algorithm: unsigned.database_hash_algorithm,
            cas_objects: unsigned.cas_objects,
            backup_fingerprint: backup_fingerprint.clone(),
        };
        let manifest_bytes =
            serde_json::to_vec_pretty(&manifest).map_err(StateError::Serialization)?;
        if let Err(error) = write_atomic(&manifest_path, &manifest_bytes) {
            let _ = fs::remove_file(&backup_db);
            return Err(error);
        }
        Ok(backup_fingerprint)
    }

    pub async fn restore_from_backup(
        backup_root: impl AsRef<Path>,
        restore_root: impl AsRef<Path>,
        expected_backup_fingerprint: &str,
    ) -> Result<Self, StateError> {
        validate_digest(expected_backup_fingerprint)?;
        let backup_root = backup_root
            .as_ref()
            .canonicalize()
            .map_err(|_| StateError::InvalidRoot)?;
        let backup_db = backup_root.join("forge.sqlite3");
        let manifest_path = backup_root.join("manifest.json");
        reject_symlink_if_present(&backup_db)?;
        reject_symlink_if_present(&manifest_path)?;
        let manifest_bytes = fs::read(&manifest_path).map_err(StateError::Filesystem)?;
        let manifest: BackupManifest =
            serde_json::from_slice(&manifest_bytes).map_err(StateError::Serialization)?;
        let database_bytes = fs::read(&backup_db).map_err(StateError::Filesystem)?;
        let actual_fingerprint = blake3::hash(&database_bytes).to_hex().to_string();
        let unsigned = BackupManifestUnsigned {
            schema_version: manifest.schema_version,
            database_schema_version: manifest.database_schema_version,
            database_fingerprint: manifest.database_fingerprint.clone(),
            database_hash_algorithm: manifest.database_hash_algorithm.clone(),
            cas_objects: manifest.cas_objects.clone(),
        };
        let calculated_backup_fingerprint = backup_manifest_fingerprint(&unsigned)?;
        if manifest.schema_version != 2
            || manifest.database_schema_version < 1
            || manifest.database_schema_version > SCHEMA_VERSION
            || manifest.database_hash_algorithm != "blake3"
            || manifest.database_fingerprint != actual_fingerprint
            || manifest.backup_fingerprint != expected_backup_fingerprint
            || calculated_backup_fingerprint != expected_backup_fingerprint
        {
            return Err(StateError::Integrity);
        }
        let cas_objects = verify_backup_cas(&backup_root, &manifest.cas_objects)?;

        fs::create_dir_all(restore_root.as_ref()).map_err(StateError::Filesystem)?;
        let restore_root = restore_root
            .as_ref()
            .canonicalize()
            .map_err(|_| StateError::InvalidRoot)?;
        let restored_db = restore_root.join("forge.sqlite3");
        let restored_cas = restore_root.join("cas");
        reject_symlink_if_present(&restored_db)?;
        reject_symlink_if_present(&restored_cas)?;
        if restored_db.exists() || restored_cas.exists() {
            return Err(StateError::BackupTargetExists);
        }
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&restored_db)
            .map_err(StateError::Filesystem)?;
        destination
            .write_all(&database_bytes)
            .map_err(StateError::Filesystem)?;
        destination.sync_all().map_err(StateError::Filesystem)?;
        drop(destination);
        ensure_real_directory(&restored_cas, true)?;
        for (entry, bytes) in cas_objects {
            let shard = restored_cas.join(&entry.digest[..2]);
            ensure_real_directory(&shard, true)?;
            write_atomic(&shard.join(&entry.digest), &bytes)?;
        }
        let restored = Self::open(&restore_root).await?;
        restored.integrity_check().await?;
        Ok(restored)
    }

    pub async fn integrity_check(&self) -> Result<(), StateError> {
        let row = sqlx::query("PRAGMA quick_check")
            .fetch_one(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        let result: String = row.try_get(0).map_err(StateError::Storage)?;
        if result != "ok" {
            return Err(StateError::Integrity);
        }
        let version_row = sqlx::query("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        let version: i64 = version_row.try_get(0).map_err(StateError::Storage)?;
        if version != SCHEMA_VERSION {
            return Err(StateError::Integrity);
        }
        let totals = sqlx::query(
            "SELECT totals.pool, totals.tokens, totals.cost_micros, totals.record_count, \
             COALESCE(SUM(usage.tokens), 0) AS ledger_tokens, \
             COALESCE(SUM(usage.cost_micros), 0) AS ledger_cost_micros, \
             COUNT(usage.usage_id) AS ledger_count \
             FROM resource_usage_totals AS totals \
             LEFT JOIN resource_usage AS usage ON usage.pool=totals.pool \
             GROUP BY totals.pool, totals.tokens, totals.cost_micros, totals.record_count",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        if totals.len() != 2 {
            return Err(StateError::Integrity);
        }
        let mut seen_pools = BTreeSet::new();
        let mut total_records = 0_i64;
        for row in totals {
            let pool: String = row.try_get("pool").map_err(StateError::Storage)?;
            let tokens: i64 = row.try_get("tokens").map_err(StateError::Storage)?;
            let cost_micros: i64 = row.try_get("cost_micros").map_err(StateError::Storage)?;
            let record_count: i64 = row.try_get("record_count").map_err(StateError::Storage)?;
            let ledger_tokens: i64 = row.try_get("ledger_tokens").map_err(StateError::Storage)?;
            let ledger_cost_micros: i64 = row
                .try_get("ledger_cost_micros")
                .map_err(StateError::Storage)?;
            let ledger_count: i64 = row.try_get("ledger_count").map_err(StateError::Storage)?;
            if !matches!(pool.as_str(), "ordinary" | "survival")
                || !seen_pools.insert(pool)
                || tokens != ledger_tokens
                || cost_micros != ledger_cost_micros
                || record_count != ledger_count
            {
                return Err(StateError::Integrity);
            }
            total_records = total_records
                .checked_add(record_count)
                .ok_or(StateError::Integrity)?;
        }
        if total_records > 100_000 {
            return Err(StateError::Integrity);
        }
        Ok(())
    }

    async fn migrate(&self) -> Result<(), StateError> {
        let version_row = sqlx::query("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        let current: i64 = version_row.try_get(0).map_err(StateError::Storage)?;
        if current > SCHEMA_VERSION {
            return Err(StateError::Integrity);
        }
        if current == 0 {
            let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
            for statement in MIGRATIONS.iter().copied() {
                sqlx::query(statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            sqlx::query("PRAGMA user_version = 1")
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            transaction.commit().await.map_err(StateError::Storage)?;
        }
        if current < 2 {
            let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
            for statement in RESOURCE_USAGE_MIGRATIONS.iter().copied() {
                sqlx::query(statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            sqlx::query("PRAGMA user_version = 2")
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            transaction.commit().await.map_err(StateError::Storage)?;
        }
        if current < 3 {
            let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
            let columns = sqlx::query("PRAGMA table_info(command_intents)")
                .fetch_all(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            let has_column = |name: &str| -> Result<bool, StateError> {
                columns
                    .iter()
                    .map(|row| {
                        row.try_get::<String, _>("name")
                            .map_err(StateError::Storage)
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(|names| names.iter().any(|column| column == name))
            };
            if !has_column("owner_instance_id")? {
                sqlx::query(CORRECTION_MIGRATIONS[0])
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            if !has_column("lease_expires_at_ms")? {
                sqlx::query(CORRECTION_MIGRATIONS[1])
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            for statement in CORRECTION_MIGRATIONS.iter().skip(2).copied() {
                sqlx::query(statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            sqlx::query("PRAGMA user_version = 3")
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            transaction.commit().await.map_err(StateError::Storage)?;
        }
        if current < 4 {
            let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
            for statement in OUTCOME_RESOLUTION_MIGRATIONS.iter().copied() {
                sqlx::query(statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            sqlx::query("PRAGMA user_version = 4")
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            transaction.commit().await.map_err(StateError::Storage)?;
        }
        if current < 5 {
            let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
            let event_columns = sqlx::query("PRAGMA table_info(event_outbox)")
                .fetch_all(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            let has_event_column = |name: &str| -> Result<bool, StateError> {
                event_columns
                    .iter()
                    .map(|row| {
                        row.try_get::<String, _>("name")
                            .map_err(StateError::Storage)
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(|names| names.iter().any(|column| column == name))
            };
            if !has_event_column("producer_id")? {
                sqlx::query(EVENT_DELIVERY_MIGRATIONS[0])
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            if !has_event_column("producer_sequence")? {
                sqlx::query(EVENT_DELIVERY_MIGRATIONS[1])
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            for statement in EVENT_DELIVERY_MIGRATIONS.iter().skip(2).copied() {
                sqlx::query(statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            sqlx::query("PRAGMA user_version = 5")
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            transaction.commit().await.map_err(StateError::Storage)?;
        }
        if current < 6 {
            let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
            for statement in EFFECT_FINALIZATION_MIGRATIONS.iter().copied() {
                sqlx::query(statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            sqlx::query("PRAGMA user_version = 6")
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            transaction.commit().await.map_err(StateError::Storage)?;
        }
        if current < 7 {
            let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
            for statement in EFFECT_FINALIZATION_REVISION_MIGRATIONS.iter().copied() {
                sqlx::query(statement)
                    .execute(&mut *transaction)
                    .await
                    .map_err(StateError::Storage)?;
            }
            sqlx::query("PRAGMA user_version = 7")
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?;
            transaction.commit().await.map_err(StateError::Storage)?;
        }
        Ok(())
    }

    async fn transition_idempotency(
        &self,
        identity: &str,
        state: IdempotencyState,
        result: Option<Vec<u8>>,
        error_code: Option<String>,
    ) -> Result<(), StateError> {
        self.ensure_owner_healthy()?;
        if let Some(code) = error_code.as_deref() {
            validate_error_code(code)?;
        }
        let update = sqlx::query(
            "UPDATE command_intents SET state=?2, result=?3, error_code=?4, owner_instance_id=NULL, lease_expires_at_ms=NULL, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='in_flight' AND owner_instance_id=?5",
        )
        .bind(identity)
        .bind(state.as_db())
        .bind(result)
        .bind(error_code)
        .bind(&self.owner.instance_id)
        .execute(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        if update.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        Ok(())
    }

    async fn mark_interrupted_commands_unknown(&self) -> Result<(), StateError> {
        let now = unix_time_ms()?;
        sqlx::query("UPDATE command_intents SET state='unknown_outcome', error_code='FORGE.COMMAND.RECOVERY_UNKNOWN', owner_instance_id=NULL, lease_expires_at_ms=NULL WHERE state='in_flight' AND (owner_instance_id IS NULL OR NOT EXISTS (SELECT 1 FROM store_instances AS owner WHERE owner.instance_id=command_intents.owner_instance_id AND owner.heartbeat_at_ms > ?1 - ?2 AND command_intents.lease_expires_at_ms > ?1))")
            .bind(now)
            .bind(OWNER_LEASE_MS)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        sqlx::query("DELETE FROM store_instances WHERE heartbeat_at_ms <= ?1 - ?2")
            .bind(now)
            .bind(OWNER_LEASE_MS)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(())
    }
}

fn unix_time_ms() -> Result<i64, StateError> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StateError::Integrity)?
        .as_millis();
    i64::try_from(milliseconds).map_err(|_| StateError::Integrity)
}

#[cfg(test)]
fn crash_commit_process_at(boundary: &str) {
    if std::env::var("FORGE_TEST_COMMIT_CRASH_BOUNDARY")
        .ok()
        .as_deref()
        == Some(boundary)
    {
        std::process::exit(86);
    }
}

fn new_store_instance_id() -> String {
    let sequence = STORE_INSTANCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let material = format!(
        "{}:{}:{}",
        std::process::id(),
        unix_time_ms().unwrap_or_default(),
        sequence
    );
    format!("store.{}", blake3::hash(material.as_bytes()).to_hex())
}

fn ensure_safe_payload(value: &Value) -> Result<(), StateError> {
    let mut pending = vec![(value, 0_usize)];
    let mut visited = 0_usize;
    while let Some((current, depth)) = pending.pop() {
        visited += 1;
        if visited > 100_000 || depth > 64 {
            return Err(StateError::SensitivePayload);
        }
        match current {
            Value::Object(fields) => {
                for (name, value) in fields {
                    if secret_bearing_field(name)
                        && !(value.is_number() && safe_numeric_usage_field(name))
                    {
                        return Err(StateError::SensitivePayload);
                    }
                    pending.push((value, depth + 1));
                }
            }
            Value::Array(items) => {
                pending.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::String(value) if looks_like_credential(value) => {
                return Err(StateError::SensitivePayload);
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn validate_payload_for_persistence(value: &Value) -> Result<(), StateError> {
    ensure_safe_payload(value)
}

fn safe_numeric_usage_field(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "tokens" | "cache_tokens_reused" | "cachetokensreused"
    )
}

fn secret_bearing_field(name: &str) -> bool {
    let mut normalized = String::with_capacity(name.len() + 8);
    let characters = name.chars().collect::<Vec<_>>();
    for (index, character) in characters.iter().copied().enumerate() {
        if !character.is_ascii_alphanumeric() {
            normalized.push(' ');
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|i| characters.get(i))
            .copied();
        let next = characters.get(index + 1).copied();
        let camel_boundary = character.is_ascii_uppercase()
            && previous.is_some_and(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
            || character.is_ascii_uppercase()
                && previous.is_some_and(|value| value.is_ascii_uppercase())
                && next.is_some_and(|value| value.is_ascii_lowercase());
        if camel_boundary {
            normalized.push(' ');
        }
        normalized.push(character.to_ascii_lowercase());
    }
    let tokens = normalized.split_ascii_whitespace().collect::<Vec<_>>();
    let sensitive_parts = [
        "password",
        "passwd",
        "secret",
        "secrets",
        "token",
        "credential",
        "credentials",
        "cookie",
        "cookies",
        "authorization",
        "authorizationheader",
    ];
    if tokens.iter().any(|token| {
        sensitive_parts.contains(token)
            || sensitive_parts.iter().any(|marker| token.contains(marker))
    }) {
        return true;
    }

    // `key` is sensitive as a complete name/component and in common compound
    // credential names. Requiring a token or a known compound avoids treating
    // unrelated words such as "monkey" as credential fields.
    let known_compounds = [
        "apikey",
        "accesskey",
        "clientkey",
        "encryptionkey",
        "kmskey",
        "privatekey",
        "publickey",
        "secretkey",
        "signingkey",
        "sshkey",
    ];
    tokens
        .iter()
        .any(|token| *token == "key" || *token == "keys")
        || known_compounds
            .iter()
            .any(|compound| name.to_ascii_lowercase().contains(compound))
}

fn validate_error_code(value: &str) -> Result<(), StateError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_uppercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        });
    if valid {
        Ok(())
    } else {
        Err(StateError::InvalidKey)
    }
}

fn looks_like_credential(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("bearer ")
        || lower.contains("-----begin ") && lower.contains("private key-----")
    {
        return true;
    }
    ["ghp_", "github_pat_", "gho_", "ghu_", "ghs_", "ghr_", "sk-"]
        .iter()
        .any(|marker| {
            lower.find(marker).is_some_and(|index| {
                lower[index + marker.len()..]
                    .bytes()
                    .take_while(u8::is_ascii_alphanumeric)
                    .count()
                    >= 16
            })
        })
        || value.starts_with("AKIA")
            && value.get(4..20).is_some_and(|suffix| {
                suffix.len() == 16 && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
            })
}

fn validate_key(value: &str) -> Result<(), StateError> {
    let valid = !value.trim().is_empty()
        && value.len() <= 512
        && !value.contains('\0')
        && !value.split(['/', '\\']).any(|part| part == "..");
    if valid {
        Ok(())
    } else {
        Err(StateError::InvalidKey)
    }
}

fn validate_identity(value: &str) -> Result<(), StateError> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(StateError::InvalidIdempotencyIdentity)
    }
}

fn validate_resource_usage_record(record: &PersistedResourceUsageRecord) -> Result<(), StateError> {
    validate_identity(&record.usage_id)?;
    for value in [
        record.owner.as_str(),
        record.category.as_str(),
        record.attribution.project_id.as_str(),
        record.attribution.work_order_id.as_str(),
        record.attribution.execution_id.as_str(),
        record.attribution.capability_id.as_str(),
    ] {
        validate_key(value)?;
    }
    if let Some(provider_id) = record.attribution.provider_id.as_deref() {
        validate_key(provider_id)?;
    }
    if record.used == PersistedResourceVector::default() && record.cache_tokens_reused == 0 {
        return Err(StateError::InvalidKey);
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), StateError> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(StateError::Integrity)
    }
}

fn reject_symlink_if_present(path: &Path) -> Result<(), StateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(StateError::Integrity),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(StateError::Filesystem(error)),
    }
}

fn backup_manifest_fingerprint(unsigned: &BackupManifestUnsigned) -> Result<String, StateError> {
    let bytes = serde_json::to_vec(unsigned).map_err(StateError::Serialization)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn read_cas_snapshot(cas_root: &Path) -> Result<Vec<(BackupCasObject, Vec<u8>)>, StateError> {
    if !ensure_real_directory(cas_root, false)? {
        return Ok(Vec::new());
    }
    let mut objects = Vec::new();
    for shard_entry in fs::read_dir(cas_root).map_err(StateError::Filesystem)? {
        let shard_entry = shard_entry.map_err(StateError::Filesystem)?;
        let shard_path = shard_entry.path();
        reject_symlink_if_present(&shard_path)?;
        if !shard_entry
            .file_type()
            .map_err(StateError::Filesystem)?
            .is_dir()
        {
            return Err(StateError::Integrity);
        }
        let shard = shard_entry
            .file_name()
            .into_string()
            .map_err(|_| StateError::Integrity)?
            .to_ascii_lowercase();
        if shard.len() != 2 || !shard.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(StateError::Integrity);
        }
        for object_entry in fs::read_dir(&shard_path).map_err(StateError::Filesystem)? {
            let object_entry = object_entry.map_err(StateError::Filesystem)?;
            let object_path = object_entry.path();
            reject_symlink_if_present(&object_path)?;
            if !object_entry
                .file_type()
                .map_err(StateError::Filesystem)?
                .is_file()
            {
                return Err(StateError::Integrity);
            }
            let digest = object_entry
                .file_name()
                .into_string()
                .map_err(|_| StateError::Integrity)?
                .to_ascii_lowercase();
            validate_digest(&digest)?;
            if !digest.starts_with(&shard) {
                return Err(StateError::Integrity);
            }
            let bytes = fs::read(&object_path).map_err(StateError::Filesystem)?;
            if blake3::hash(&bytes).to_hex().as_str() != digest {
                return Err(StateError::Integrity);
            }
            objects.push((
                BackupCasObject {
                    digest,
                    size: u64::try_from(bytes.len()).map_err(|_| StateError::Integrity)?,
                },
                bytes,
            ));
        }
    }
    objects.sort_by(|left, right| left.0.digest.cmp(&right.0.digest));
    if objects
        .windows(2)
        .any(|pair| pair[0].0.digest == pair[1].0.digest)
    {
        return Err(StateError::Integrity);
    }
    Ok(objects)
}

fn snapshot_cas(
    source_root: &Path,
    backup_root: &Path,
) -> Result<Vec<BackupCasObject>, StateError> {
    let source_cas = source_root.join("cas");
    let backup_cas = backup_root.join("cas");
    reject_symlink_if_present(&backup_cas)?;
    ensure_real_directory(&backup_cas, true)?;
    let objects = read_cas_snapshot(&source_cas)?;
    for (entry, bytes) in &objects {
        let shard = backup_cas.join(&entry.digest[..2]);
        ensure_real_directory(&shard, true)?;
        write_atomic(&shard.join(&entry.digest), bytes)?;
    }
    Ok(objects.into_iter().map(|(entry, _)| entry).collect())
}

fn verify_backup_cas(
    backup_root: &Path,
    expected: &[BackupCasObject],
) -> Result<Vec<(BackupCasObject, Vec<u8>)>, StateError> {
    let mut expected = expected.to_vec();
    expected.sort_by(|left, right| left.digest.cmp(&right.digest));
    if expected
        .windows(2)
        .any(|pair| pair[0].digest == pair[1].digest)
    {
        return Err(StateError::Integrity);
    }
    for object in &expected {
        validate_digest(&object.digest)?;
    }
    let actual = read_cas_snapshot(&backup_root.join("cas"))?;
    let actual_manifest = actual
        .iter()
        .map(|(entry, _)| entry.clone())
        .collect::<Vec<_>>();
    if actual_manifest != expected {
        return Err(StateError::Integrity);
    }
    Ok(actual)
}

fn ensure_real_directory(path: &Path, create: bool) -> Result<bool, StateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(StateError::Integrity)
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => Ok(false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match fs::create_dir(path) {
            Ok(()) => ensure_real_directory(path, false),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                ensure_real_directory(path, false)
            }
            Err(error) => Err(StateError::Filesystem(error)),
        },
        Err(error) => Err(StateError::Filesystem(error)),
    }
}

fn verify_blob(path: &Path, expected_digest: &str) -> Result<(), StateError> {
    reject_symlink_if_present(path)?;
    let bytes = fs::read(path).map_err(StateError::Filesystem)?;
    if blake3::hash(&bytes).to_hex().as_str() == expected_digest {
        Ok(())
    } else {
        Err(StateError::Integrity)
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StateError> {
    reject_symlink_if_present(path)?;
    let parent = path.parent().ok_or(StateError::InvalidRoot)?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".forge-{}.{}.tmp", std::process::id(), sequence));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if fs::read(path)? == bytes {
                    Ok(())
                } else {
                    Err(StateError::Integrity)
                }
            }
            Err(error) => Err(StateError::Filesystem(error)),
        }
    })();
    let cleanup = fs::remove_file(&temporary).map_err(StateError::Filesystem);
    result?;
    cleanup
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    fn digest(input: &str) -> String {
        blake3::hash(input.as_bytes()).to_hex().to_string()
    }

    fn usage_record(usage_id: &str) -> PersistedResourceUsageRecord {
        PersistedResourceUsageRecord {
            usage_id: digest(usage_id),
            pool: ResourceUsagePool::Ordinary,
            owner: "forge.command".into(),
            category: "provider.inference".into(),
            used: PersistedResourceVector {
                tokens: 37,
                cost_micros: 125,
                ..Default::default()
            },
            cache_tokens_reused: 12,
            attribution: ResourceUsageAttribution {
                project_id: "hive-forge".into(),
                work_order_id: "FGE-004-M00".into(),
                execution_id: "exec-001".into(),
                capability_id: "forge.ai.inference".into(),
                provider_id: Some("provider.test".into()),
            },
        }
    }

    fn usage_limit(tokens: u64, cost_micros: u64) -> PersistedResourceVector {
        PersistedResourceVector {
            tokens,
            cost_micros,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn credential_canary_never_reaches_canonical_state_receipts_outbox_cache_or_replay() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let canary = "ghp_C03CANARY0123456789ABCDEF";
        let bearer = format!("Bearer {canary}");
        let payload = json!({"message": bearer});
        for field in [
            "github_token",
            "openai_api_key",
            "oauth_secret",
            "ssh_private_key",
            "aws_secret_access_key",
            "signingKey",
            "credentials",
            "client_secret_value",
            "clientSecretValue",
            "accessTokenMetadata",
        ] {
            let keyed_secret = json!({(field): "opaque-reference-value"});
            assert!(matches!(
                validate_payload_for_persistence(&keyed_secret),
                Err(StateError::SensitivePayload)
            ));
        }
        for keyed_secret in [
            json!({"credentials": 123}),
            json!({"api_token": 123}),
            json!({"signingKey": 123}),
        ] {
            assert!(matches!(
                validate_payload_for_persistence(&keyed_secret),
                Err(StateError::SensitivePayload)
            ));
        }
        assert!(validate_payload_for_persistence(&json!({"tokens": 37})).is_ok());
        assert!(validate_payload_for_persistence(&json!({"cacheTokensReused": 12})).is_ok());
        assert!(validate_payload_for_persistence(&json!({"monkey": "ordinary value"})).is_ok());

        let canonical_error = store
            .put_canonical("project", "private", &payload)
            .await
            .expect_err("credential-like state is rejected");
        assert!(matches!(canonical_error, StateError::SensitivePayload));
        assert!(!canonical_error.to_string().contains(canary));
        assert!(matches!(
            store
                .compare_and_set_canonical("project", "private-cas", None, &payload)
                .await,
            Err(StateError::SensitivePayload)
        ));

        let receipt_identity = digest("secret-canary-receipt");
        assert!(
            store
                .begin_idempotent(&receipt_identity)
                .await
                .expect("begin command")
                .is_none()
        );
        assert!(matches!(
            store
                .commit_command(&receipt_identity, &payload, None, &[])
                .await,
            Err(StateError::SensitivePayload)
        ));
        let receipt = store
            .begin_idempotent(&receipt_identity)
            .await
            .expect("read command intent")
            .expect("in-flight intent remains without a receipt");
        assert_eq!(receipt.state, IdempotencyState::InFlight);
        assert_eq!(receipt.result, None);
        assert!(matches!(
            store
                .fail_idempotent(&receipt_identity, IdempotencyState::UnknownOutcome, canary,)
                .await,
            Err(StateError::InvalidKey)
        ));
        assert_eq!(
            store
                .begin_idempotent(&receipt_identity)
                .await
                .expect("rejected error code does not persist")
                .expect("intent remains active")
                .state,
            IdempotencyState::InFlight
        );

        assert!(matches!(
            store
                .enqueue_event(&StoredEvent {
                    event_id: "evt.secret-canary".into(),
                    contract_id: "forge.event.private".into(),
                    payload: payload.clone(),
                })
                .await,
            Err(StateError::SensitivePayload)
        ));
        assert!(matches!(
            store
                .record_evidence(&EvidenceRecord {
                    evidence_id: "evidence.secret-canary".into(),
                    kind: "security".into(),
                    fingerprint: digest("secret-evidence"),
                    payload: payload.clone(),
                })
                .await,
            Err(StateError::SensitivePayload)
        ));
        assert!(
            store
                .pending_events_for_consumer("test-observer", 10)
                .await
                .expect("event replay")
                .is_empty()
        );

        assert!(matches!(
            store
                .put_cache("test", "secret-canary", &digest("cache-key"), &payload)
                .await,
            Err(StateError::SensitivePayload)
        ));
        assert_eq!(
            store
                .get_cache("test", "secret-canary", &digest("cache-key"))
                .await
                .expect("cache lookup"),
            None
        );

        for entry in fs::read_dir(root.path()).expect("list state files") {
            let path = entry.expect("state file").path();
            if path.is_file() {
                let bytes = fs::read(path).expect("read state file");
                assert!(
                    !bytes
                        .windows(canary.len())
                        .any(|window| window == canary.as_bytes()),
                    "credential canary must not be persisted"
                );
            }
        }
    }

    #[tokio::test]
    async fn canonical_state_survives_while_disposable_cache_can_be_cleared() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        store
            .put_canonical("kernel", "boot.phase", &json!("native_ready"))
            .await
            .expect("canonical write");
        store
            .put_cache("context", "entry-1", &digest("v1"), &json!({"value": 1}))
            .await
            .expect("cache write");
        assert_eq!(store.clear_cache().await.expect("clear cache"), 1);
        assert_eq!(
            store
                .get_cache("context", "entry-1", &digest("v1"))
                .await
                .expect("cache lookup"),
            None
        );
        assert_eq!(
            store
                .get_canonical("kernel", "boot.phase")
                .await
                .expect("canonical read"),
            Some(json!("native_ready"))
        );
    }

    #[tokio::test]
    async fn resource_usage_is_durable_idempotent_and_attributed() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        let record = usage_record("request-1");
        let limit = usage_limit(100, 500);

        assert_eq!(store.schema_version(), 7);
        assert!(
            store
                .record_resource_usage(&record, &limit)
                .await
                .expect("first charge is inserted")
        );
        assert!(
            !store
                .record_resource_usage(&record, &limit)
                .await
                .expect("replay is deduplicated")
        );

        let mut conflicting = record.clone();
        conflicting.used.tokens += 1;
        assert!(matches!(
            store.record_resource_usage(&conflicting, &limit).await,
            Err(StateError::ResourceUsageConflict)
        ));
        assert_eq!(
            store.resource_usage_records().await.expect("ledger read"),
            vec![record.clone()]
        );

        drop(store);
        let reopened = ForgeStateStore::open(root.path())
            .await
            .expect("reopen store");
        assert_eq!(
            reopened
                .resource_usage_records()
                .await
                .expect("recovered ledger"),
            vec![record]
        );
    }

    #[tokio::test]
    async fn concurrent_resource_usage_replays_insert_one_immutable_row() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        let record = usage_record("concurrent-request");
        let limit = usage_limit(100, 500);
        let first = store.record_resource_usage(&record, &limit);
        let second = store.record_resource_usage(&record, &limit);
        let (first, second) = tokio::join!(first, second);
        let outcomes = [
            first.expect("first write"),
            second.expect("concurrent replay"),
        ];
        assert_eq!(outcomes.iter().filter(|inserted| **inserted).count(), 1);
        assert_eq!(
            store
                .resource_usage_records()
                .await
                .expect("single ledger row"),
            vec![record]
        );
    }

    #[tokio::test]
    async fn concurrent_distinct_usage_cannot_exceed_the_durable_pool_ceiling() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        let limit = usage_limit(37, 125);
        let first = usage_record("budget-race-a");
        let second = usage_record("budget-race-b");
        let (first, second) = tokio::join!(
            store.record_resource_usage(&first, &limit),
            store.record_resource_usage(&second, &limit)
        );
        let results = [first, second];
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Ok(true)))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(StateError::ResourceBudgetExceeded)))
                .count(),
            1
        );
        assert_eq!(
            store
                .resource_usage_records()
                .await
                .expect("bounded ledger")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn resource_ledger_limit_is_shared_across_store_instances() {
        let root = tempfile::tempdir().expect("temp directory");
        let first = ForgeStateStore::open(root.path())
            .await
            .expect("first store");
        let second = ForgeStateStore::open(root.path())
            .await
            .expect("second store");
        sqlx::query("UPDATE resource_usage_totals SET record_count=99999 WHERE pool='ordinary'")
            .execute(&first.pool)
            .await
            .expect("arrange shared ledger near capacity");

        let a = usage_record("global-cap-a");
        let b = usage_record("global-cap-b");
        let limit = usage_limit(100, 500);
        let (a_result, b_result) = tokio::join!(
            first.record_resource_usage(&a, &limit),
            second.record_resource_usage(&b, &limit)
        );
        let results = [a_result, b_result];
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Ok(true)))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(StateError::ResourceLedgerLimit)))
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn opening_a_second_store_preserves_live_intents_and_recovers_stale_owners() {
        let live_root = tempfile::tempdir().expect("live temp directory");
        let live_owner = ForgeStateStore::open(live_root.path())
            .await
            .expect("first live store");
        let identity = digest("live-intent");
        assert!(
            live_owner
                .begin_idempotent(&identity)
                .await
                .expect("begin live intent")
                .is_none()
        );

        let concurrent_store = ForgeStateStore::open(live_root.path())
            .await
            .expect("second live store");
        assert_eq!(
            concurrent_store
                .begin_idempotent(&identity)
                .await
                .expect("live intent remains visible")
                .expect("existing intent"),
            IdempotencyRecord {
                state: IdempotencyState::InFlight,
                result: None,
                error_code: None,
                latest_resolution: None,
            }
        );

        let stale_root = tempfile::tempdir().expect("stale temp directory");
        let stale_owner = ForgeStateStore::open(stale_root.path())
            .await
            .expect("initial store");
        let stale_identity = digest("stale-intent");
        assert!(
            stale_owner
                .begin_idempotent(&stale_identity)
                .await
                .expect("begin stale intent")
                .is_none()
        );
        sqlx::query("UPDATE store_instances SET heartbeat_at_ms=0 WHERE instance_id=?1")
            .bind(&stale_owner.owner.instance_id)
            .execute(&stale_owner.pool)
            .await
            .expect("simulate stale owner heartbeat");

        let recovering_store = ForgeStateStore::open(stale_root.path())
            .await
            .expect("recovery store");
        assert_eq!(
            recovering_store
                .begin_idempotent(&stale_identity)
                .await
                .expect("stale intent is readable")
                .expect("recovered intent")
                .state,
            IdempotencyState::UnknownOutcome
        );
    }

    #[tokio::test]
    async fn concurrent_store_instances_admit_one_intent_owner_and_one_durable_effect() {
        let root = tempfile::tempdir().expect("concurrent command state root");
        let first = ForgeStateStore::open(root.path())
            .await
            .expect("first store opens");
        let second = ForgeStateStore::open(root.path())
            .await
            .expect("second store opens");
        first
            .put_canonical("project", "setting", &json!({"value": 1}))
            .await
            .expect("initial canonical state");
        let identity = digest("shared-concurrent-intent");
        let (first_admission, second_admission) = tokio::join!(
            first.begin_idempotent(&identity),
            second.begin_idempotent(&identity)
        );
        let first_admission = first_admission.expect("first admission result");
        let second_admission = second_admission.expect("second admission result");
        assert_ne!(
            first_admission.is_none(),
            second_admission.is_none(),
            "exactly one store instance must own the new intent"
        );
        let (owner, duplicate) = if first_admission.is_none() {
            (&first, &second)
        } else {
            (&second, &first)
        };
        owner
            .commit_command(
                &identity,
                &json!({"receipt": "committed"}),
                Some(&CanonicalStateTransition {
                    owner: "project".into(),
                    key: "setting".into(),
                    expected_version: Some(1),
                    value: json!({"value": 2}),
                }),
                &[StoredEvent {
                    event_id: "evt.concurrent.once".into(),
                    contract_id: "forge.event.changed".into(),
                    payload: json!({"value": 2}),
                }],
            )
            .await
            .expect("current owner commits once");
        assert!(matches!(
            duplicate
                .commit_command(
                    &identity,
                    &json!({"receipt": "duplicate"}),
                    Some(&CanonicalStateTransition {
                        owner: "project".into(),
                        key: "setting".into(),
                        expected_version: Some(1),
                        value: json!({"value": 3}),
                    }),
                    &[StoredEvent {
                        event_id: "evt.concurrent.once".into(),
                        contract_id: "forge.event.changed".into(),
                        payload: json!({"value": 3}),
                    }],
                )
                .await,
            Err(StateError::StateConflict)
        ));
        assert_eq!(
            owner
                .get_canonical("project", "setting")
                .await
                .expect("committed canonical state"),
            Some(json!({"value": 2}))
        );
        assert_eq!(
            owner
                .pending_events_for_consumer("test-observer", 10)
                .await
                .expect("one outbox fact")
                .len(),
            1
        );
        assert_eq!(
            duplicate
                .begin_idempotent(&identity)
                .await
                .expect("read committed identity")
                .expect("one durable receipt")
                .state,
            IdempotencyState::Committed
        );
    }

    #[tokio::test]
    async fn command_state_outbox_and_receipt_commit_atomically() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        store
            .put_canonical("project", "setting", &json!({"value": 1}))
            .await
            .expect("seed canonical state");
        let identity = digest("command-atomic");
        assert!(
            store
                .begin_idempotent(&identity)
                .await
                .expect("begin command")
                .is_none()
        );
        store
            .enqueue_event(&StoredEvent {
                event_id: "evt.conflict".into(),
                contract_id: "forge.event.changed".into(),
                payload: json!({"value": "prior"}),
            })
            .await
            .expect("seed conflicting outbox event");

        let conflicting = StoredEvent {
            event_id: "evt.conflict".into(),
            contract_id: "forge.event.changed".into(),
            payload: json!({"value": "new"}),
        };
        assert!(matches!(
            store
                .commit_command(
                    &identity,
                    &json!({"receipt": "new"}),
                    Some(&CanonicalStateTransition {
                        owner: "project".into(),
                        key: "setting".into(),
                        expected_version: Some(1),
                        value: json!({"value": 2}),
                    }),
                    &[conflicting],
                )
                .await,
            Err(StateError::StateConflict)
        ));
        assert_eq!(
            store
                .get_canonical("project", "setting")
                .await
                .expect("state"),
            Some(json!({"value": 1}))
        );
        assert_eq!(
            store
                .pending_events_for_consumer("test-observer", 10)
                .await
                .expect("outbox")
                .len(),
            1
        );
        assert_eq!(
            store
                .begin_idempotent(&identity)
                .await
                .expect("receipt remains in flight")
                .expect("in-flight receipt")
                .state,
            IdempotencyState::InFlight
        );

        store
            .commit_command(
                &identity,
                &json!({"receipt": "committed"}),
                Some(&CanonicalStateTransition {
                    owner: "project".into(),
                    key: "setting".into(),
                    expected_version: Some(1),
                    value: json!({"value": 2}),
                }),
                &[StoredEvent {
                    event_id: "evt.atomic".into(),
                    contract_id: "forge.event.changed".into(),
                    payload: json!({"value": 2}),
                }],
            )
            .await
            .expect("atomic command commit");
        assert_eq!(
            store
                .get_canonical("project", "setting")
                .await
                .expect("state"),
            Some(json!({"value": 2}))
        );
        assert_eq!(
            store
                .pending_events_for_consumer("test-observer", 10)
                .await
                .expect("outbox")
                .len(),
            2
        );
        assert_eq!(
            store
                .begin_idempotent(&identity)
                .await
                .expect("committed receipt")
                .expect("receipt record")
                .state,
            IdempotencyState::Committed
        );
    }

    #[tokio::test]
    async fn confirmed_effect_finalization_is_recoverable_and_keeps_state_outbox_and_receipt_atomic()
     {
        let root = tempfile::tempdir().expect("temp directory");
        let identity = digest("confirmed-effect-finalization");
        let result = json!({
            "value": 2,
            "_forgeCommandReceipt": {
                "commitEvidenceFingerprint": digest("external-effect-committed")
            }
        });
        let event = StoredEvent {
            event_id: "evt.effect-confirmed".into(),
            contract_id: "forge.event.effect-confirmed".into(),
            payload: json!({"producer": "kernel.command", "value": 2}),
        };
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        store
            .put_canonical("project", "state", &json!({"value": 1}))
            .await
            .expect("seed canonical state");
        assert!(
            store
                .begin_idempotent(&identity)
                .await
                .expect("begin effectful command")
                .is_none()
        );
        store
            .stage_effect_finalization(
                &identity,
                &result,
                Some(&CanonicalStateTransition {
                    owner: "project".into(),
                    key: "state".into(),
                    expected_version: Some(99),
                    value: json!({"value": 2}),
                }),
                std::slice::from_ref(&event),
            )
            .await
            .expect("persist effect result before local finalization");
        assert!(matches!(
            store.finalize_effect_committed(&identity).await,
            Err(StateError::StateConflict)
        ));
        assert_eq!(
            store
                .get_canonical("project", "state")
                .await
                .expect("state remains unchanged"),
            Some(json!({"value": 1}))
        );
        assert!(
            store
                .pending_events_for_consumer("consumer-a", 10)
                .await
                .expect("no event before the atomic commit")
                .is_empty()
        );
        assert_eq!(
            store
                .begin_idempotent(&identity)
                .await
                .expect("read intent")
                .expect("intent remains durable")
                .state,
            IdempotencyState::EffectCommittedPendingFinalization
        );
        assert!(matches!(
            store
                .reconcile_unknown_outcome(
                    &identity,
                    CommandOutcomeResolutionKind::EffectNotCommitted,
                    &digest("contradictory-not-committed"),
                    "decision.contradict.effect-not-committed",
                    &digest("decision.contradict.effect-not-committed"),
                    1,
                )
                .await,
            Err(StateError::StateConflict)
        ));

        let pending = store
            .effect_finalization(&identity)
            .await
            .expect("read staged effect")
            .expect("pending finalization exists");
        let revised_transition = CanonicalStateTransition {
            owner: "project".into(),
            key: "state".into(),
            expected_version: Some(1),
            value: json!({"value": 2}),
        };
        let revised_fingerprint = store
            .revise_effect_finalization(
                &identity,
                &pending.fingerprint,
                Some(&revised_transition),
                "decision.effect-finalization.revision-1",
                &digest("decision.effect-finalization.revision-1"),
                2,
            )
            .await
            .expect("host-authorized revision reaches the state boundary");
        assert_ne!(revised_fingerprint, pending.fingerprint);
        let revision_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM command_finalization_revisions WHERE identity=?1",
        )
        .bind(&identity)
        .fetch_one(&store.pool)
        .await
        .expect("read immutable revision audit");
        assert_eq!(revision_count, 1);
        assert!(matches!(
            store
                .revise_effect_finalization(
                    &identity,
                    &revised_fingerprint,
                    Some(&CanonicalStateTransition {
                        owner: "project".into(),
                        key: "state".into(),
                        expected_version: Some(2),
                        value: json!({"value": 3}),
                    }),
                    "decision.effect-finalization.revision-1",
                    &digest("decision.effect-finalization.revision-1"),
                    3,
                )
                .await,
            Err(StateError::AuthorizationDecisionRejected)
        ));

        drop(store);
        let recovered = ForgeStateStore::open(root.path())
            .await
            .expect("reopen pending finalization");
        assert_eq!(
            recovered
                .effect_finalization(&identity)
                .await
                .expect("read reopened staged effect")
                .expect("staged effect survives reopen")
                .fingerprint,
            revised_fingerprint
        );
        assert!(
            recovered
                .finalize_effect_committed(&identity)
                .await
                .expect("resume local finalization after reopen")
        );
        assert_eq!(
            recovered
                .get_canonical("project", "state")
                .await
                .expect("read committed state"),
            Some(json!({"value": 2}))
        );
        assert_eq!(
            recovered
                .pending_events_for_consumer("consumer-a", 10)
                .await
                .expect("consumer a event")
                .iter()
                .map(|record| record.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["evt.effect-confirmed"]
        );
        assert_eq!(
            recovered
                .pending_events_for_consumer("consumer-b", 10)
                .await
                .expect("consumer b has independent event state")
                .len(),
            1
        );
        let receipt = recovered
            .begin_idempotent(&identity)
            .await
            .expect("read durable receipt")
            .expect("committed command receipt");
        assert_eq!(receipt.state, IdempotencyState::Committed);
        assert_eq!(receipt.result, Some(result));
        assert!(
            recovered
                .effect_finalization(&identity)
                .await
                .expect("read finalization record")
                .expect("finalization record")
                .finalized_at_ms
                .is_some()
        );
        assert!(
            !recovered
                .finalize_effect_committed(&identity)
                .await
                .expect("duplicate finalization is idempotent")
        );
        assert_eq!(
            recovered
                .pending_events_for_consumer("consumer-a", 10)
                .await
                .expect("one immutable outbox event")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn command_finalization_process_crashes_recover_atomically_at_each_write_boundary() {
        let identity = digest("confirmed-effect-finalization-crash");
        if let Ok(root) = std::env::var("FORGE_TEST_FINALIZATION_CRASH_ROOT") {
            let store = ForgeStateStore::open(root)
                .await
                .expect("child crash store opens");
            store
                .finalize_effect_committed(&identity)
                .await
                .expect("child finalizes staged effect before boundary exit");
            panic!("requested finalization boundary did not terminate the child process");
        }

        for boundary in [
            "finalize_state_cas",
            "finalize_outbox_1",
            "finalize_outbox_2",
            "finalize_receipt",
            "finalize_marker",
            "finalize_after_commit",
        ] {
            let root = tempfile::tempdir().expect("process-crash state root");
            let result = json!({
                "value": 2,
                "_forgeCommandReceipt": {
                    "commitEvidenceFingerprint": digest("crash-boundary-effect")
                }
            });
            let store = ForgeStateStore::open(root.path())
                .await
                .expect("prepare crash state");
            store
                .put_canonical("project", "state", &json!({"value": 1}))
                .await
                .expect("seed state before command");
            assert!(
                store
                    .begin_idempotent(&identity)
                    .await
                    .expect("begin effect")
                    .is_none()
            );
            store
                .stage_effect_finalization(
                    &identity,
                    &result,
                    Some(&CanonicalStateTransition {
                        owner: "project".into(),
                        key: "state".into(),
                        expected_version: Some(1),
                        value: json!({"value": 2}),
                    }),
                    &[
                        StoredEvent {
                            event_id: "evt.finalize.crash.one".into(),
                            contract_id: "forge.event.confirmed".into(),
                            payload: json!({"producer": "kernel.command", "sequence": 1}),
                        },
                        StoredEvent {
                            event_id: "evt.finalize.crash.two".into(),
                            contract_id: "forge.event.confirmed".into(),
                            payload: json!({"producer": "kernel.command", "sequence": 2}),
                        },
                    ],
                )
                .await
                .expect("stage effect before crashable finalization");
            drop(store);

            let child = Command::new(std::env::current_exe().expect("current test executable"))
                .arg("command_finalization_process_crashes_recover_atomically_at_each_write_boundary")
                .arg("--nocapture")
                .env("FORGE_TEST_FINALIZATION_CRASH_ROOT", root.path())
                .env("FORGE_TEST_COMMIT_CRASH_BOUNDARY", boundary)
                .output()
                .expect("finalization crash child starts");
            assert_eq!(
                child.status.code(),
                Some(86),
                "child did not exit at requested finalization boundary {boundary}: {}",
                String::from_utf8_lossy(&child.stderr)
            );

            let recovered = ForgeStateStore::open(root.path())
                .await
                .expect("reopen after finalization crash");
            let committed = boundary == "finalize_after_commit";
            if committed {
                assert_eq!(
                    recovered
                        .get_canonical("project", "state")
                        .await
                        .expect("committed state"),
                    Some(json!({"value": 2}))
                );
                assert_eq!(
                    recovered
                        .begin_idempotent(&identity)
                        .await
                        .expect("read committed receipt")
                        .expect("receipt")
                        .state,
                    IdempotencyState::Committed
                );
                assert!(
                    recovered
                        .effect_finalization(&identity)
                        .await
                        .expect("read finalization")
                        .expect("finalization")
                        .finalized_at_ms
                        .is_some()
                );
                assert_eq!(
                    recovered
                        .pending_events_for_consumer("crash-test-consumer", 10)
                        .await
                        .expect("committed outbox events")
                        .len(),
                    2
                );
                assert!(
                    !recovered
                        .finalize_effect_committed(&identity)
                        .await
                        .expect("committed replay is a no-op")
                );
            } else {
                assert_eq!(
                    recovered
                        .get_canonical("project", "state")
                        .await
                        .expect("uncommitted state"),
                    Some(json!({"value": 1}))
                );
                assert_eq!(
                    recovered
                        .begin_idempotent(&identity)
                        .await
                        .expect("read pending receipt")
                        .expect("pending intent")
                        .state,
                    IdempotencyState::EffectCommittedPendingFinalization
                );
                assert!(
                    recovered
                        .pending_events_for_consumer("crash-test-consumer", 10)
                        .await
                        .expect("no partial outbox events")
                        .is_empty()
                );
                assert!(
                    recovered
                        .finalize_effect_committed(&identity)
                        .await
                        .expect("retry local finalization after rollback")
                );
                assert_eq!(
                    recovered
                        .get_canonical("project", "state")
                        .await
                        .expect("recovered state"),
                    Some(json!({"value": 2}))
                );
                assert_eq!(
                    recovered
                        .pending_events_for_consumer("crash-test-consumer", 10)
                        .await
                        .expect("all outbox events committed on retry")
                        .len(),
                    2
                );
            }
        }
    }

    #[tokio::test]
    async fn command_commit_process_crashes_recover_atomically_at_each_write_boundary() {
        const IDENTITY: &str = "b";
        let identity = blake3::hash(IDENTITY.as_bytes()).to_hex().to_string();
        if let Ok(root) = std::env::var("FORGE_TEST_COMMIT_CRASH_ROOT") {
            let store = ForgeStateStore::open(root)
                .await
                .expect("child crash store opens");
            store
                .put_canonical("project", "setting", &json!({"value": 1}))
                .await
                .expect("child seeds state");
            assert!(
                store
                    .begin_idempotent(&identity)
                    .await
                    .expect("child admits intent")
                    .is_none()
            );
            store
                .commit_command(
                    &identity,
                    &json!({"receipt": "committed"}),
                    Some(&CanonicalStateTransition {
                        owner: "project".into(),
                        key: "setting".into(),
                        expected_version: Some(1),
                        value: json!({"value": 2}),
                    }),
                    &[
                        StoredEvent {
                            event_id: "evt.crash.one".into(),
                            contract_id: "forge.event.changed".into(),
                            payload: json!({"sequence": 1}),
                        },
                        StoredEvent {
                            event_id: "evt.crash.two".into(),
                            contract_id: "forge.event.changed".into(),
                            payload: json!({"sequence": 2}),
                        },
                    ],
                )
                .await
                .expect("child command transaction completes before requested boundary exit");
            panic!("requested transaction boundary did not terminate the child process");
        }

        for boundary in [
            "state_cas",
            "outbox_1",
            "outbox_2",
            "receipt",
            "after_commit",
        ] {
            let root = tempfile::tempdir().expect("process-crash state root");
            let child = std::process::Command::new(
                std::env::current_exe().expect("current test executable"),
            )
            .arg("command_commit_process_crashes_recover_atomically_at_each_write_boundary")
            .arg("--nocapture")
            .env("FORGE_TEST_COMMIT_CRASH_ROOT", root.path())
            .env("FORGE_TEST_COMMIT_CRASH_BOUNDARY", boundary)
            .output()
            .expect("crash child process starts");
            assert_eq!(
                child.status.code(),
                Some(86),
                "child did not exit at requested boundary {boundary}: {}",
                String::from_utf8_lossy(&child.stderr)
            );

            let store = ForgeStateStore::open(root.path())
                .await
                .expect("reopen after abrupt process exit");
            let committed = boundary == "after_commit";
            assert_eq!(
                store
                    .get_canonical("project", "setting")
                    .await
                    .expect("canonical state after recovery"),
                Some(if committed {
                    json!({"value": 2})
                } else {
                    json!({"value": 1})
                }),
                "state diverged at {boundary}"
            );
            assert_eq!(
                store
                    .pending_events_for_consumer("test-observer", 10)
                    .await
                    .expect("outbox after recovery")
                    .len(),
                if committed { 2 } else { 0 },
                "outbox diverged at {boundary}"
            );

            if committed {
                assert_eq!(
                    store
                        .begin_idempotent(&identity)
                        .await
                        .expect("committed receipt lookup")
                        .expect("committed receipt")
                        .state,
                    IdempotencyState::Committed
                );
            } else {
                sqlx::query("UPDATE store_instances SET heartbeat_at_ms=0 WHERE instance_id != ?1")
                    .bind(&store.owner.instance_id)
                    .execute(&store.pool)
                    .await
                    .expect("age crashed owner heartbeat");
                sqlx::query("UPDATE command_intents SET lease_expires_at_ms=0 WHERE identity=?1")
                    .bind(&identity)
                    .execute(&store.pool)
                    .await
                    .expect("expire crashed command lease");
                assert_eq!(
                    store
                        .begin_idempotent(&identity)
                        .await
                        .expect("stale outcome lookup")
                        .expect("stale outcome")
                        .state,
                    IdempotencyState::UnknownOutcome,
                    "pre-commit crash at {boundary} must remain unreplayed until reconciliation"
                );
            }
        }
    }

    #[tokio::test]
    async fn resource_usage_schema_migrates_from_v1_without_recreating_canonical_state() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        store
            .put_canonical("kernel", "migration.marker", &json!("preserved"))
            .await
            .expect("canonical row");
        sqlx::query("DROP TABLE resource_usage")
            .execute(&store.pool)
            .await
            .expect("simulate v1 schema");
        sqlx::query("PRAGMA user_version = 1")
            .execute(&store.pool)
            .await
            .expect("set v1 schema version");

        store
            .migrate()
            .await
            .expect("v1 to current schema migration");
        store.integrity_check().await.expect("migrated integrity");
        assert_eq!(store.schema_version(), 7);
        assert_eq!(
            store
                .get_canonical("kernel", "migration.marker")
                .await
                .expect("preserved canonical read"),
            Some(json!("preserved"))
        );
        assert!(
            store
                .record_resource_usage(&usage_record("after-migration"), &usage_limit(100, 500))
                .await
                .expect("usage table available after migration")
        );
    }

    #[tokio::test]
    async fn schema_v3_migrates_outcome_resolution_ledger_without_losing_intents() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        let identity = digest("schema-v3-intent");
        assert!(
            store
                .begin_idempotent(&identity)
                .await
                .expect("intent")
                .is_none()
        );
        store
            .fail_idempotent(
                &identity,
                IdempotencyState::UnknownOutcome,
                "FORGE.COMMAND.UNKNOWN_OUTCOME",
            )
            .await
            .expect("mark ambiguous intent");
        sqlx::query("DROP TABLE command_outcome_resolutions")
            .execute(&store.pool)
            .await
            .expect("simulate schema v3");
        sqlx::query("PRAGMA user_version = 3")
            .execute(&store.pool)
            .await
            .expect("set v3 schema version");

        store.migrate().await.expect("v3 to v4 migration");
        assert_eq!(store.schema_version(), 7);
        store.integrity_check().await.expect("migrated integrity");
        let intent = store
            .begin_idempotent(&identity)
            .await
            .expect("intent survives migration")
            .expect("ambiguous intent");
        assert_eq!(intent.state, IdempotencyState::UnknownOutcome);
        assert_eq!(intent.latest_resolution, None);
        let table: Option<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='command_outcome_resolutions'",
        )
        .fetch_optional(&store.pool)
        .await
        .expect("resolution table query");
        assert_eq!(table.as_deref(), Some("command_outcome_resolutions"));
    }

    #[tokio::test]
    async fn process_termination_rolls_back_an_uncommitted_canonical_state_write() {
        const CRASH_ROOT: &str = "FORGE_STATE_CRASH_TEST_ROOT";
        if let Some(root) = std::env::var_os(CRASH_ROOT) {
            let store = ForgeStateStore::open(root).await.expect("child store");
            let mut transaction = store.pool.begin().await.expect("child transaction");
            sqlx::query(
                "INSERT INTO canonical_state(owner, key, value, version) VALUES('fault', 'partial', X'7B7D', 1)",
            )
            .execute(&mut *transaction)
            .await
            .expect("uncommitted write");
            println!("FORGE_STATE_CRASH_READY");
            std::io::stdout().flush().expect("flush ready marker");
            std::thread::sleep(Duration::from_secs(30));
            return;
        }

        let root = tempfile::tempdir().expect("temporary state");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("initial store");
        store.pool.close().await;
        drop(store);
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .arg("--exact")
            .arg(
                "store::tests::process_termination_rolls_back_an_uncommitted_canonical_state_write",
            )
            .arg("--nocapture")
            .env(CRASH_ROOT, root.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("fault child starts");
        let stdout = child.stdout.take().expect("child stdout");
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        loop {
            match receiver.recv_timeout(Duration::from_secs(10)) {
                Ok(line) if line == "FORGE_STATE_CRASH_READY" => break,
                Ok(_) => continue,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("child did not announce its open transaction: {error}");
                }
            }
        }
        child
            .kill()
            .expect("terminate child during open transaction");
        assert!(!child.wait().expect("child exits").success());
        reader.join().expect("stdout reader exits");

        let recovered = ForgeStateStore::open(root.path())
            .await
            .expect("reopen after process termination");
        assert_eq!(
            recovered
                .get_canonical("fault", "partial")
                .await
                .expect("read recovered state"),
            None
        );
        recovered
            .integrity_check()
            .await
            .expect("recovered database integrity");
    }

    #[tokio::test]
    async fn canonical_compare_and_set_serializes_competing_transitions() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        assert!(
            store
                .compare_and_set_canonical("kernel", "counter", None, &json!(1))
                .await
                .expect("first insert")
        );
        assert!(
            !store
                .compare_and_set_canonical("kernel", "counter", None, &json!(2))
                .await
                .expect("duplicate insert")
        );
        assert!(
            !store
                .compare_and_set_canonical("kernel", "counter", Some(9), &json!(2))
                .await
                .expect("stale version")
        );
        assert!(
            store
                .compare_and_set_canonical("kernel", "counter", Some(1), &json!(2))
                .await
                .expect("current version")
        );
        assert_eq!(
            store
                .get_canonical("kernel", "counter")
                .await
                .expect("read"),
            Some(json!(2))
        );
    }

    #[tokio::test]
    async fn idempotency_does_not_reexecute_an_existing_or_ambiguous_command() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        let identity = digest("semantic command");
        assert_eq!(
            store
                .begin_idempotent(&identity)
                .await
                .expect("first admission"),
            None
        );
        store
            .fail_idempotent(
                &identity,
                IdempotencyState::UnknownOutcome,
                "FORGE.COMMAND.UNKNOWN_OUTCOME",
            )
            .await
            .expect("unknown outcome recorded");
        let prior = store
            .begin_idempotent(&identity)
            .await
            .expect("dedupe lookup")
            .expect("existing command");
        assert_eq!(prior.state, IdempotencyState::UnknownOutcome);
        assert_eq!(prior.result, None);
    }

    #[tokio::test]
    async fn durable_event_acknowledgements_are_independent_per_consumer() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        let event = StoredEvent {
            event_id: "evt-1".into(),
            contract_id: "forge.event.test".into(),
            payload: json!({"ok": true}),
        };
        store.enqueue_event(&event).await.expect("append event");
        store
            .enqueue_event(&event)
            .await
            .expect("identical retry is idempotent");
        assert_eq!(
            store
                .pending_events_for_consumer("consumer-a", 10)
                .await
                .expect("read pending")
                .len(),
            1
        );
        assert!(
            store
                .pending_events_for_consumer("consumer-b", 10)
                .await
                .expect("other consumer still pending")
                .iter()
                .any(|record| record.event_id == "evt-1")
        );
        assert!(
            store
                .acknowledge_event_for_consumer("evt-1", "consumer-a")
                .await
                .expect("ack consumer a")
        );
        assert!(
            !store
                .acknowledge_event_for_consumer("evt-1", "consumer-a")
                .await
                .expect("second ack is a no-op")
        );
        assert!(
            store
                .pending_events_for_consumer("consumer-a", 10)
                .await
                .expect("consumer a acknowledged")
                .is_empty()
        );
        assert_eq!(
            store
                .pending_events_for_consumer("consumer-b", 10)
                .await
                .expect("consumer b remains pending")
                .len(),
            1
        );
        assert!(
            store
                .acknowledge_event_for_consumer("evt-1", "consumer-b")
                .await
                .expect("ack consumer b")
        );
        assert!(
            store
                .pending_events_for_consumer("consumer-b", 10)
                .await
                .expect("consumer b acknowledged")
                .is_empty()
        );
        store.pool.close().await;
        drop(store);
        let reopened = ForgeStateStore::open(root.path())
            .await
            .expect("reopen acknowledged outbox");
        assert!(
            reopened
                .pending_events_for_consumer("consumer-a", 10)
                .await
                .expect("ack remains durable after restart")
                .is_empty()
        );
        assert!(
            reopened
                .pending_events_for_consumer("consumer-b", 10)
                .await
                .expect("consumer b ack remains durable")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn outbox_preserves_per_producer_insert_order_not_event_id_lexical_order() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        for event_id in ["producer.event.2", "producer.event.10"] {
            store
                .enqueue_event(&StoredEvent {
                    event_id: event_id.into(),
                    contract_id: "forge.event.ordered".into(),
                    payload: json!({"producer": "forge.kernel", "eventId": event_id}),
                })
                .await
                .expect("append ordered producer event");
        }
        let events = store
            .pending_events_for_consumer("ordered-consumer", 10)
            .await
            .expect("ordered replay");
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_id.as_str())
                .collect::<Vec<_>>(),
            ["producer.event.2", "producer.event.10"]
        );
    }

    #[tokio::test]
    async fn reusing_an_event_id_for_different_content_is_rejected() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        store
            .enqueue_event(&StoredEvent {
                event_id: "event-1".into(),
                contract_id: "forge.event.test".into(),
                payload: json!({"value": 1}),
            })
            .await
            .expect("first event");
        let changed = StoredEvent {
            event_id: "event-1".into(),
            contract_id: "forge.event.test".into(),
            payload: json!({"value": 2}),
        };
        assert!(matches!(
            store.enqueue_event(&changed).await,
            Err(StateError::StateConflict)
        ));
    }

    #[tokio::test]
    async fn evidence_is_immutable_and_cas_checks_content_integrity() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        let evidence = EvidenceRecord {
            evidence_id: "proof-1".into(),
            kind: "test".into(),
            fingerprint: digest("head"),
            payload: json!({"passed": true}),
        };
        store
            .record_evidence(&evidence)
            .await
            .expect("record proof");
        store
            .record_evidence(&evidence)
            .await
            .expect("identical retry is idempotent");
        let mut changed = evidence.clone();
        changed.payload = json!({"passed": false});
        assert!(matches!(
            store.record_evidence(&changed).await,
            Err(StateError::EvidenceConflict)
        ));

        let blob_id = store
            .put_blob(b"immutable evidence")
            .expect("store CAS object");
        assert_eq!(
            store
                .get_blob(&blob_id)
                .expect("read CAS object")
                .as_deref(),
            Some(&b"immutable evidence"[..])
        );
    }

    #[tokio::test]
    async fn backup_manifest_is_verified_and_database_restores_into_a_new_root() {
        let root = tempfile::tempdir().expect("temp directory");
        let backup = tempfile::tempdir().expect("backup directory");
        let source = ForgeStateStore::open(root.path())
            .await
            .expect("store opens");
        source
            .put_canonical("kernel", "checkpoint", &json!("m00"))
            .await
            .expect("write state");
        let blob_digest = source
            .put_blob(b"backup content addressed evidence")
            .expect("store backup CAS object");
        let command_identity = digest("backup-unknown-command");
        assert!(
            source
                .begin_idempotent(&command_identity)
                .await
                .expect("begin command")
                .is_none()
        );
        source
            .fail_idempotent(
                &command_identity,
                IdempotencyState::UnknownOutcome,
                "FORGE.COMMAND.PROVIDER_STATUS_UNKNOWN",
            )
            .await
            .expect("record unknown outcome");
        source
            .reconcile_unknown_outcome(
                &command_identity,
                CommandOutcomeResolutionKind::EffectNotCommitted,
                &digest("backup-provider-evidence"),
                "resolution.backup",
                &digest("backup-signed-decision"),
                1_000,
            )
            .await
            .expect("record signed outcome resolution");
        let backup_digest = source
            .backup_to(backup.path())
            .await
            .expect("backup snapshot");
        let bytes = fs::read(backup.path().join("forge.sqlite3")).expect("backup database");
        let database_digest = blake3::hash(&bytes).to_hex().to_string();
        let manifest: BackupManifest = serde_json::from_slice(
            &fs::read(backup.path().join("manifest.json")).expect("backup manifest"),
        )
        .expect("parse backup manifest");
        assert_eq!(manifest.database_fingerprint, database_digest);
        assert_eq!(manifest.backup_fingerprint, backup_digest);
        assert_eq!(manifest.cas_objects.len(), 1);
        let restored_root = tempfile::tempdir().expect("restore directory");
        let restored = ForgeStateStore::restore_from_backup(
            backup.path(),
            restored_root.path(),
            &backup_digest,
        )
        .await
        .expect("restore verified backup");
        restored
            .integrity_check()
            .await
            .expect("candidate integrity");
        assert_eq!(
            restored
                .get_canonical("kernel", "checkpoint")
                .await
                .expect("read restored data"),
            Some(json!("m00"))
        );
        assert_eq!(
            restored
                .get_blob(&blob_digest)
                .expect("read restored CAS object")
                .as_deref(),
            Some(&b"backup content addressed evidence"[..])
        );
        let restored_intent = restored
            .begin_idempotent(&command_identity)
            .await
            .expect("read restored command intent")
            .expect("restored command intent");
        assert_eq!(restored_intent.state, IdempotencyState::FailedRetryable);
        assert_eq!(
            restored_intent
                .latest_resolution
                .expect("restored resolution evidence")
                .kind,
            CommandOutcomeResolutionKind::EffectNotCommitted
        );
    }

    #[tokio::test]
    async fn restore_refuses_a_database_that_does_not_match_its_manifest() {
        let source_root = tempfile::tempdir().expect("source");
        let backup_root = tempfile::tempdir().expect("backup");
        let restore_root = tempfile::tempdir().expect("restore");
        let source = ForgeStateStore::open(source_root.path())
            .await
            .expect("store");
        let trusted_fingerprint = source.backup_to(backup_root.path()).await.expect("backup");
        fs::write(backup_root.path().join("forge.sqlite3"), b"tampered").expect("tamper fixture");
        let manifest_path = backup_root.path().join("manifest.json");
        let mut manifest: BackupManifest =
            serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
                .expect("parse manifest");
        manifest.database_fingerprint = blake3::hash(b"tampered").to_hex().to_string();
        manifest.backup_fingerprint = backup_manifest_fingerprint(&BackupManifestUnsigned {
            schema_version: manifest.schema_version,
            database_schema_version: manifest.database_schema_version,
            database_fingerprint: manifest.database_fingerprint.clone(),
            database_hash_algorithm: manifest.database_hash_algorithm.clone(),
            cas_objects: manifest.cas_objects.clone(),
        })
        .expect("attacker can recompute the adjacent unkeyed manifest hash");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize modified manifest"),
        )
        .expect("replace manifest");
        assert!(matches!(
            ForgeStateStore::restore_from_backup(
                backup_root.path(),
                restore_root.path(),
                &trusted_fingerprint,
            )
            .await,
            Err(StateError::Integrity)
        ));
    }

    #[tokio::test]
    async fn content_store_refuses_a_non_directory_path_component() {
        let root = tempfile::tempdir().expect("state root");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        fs::write(root.path().join("cas"), b"not a directory").expect("fixture");
        assert!(matches!(
            store.put_blob(b"content"),
            Err(StateError::Integrity)
        ));
    }

    #[test]
    fn keys_and_identities_reject_paths_and_unbounded_values() {
        assert!(validate_key("module.state").is_ok());
        assert!(validate_key("../escape").is_err());
        assert!(validate_identity(&digest("valid")).is_ok());
        assert!(validate_identity("not-a-fingerprint").is_err());
    }
}
