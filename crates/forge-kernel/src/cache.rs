use std::collections::BTreeMap;

use forge_state::{ForgeStateStore, StateError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::capabilities::PrivacyClass;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheIdentity {
    pub schema_version: u16,
    pub scope_id: String,
    pub kind: String,
    pub semantic_input_fingerprint: String,
    pub dependency_fingerprints: BTreeMap<String, String>,
    pub environment_fingerprint: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    pub schema_version: u16,
    pub identity_fingerprint: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub producer: String,
    pub privacy: PrivacyClass,
    pub value: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheLookup {
    Hit(Value),
    Miss,
    Stale,
    FingerprintMismatch,
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache identity or policy is invalid")]
    InvalidIdentity,
    #[error("cache storage operation failed")]
    State(#[from] StateError),
    #[error("cache entry is corrupt")]
    Corrupt,
    #[error("sensitive values cannot be stored in the reusable cache")]
    PrivacyDenied,
}

impl CacheIdentity {
    pub fn new(
        scope_id: impl Into<String>,
        kind: impl Into<String>,
        semantic_input_fingerprint: impl Into<String>,
        dependency_fingerprints: BTreeMap<String, String>,
        environment_fingerprint: impl Into<String>,
    ) -> Result<Self, CacheError> {
        let scope_id = scope_id.into();
        let kind = kind.into();
        let semantic_input_fingerprint = semantic_input_fingerprint.into();
        let environment_fingerprint = environment_fingerprint.into();
        if !valid_token(&scope_id)
            || !valid_token(&kind)
            || !valid_digest(&semantic_input_fingerprint)
            || !valid_digest(&environment_fingerprint)
            || dependency_fingerprints.len() > 256
            || dependency_fingerprints
                .iter()
                .any(|(id, digest)| !valid_token(id) || !valid_digest(digest))
        {
            return Err(CacheError::InvalidIdentity);
        }
        let bytes = serde_json::to_vec(&(
            1_u16,
            &scope_id,
            &kind,
            &semantic_input_fingerprint,
            &dependency_fingerprints,
            &environment_fingerprint,
        ))
        .map_err(|_| CacheError::InvalidIdentity)?;
        let fingerprint = blake3::hash(&bytes).to_hex().to_string();
        Ok(Self {
            schema_version: 1,
            scope_id,
            kind,
            semantic_input_fingerprint,
            dependency_fingerprints,
            environment_fingerprint,
            fingerprint,
        })
    }
}

pub async fn put(
    store: &ForgeStateStore,
    identity: &CacheIdentity,
    producer: &str,
    created_at_ms: u64,
    ttl_ms: u64,
    privacy: PrivacyClass,
    value: Value,
) -> Result<(), CacheError> {
    validate_identity(identity)?;
    if !valid_token(producer) || ttl_ms == 0 || created_at_ms.checked_add(ttl_ms).is_none() {
        return Err(CacheError::InvalidIdentity);
    }
    if privacy > PrivacyClass::Internal {
        return Err(CacheError::PrivacyDenied);
    }
    let entry = CacheEntry {
        schema_version: 1,
        identity_fingerprint: identity.fingerprint.clone(),
        created_at_ms,
        expires_at_ms: created_at_ms + ttl_ms,
        producer: producer.to_owned(),
        privacy,
        value,
    };
    let value = serde_json::to_value(entry).map_err(|_| CacheError::Corrupt)?;
    store
        .put_cache(
            "forge.runtime",
            &identity.fingerprint,
            &identity.fingerprint,
            &value,
        )
        .await?;
    Ok(())
}

pub async fn get(
    store: &ForgeStateStore,
    identity: &CacheIdentity,
    now_ms: u64,
) -> Result<CacheLookup, CacheError> {
    validate_identity(identity)?;
    let Some(value) = store
        .get_cache(
            "forge.runtime",
            &identity.fingerprint,
            &identity.fingerprint,
        )
        .await?
    else {
        return Ok(CacheLookup::Miss);
    };
    let entry: CacheEntry = serde_json::from_value(value).map_err(|_| CacheError::Corrupt)?;
    if entry.schema_version != 1
        || entry.identity_fingerprint != identity.fingerprint
        || entry.privacy > PrivacyClass::Internal
    {
        return Ok(CacheLookup::FingerprintMismatch);
    }
    if now_ms < entry.created_at_ms || now_ms >= entry.expires_at_ms {
        return Ok(CacheLookup::Stale);
    }
    Ok(CacheLookup::Hit(entry.value))
}

fn validate_identity(identity: &CacheIdentity) -> Result<(), CacheError> {
    if identity.schema_version != 1 {
        return Err(CacheError::InvalidIdentity);
    }
    let verified = CacheIdentity::new(
        identity.scope_id.clone(),
        identity.kind.clone(),
        identity.semantic_input_fingerprint.clone(),
        identity.dependency_fingerprints.clone(),
        identity.environment_fingerprint.clone(),
    )?;
    if verified.fingerprint == identity.fingerprint {
        Ok(())
    } else {
        Err(CacheError::InvalidIdentity)
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_token(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn digest(value: &str) -> String {
        blake3::hash(value.as_bytes()).to_hex().to_string()
    }

    #[tokio::test]
    async fn cache_requires_full_semantic_identity_and_respects_ttl() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let identity = CacheIdentity::new(
            "project-a",
            "test-proof",
            digest("input"),
            BTreeMap::new(),
            digest("toolchain"),
        )
        .expect("identity");
        put(
            &store,
            &identity,
            "forge.kernel",
            100,
            50,
            PrivacyClass::Internal,
            json!({"pass": true}),
        )
        .await
        .expect("cache write");
        assert_eq!(
            get(&store, &identity, 120).await.expect("hit"),
            CacheLookup::Hit(json!({"pass": true}))
        );
        assert_eq!(
            get(&store, &identity, 150).await.expect("expired"),
            CacheLookup::Stale
        );
        store.clear_cache().await.expect("cache clear");
        assert_eq!(
            get(&store, &identity, 120).await.expect("miss"),
            CacheLookup::Miss
        );
    }

    #[tokio::test]
    async fn sensitive_cache_values_are_rejected_even_with_a_valid_semantic_key() {
        let root = tempfile::tempdir().expect("temp directory");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let identity = CacheIdentity::new(
            "project-a",
            "test-proof",
            digest("input"),
            BTreeMap::new(),
            digest("environment"),
        )
        .expect("identity");
        assert!(matches!(
            put(
                &store,
                &identity,
                "forge.kernel",
                100,
                50,
                PrivacyClass::Sensitive,
                json!({"secret": true}),
            )
            .await,
            Err(CacheError::PrivacyDenied)
        ));
    }
}
