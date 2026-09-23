use std::collections::BTreeMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    Declared,
    ContractValidated,
    ConformancePassed,
    RuntimeVerified,
    Proven,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Ready,
    Degraded,
    NotReady,
    Unknown,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    Public,
    Internal,
    Sensitive,
    Secret,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectClass {
    Pure,
    LocalMutation,
    ReversibleExternal,
    IdempotentExternal,
    IrreversibleExternal,
    HighAssurance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    Native,
    LocalWorker,
    Remote,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityDescriptor {
    pub id: String,
    pub capability_id: String,
    pub version: String,
    pub contract_id: String,
    pub evidence: EvidenceLevel,
    pub health: HealthState,
    pub health_observed_at_ms: u64,
    pub health_max_age_ms: u64,
    pub privacy: PrivacyClass,
    pub side_effect: SideEffectClass,
    pub deterministic: bool,
    pub quality_basis_points: u16,
    pub latency_ms: u32,
    pub token_cost: u32,
    pub monetary_cost_micros: u64,
    pub locality: Locality,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilitySnapshot {
    pub epoch: u64,
    pub digest: String,
    pub entries: BTreeMap<String, CapabilityDescriptor>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CapabilityError {
    #[error("capability descriptor is invalid")]
    InvalidDescriptor,
    #[error("capability already exists")]
    Duplicate,
    #[error("capability does not exist")]
    NotFound,
    #[error("evidence promotion must be monotonic or explicitly quarantined")]
    EvidenceRegression,
    #[error("capability snapshot could not be serialized")]
    SnapshotSerialization,
}

pub struct CapabilityRegistry {
    epoch: RwLock<u64>,
    entries: RwLock<BTreeMap<String, CapabilityDescriptor>>,
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            epoch: RwLock::new(0),
            entries: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn register(&self, descriptor: CapabilityDescriptor) -> Result<(), CapabilityError> {
        validate_descriptor(&descriptor)?;
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if entries.contains_key(&descriptor.id) {
            return Err(CapabilityError::Duplicate);
        }
        entries.insert(descriptor.id.clone(), descriptor);
        self.bump_epoch();
        Ok(())
    }

    pub fn update_health(
        &self,
        id: &str,
        health: HealthState,
        observed_at_ms: u64,
    ) -> Result<(), CapabilityError> {
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let descriptor = entries.get_mut(id).ok_or(CapabilityError::NotFound)?;
        if observed_at_ms < descriptor.health_observed_at_ms {
            return Err(CapabilityError::InvalidDescriptor);
        }
        descriptor.health = health;
        descriptor.health_observed_at_ms = observed_at_ms;
        if health == HealthState::Quarantined {
            descriptor.evidence = EvidenceLevel::Quarantined;
        }
        self.bump_epoch();
        Ok(())
    }

    pub fn promote_evidence(
        &self,
        id: &str,
        evidence: EvidenceLevel,
    ) -> Result<(), CapabilityError> {
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let descriptor = entries.get_mut(id).ok_or(CapabilityError::NotFound)?;
        if descriptor.evidence == EvidenceLevel::Quarantined
            && evidence != EvidenceLevel::Quarantined
        {
            return Err(CapabilityError::EvidenceRegression);
        }
        if evidence != EvidenceLevel::Quarantined && evidence < descriptor.evidence {
            return Err(CapabilityError::EvidenceRegression);
        }
        descriptor.evidence = evidence;
        if evidence == EvidenceLevel::Quarantined {
            descriptor.health = HealthState::Quarantined;
        }
        self.bump_epoch();
        Ok(())
    }

    pub fn snapshot(&self) -> Result<CapabilitySnapshot, CapabilityError> {
        let entries_guard = self
            .entries
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let epoch = *self
            .epoch
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entries = entries_guard.clone();
        let bytes =
            serde_json::to_vec(&entries).map_err(|_| CapabilityError::SnapshotSerialization)?;
        Ok(CapabilitySnapshot {
            epoch,
            digest: blake3::hash(&bytes).to_hex().to_string(),
            entries,
        })
    }

    fn bump_epoch(&self) {
        let mut epoch = self
            .epoch
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *epoch = epoch.saturating_add(1);
    }
}

fn validate_descriptor(descriptor: &CapabilityDescriptor) -> Result<(), CapabilityError> {
    let valid_identity = !descriptor.id.trim().is_empty()
        && descriptor.id.len() <= 160
        && descriptor
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        && !descriptor.id.contains("..")
        && !descriptor.capability_id.trim().is_empty()
        && descriptor.capability_id.len() <= 160
        && descriptor
            .capability_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        && !descriptor.capability_id.contains("..")
        && !descriptor.contract_id.trim().is_empty()
        && descriptor.contract_id.len() <= 160
        && descriptor
            .contract_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        && !descriptor.contract_id.contains("..")
        && !descriptor.version.trim().is_empty();
    if !valid_identity
        || descriptor.quality_basis_points > 10_000
        || descriptor.health_max_age_ms == 0
        || descriptor.evidence == EvidenceLevel::Quarantined
            && descriptor.health != HealthState::Quarantined
    {
        return Err(CapabilityError::InvalidDescriptor);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(id: &str) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: id.into(),
            capability_id: id.into(),
            version: "1.0.0".into(),
            contract_id: "forge.contract.test".into(),
            evidence: EvidenceLevel::Declared,
            health: HealthState::Unknown,
            health_observed_at_ms: 0,
            health_max_age_ms: 1000,
            privacy: PrivacyClass::Internal,
            side_effect: SideEffectClass::Pure,
            deterministic: true,
            quality_basis_points: 9000,
            latency_ms: 20,
            token_cost: 0,
            monetary_cost_micros: 0,
            locality: Locality::Native,
        }
    }

    #[test]
    fn snapshots_are_immutable_and_epoch_bound() {
        let registry = CapabilityRegistry::new();
        registry
            .register(descriptor("native.parse"))
            .expect("register capability");
        let old = registry.snapshot().expect("snapshot");
        registry
            .promote_evidence("native.parse", EvidenceLevel::ConformancePassed)
            .expect("promotion");
        let new = registry.snapshot().expect("snapshot");
        assert_eq!(
            old.entries["native.parse"].evidence,
            EvidenceLevel::Declared
        );
        assert!(new.epoch > old.epoch);
        assert_ne!(new.digest, old.digest);
    }

    #[test]
    fn quarantine_cannot_be_silently_reversed_by_stale_evidence() {
        let registry = CapabilityRegistry::new();
        registry
            .register(descriptor("native.cache"))
            .expect("register");
        registry
            .update_health("native.cache", HealthState::Quarantined, 10)
            .expect("quarantine");
        assert_eq!(
            registry.promote_evidence("native.cache", EvidenceLevel::Proven),
            Err(CapabilityError::EvidenceRegression)
        );
    }

    #[test]
    fn registry_rejects_untrusted_path_like_identifiers() {
        let registry = CapabilityRegistry::new();
        assert_eq!(
            registry.register(descriptor("../outside")),
            Err(CapabilityError::InvalidDescriptor)
        );
    }
}
