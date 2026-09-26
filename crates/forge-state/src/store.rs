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
use sqlx::{Row, SqlitePool};
use thiserror::Error;
use tokio::sync::watch;

const SCHEMA_VERSION: i64 = 3;
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
}

impl IdempotencyState {
    fn as_db(self) -> &'static str {
        match self {
            Self::InFlight => "in_flight",
            Self::Committed => "committed",
            Self::FailedRetryable => "failed_retryable",
            Self::FailedFinal => "failed_final",
            Self::UnknownOutcome => "unknown_outcome",
        }
    }

    fn from_db(value: &str) -> Result<Self, StateError> {
        match value {
            "in_flight" => Ok(Self::InFlight),
            "committed" => Ok(Self::Committed),
            "failed_retryable" => Ok(Self::FailedRetryable),
            "failed_final" => Ok(Self::FailedFinal),
            "unknown_outcome" => Ok(Self::UnknownOutcome),
            _ => Err(StateError::Integrity),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdempotencyRecord {
    pub state: IdempotencyState,
    pub result: Option<Value>,
    pub error_code: Option<String>,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupManifest {
    schema_version: u16,
    database_schema_version: i64,
    database_fingerprint: String,
    database_hash_algorithm: String,
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
            validate_key(&event.event_id)?;
            validate_key(&event.contract_id)?;
            ensure_safe_payload(&event.payload)?;
            let payload = serde_json::to_vec(&event.payload).map_err(StateError::Serialization)?;
            let inserted = sqlx::query("INSERT INTO event_outbox(event_id, contract_id, payload, delivery_state) VALUES(?1, ?2, ?3, 'pending') ON CONFLICT(event_id) DO NOTHING")
                .bind(&event.event_id)
                .bind(&event.contract_id)
                .bind(&payload)
                .execute(&mut *transaction)
                .await
                .map_err(StateError::Storage)?
                .rows_affected();
            if inserted == 0 {
                let row =
                    sqlx::query("SELECT contract_id, payload FROM event_outbox WHERE event_id=?1")
                        .bind(&event.event_id)
                        .fetch_optional(&mut *transaction)
                        .await
                        .map_err(StateError::Storage)?
                        .ok_or(StateError::Integrity)?;
                let contract_id: String =
                    row.try_get("contract_id").map_err(StateError::Storage)?;
                let prior_payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
                if contract_id != event.contract_id || prior_payload != payload {
                    return Err(StateError::StateConflict);
                }
            }
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
            Some(IdempotencyRecord {
                state: IdempotencyState::from_db(&state)?,
                result: result
                    .map(|bytes| serde_json::from_slice(&bytes).map_err(StateError::Serialization))
                    .transpose()?,
                error_code,
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
        validate_key(error_code)?;
        self.transition_idempotency(identity, state, None, Some(error_code.to_owned()))
            .await
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
        validate_key(&event.event_id)?;
        validate_key(&event.contract_id)?;
        ensure_safe_payload(&event.payload)?;
        let payload = serde_json::to_vec(&event.payload).map_err(StateError::Serialization)?;
        let inserted = sqlx::query("INSERT INTO event_outbox(event_id, contract_id, payload, delivery_state) VALUES(?1, ?2, ?3, 'pending') ON CONFLICT(event_id) DO NOTHING")
            .bind(&event.event_id)
            .bind(&event.contract_id)
            .bind(&payload)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?
            .rows_affected();
        if inserted == 0 {
            let row =
                sqlx::query("SELECT contract_id, payload FROM event_outbox WHERE event_id=?1")
                    .bind(&event.event_id)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(StateError::Storage)?
                    .ok_or(StateError::Integrity)?;
            let contract_id: String = row.try_get("contract_id").map_err(StateError::Storage)?;
            let prior_payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
            if contract_id != event.contract_id || prior_payload != payload {
                return Err(StateError::StateConflict);
            }
        }
        Ok(())
    }

    pub async fn pending_events(&self, limit: u32) -> Result<Vec<StoredEvent>, StateError> {
        let rows = sqlx::query(
            "SELECT event_id, contract_id, payload FROM event_outbox WHERE delivery_state='pending' ORDER BY created_at, event_id LIMIT ?1",
        )
        .bind(i64::from(limit.min(10_000)))
        .fetch_all(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        rows.into_iter()
            .map(|row| {
                let payload: Vec<u8> = row.try_get("payload").map_err(StateError::Storage)?;
                Ok(StoredEvent {
                    event_id: row.try_get("event_id").map_err(StateError::Storage)?,
                    contract_id: row.try_get("contract_id").map_err(StateError::Storage)?,
                    payload: serde_json::from_slice(&payload).map_err(StateError::Serialization)?,
                })
            })
            .collect()
    }

    pub async fn mark_event_delivered(&self, event_id: &str) -> Result<bool, StateError> {
        validate_key(event_id)?;
        let result = sqlx::query("UPDATE event_outbox SET delivery_state='delivered' WHERE event_id=?1 AND delivery_state='pending'")
            .bind(event_id)
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
        reject_symlink_if_present(&backup_db)?;
        reject_symlink_if_present(&manifest_path)?;
        if backup_db.exists() || manifest_path.exists() {
            return Err(StateError::BackupTargetExists);
        }
        let backup_path = backup_db.to_string_lossy().into_owned();
        sqlx::query("VACUUM INTO ?1")
            .bind(backup_path)
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        let bytes = fs::read(&backup_db).map_err(StateError::Filesystem)?;
        let digest = blake3::hash(&bytes).to_hex().to_string();
        let manifest = serde_json::json!({
            "schemaVersion": 1,
            "databaseSchemaVersion": SCHEMA_VERSION,
            "databaseFingerprint": digest,
            "databaseHashAlgorithm": "blake3"
        });
        let manifest_bytes =
            serde_json::to_vec_pretty(&manifest).map_err(StateError::Serialization)?;
        if let Err(error) = write_atomic(&manifest_path, &manifest_bytes) {
            let _ = fs::remove_file(&backup_db);
            return Err(error);
        }
        Ok(digest)
    }

    pub async fn restore_from_backup(
        backup_root: impl AsRef<Path>,
        restore_root: impl AsRef<Path>,
    ) -> Result<Self, StateError> {
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
        if manifest.schema_version != 1
            || manifest.database_schema_version < 1
            || manifest.database_schema_version > SCHEMA_VERSION
            || manifest.database_hash_algorithm != "blake3"
            || manifest.database_fingerprint != actual_fingerprint
        {
            return Err(StateError::Integrity);
        }

        fs::create_dir_all(restore_root.as_ref()).map_err(StateError::Filesystem)?;
        let restore_root = restore_root
            .as_ref()
            .canonicalize()
            .map_err(|_| StateError::InvalidRoot)?;
        let restored_db = restore_root.join("forge.sqlite3");
        reject_symlink_if_present(&restored_db)?;
        if restored_db.exists() {
            return Err(StateError::BackupTargetExists);
        }
        let mut source = OpenOptions::new()
            .read(true)
            .open(&backup_db)
            .map_err(StateError::Filesystem)?;
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&restored_db)
            .map_err(StateError::Filesystem)?;
        std::io::copy(&mut source, &mut destination).map_err(StateError::Filesystem)?;
        destination.sync_all().map_err(StateError::Filesystem)?;
        drop(destination);
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
                    if secret_bearing_field(name) {
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

fn secret_bearing_field(name: &str) -> bool {
    let normalized = name
        .bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(char::from)
        .collect::<String>()
        .to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "apikey",
        "privatekey",
        "credential",
        "cookie",
        "authorization",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
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
        ] {
            let keyed_secret = json!({field: "opaque-reference-value"});
            assert!(matches!(
                validate_payload_for_persistence(&keyed_secret),
                Err(StateError::SensitivePayload)
            ));
        }

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
                .pending_events(10)
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

        assert_eq!(store.schema_version(), 3);
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
                .pending_events(10)
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
        assert_eq!(store.pending_events(10).await.expect("outbox").len(), 1);
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
        assert_eq!(store.pending_events(10).await.expect("outbox").len(), 2);
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
                    .pending_events(10)
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

        store.migrate().await.expect("v1 to v2 migration");
        store.integrity_check().await.expect("migrated integrity");
        assert_eq!(store.schema_version(), 3);
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
    async fn durable_event_outbox_is_ordered_and_acknowledged_once() {
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
            store.pending_events(10).await.expect("read pending"),
            vec![event]
        );
        assert!(
            store
                .mark_event_delivered("evt-1")
                .await
                .expect("ack event")
        );
        assert!(
            !store
                .mark_event_delivered("evt-1")
                .await
                .expect("second ack is a no-op")
        );
        assert!(
            store
                .pending_events(10)
                .await
                .expect("pending after ack")
                .is_empty()
        );
        store.pool.close().await;
        drop(store);
        let reopened = ForgeStateStore::open(root.path())
            .await
            .expect("reopen acknowledged outbox");
        assert!(
            reopened
                .pending_events(10)
                .await
                .expect("ack remains durable after restart")
                .is_empty()
        );
        assert!(
            !reopened
                .mark_event_delivered("evt-1")
                .await
                .expect("ack replay after restart is a no-op")
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
        let backup_digest = source
            .backup_to(backup.path())
            .await
            .expect("backup snapshot");
        let bytes = fs::read(backup.path().join("forge.sqlite3")).expect("backup database");
        assert_eq!(blake3::hash(&bytes).to_hex().as_str(), backup_digest);
        let restored_root = tempfile::tempdir().expect("restore directory");
        let restored = ForgeStateStore::restore_from_backup(backup.path(), restored_root.path())
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
    }

    #[tokio::test]
    async fn restore_refuses_a_database_that_does_not_match_its_manifest() {
        let source_root = tempfile::tempdir().expect("source");
        let backup_root = tempfile::tempdir().expect("backup");
        let restore_root = tempfile::tempdir().expect("restore");
        let source = ForgeStateStore::open(source_root.path())
            .await
            .expect("store");
        source.backup_to(backup_root.path()).await.expect("backup");
        fs::write(backup_root.path().join("forge.sqlite3"), b"tampered").expect("tamper fixture");
        assert!(matches!(
            ForgeStateStore::restore_from_backup(backup_root.path(), restore_root.path()).await,
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
