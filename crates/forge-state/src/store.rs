use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Row, SqlitePool};
use thiserror::Error;

const SCHEMA_VERSION: i64 = 2;
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

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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
}

#[derive(Clone)]
pub struct ForgeStateStore {
    root: PathBuf,
    pool: SqlitePool,
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
        let store = Self { root, pool };
        store.migrate().await?;
        store.integrity_check().await?;
        store.mark_interrupted_commands_unknown().await?;
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

    pub async fn record_resource_usage(
        &self,
        record: &PersistedResourceUsageRecord,
        pool_limit: &PersistedResourceVector,
    ) -> Result<bool, StateError> {
        validate_resource_usage_record(record)?;
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
        let inserted = sqlx::query(
            "INSERT INTO resource_usage(usage_id, pool, tokens, cost_micros, payload) SELECT ?1, ?2, ?3, ?4, ?5 WHERE COALESCE((SELECT SUM(tokens) FROM resource_usage WHERE pool=?2), 0) <= ?6 - ?3 AND COALESCE((SELECT SUM(cost_micros) FROM resource_usage WHERE pool=?2), 0) <= ?7 - ?4 ON CONFLICT(usage_id) DO NOTHING",
        )
        .bind(&record.usage_id)
        .bind(pool)
        .bind(tokens)
        .bind(cost_micros)
        .bind(&payload)
        .bind(token_limit)
        .bind(cost_limit)
        .execute(&self.pool)
        .await
        .map_err(StateError::Storage)?
        .rows_affected();
        if inserted == 1 {
            return Ok(true);
        }
        let existing = sqlx::query("SELECT payload FROM resource_usage WHERE usage_id=?1")
            .bind(&record.usage_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        let Some(existing) = existing else {
            return Err(StateError::ResourceBudgetExceeded);
        };
        let existing_payload: Vec<u8> = existing.try_get("payload").map_err(StateError::Storage)?;
        if existing_payload == payload {
            Ok(false)
        } else {
            Err(StateError::ResourceUsageConflict)
        }
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

    pub async fn begin_idempotent(
        &self,
        identity: &str,
    ) -> Result<Option<IdempotencyRecord>, StateError> {
        validate_identity(identity)?;
        let mut transaction = self.pool.begin().await.map_err(StateError::Storage)?;
        let inserted = sqlx::query(
            "INSERT INTO command_intents(identity, state) VALUES(?1, 'in_flight') ON CONFLICT(identity) DO NOTHING",
        )
        .bind(identity)
        .execute(&mut *transaction)
        .await
        .map_err(StateError::Storage)?
        .rows_affected();
        let existing = if inserted == 0 {
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
        validate_identity(identity)?;
        let update = sqlx::query(
            "UPDATE command_intents SET state='in_flight', result=NULL, error_code=NULL, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='failed_retryable'",
        )
        .bind(identity)
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

    pub async fn enqueue_event(&self, event: &StoredEvent) -> Result<(), StateError> {
        validate_key(&event.event_id)?;
        validate_key(&event.contract_id)?;
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
        Ok(())
    }

    async fn transition_idempotency(
        &self,
        identity: &str,
        state: IdempotencyState,
        result: Option<Vec<u8>>,
        error_code: Option<String>,
    ) -> Result<(), StateError> {
        let update = sqlx::query(
            "UPDATE command_intents SET state=?2, result=?3, error_code=?4, updated_at=CURRENT_TIMESTAMP WHERE identity=?1 AND state='in_flight'",
        )
        .bind(identity)
        .bind(state.as_db())
        .bind(result)
        .bind(error_code)
        .execute(&self.pool)
        .await
        .map_err(StateError::Storage)?;
        if update.rows_affected() != 1 {
            return Err(StateError::StateConflict);
        }
        Ok(())
    }

    async fn mark_interrupted_commands_unknown(&self) -> Result<(), StateError> {
        sqlx::query("UPDATE command_intents SET state='unknown_outcome', error_code='FORGE.COMMAND.RECOVERY_UNKNOWN' WHERE state='in_flight'")
            .execute(&self.pool)
            .await
            .map_err(StateError::Storage)?;
        Ok(())
    }
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

        assert_eq!(store.schema_version(), 2);
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
        assert_eq!(store.schema_version(), 2);
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
