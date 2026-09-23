use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

use forge_contracts::{
    ContractDefinition, ContractId, ContractVersion, Fingerprint, FingerprintKind,
    ValidatedContract,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigLayer {
    CompiledDefault,
    ProfileDefault,
    Project,
    Environment,
    User,
    Process,
    Execution,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigImpactClass {
    LiveSafe,
    NewExecutionsOnly,
    ModuleRestart,
    KernelRestart,
    MigrationRequired,
    SecurityCritical,
}

#[derive(Clone, Debug)]
pub struct ConfigSpec {
    pub key: String,
    pub owner: String,
    pub schema: Value,
    pub secret_reference: bool,
    pub impact: ConfigImpactClass,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigInputs {
    layers: BTreeMap<ConfigLayer, BTreeMap<String, Value>>,
}

impl ConfigInputs {
    pub fn insert(&mut self, layer: ConfigLayer, key: impl Into<String>, value: Value) {
        self.layers
            .entry(layer)
            .or_default()
            .insert(key.into(), value);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigProvenance {
    pub layer: ConfigLayer,
    pub owner: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigCapsule {
    pub schema_version: u16,
    pub values: BTreeMap<String, Value>,
    pub provenance: BTreeMap<String, ConfigProvenance>,
    pub semantic_fingerprint: Fingerprint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReloadDisposition {
    Applied,
    RequiresRestart,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ConfigError {
    #[error("configuration spec is invalid or duplicated")]
    InvalidSpec,
    #[error("configuration key has no registered owner")]
    UnknownKey,
    #[error("configuration value violates its declared schema")]
    InvalidValue,
    #[error("secret configuration must be a secret reference")]
    SecretValueRejected,
    #[error("security-sensitive reload requires explicit approval")]
    ApprovalRequired,
    #[error("configuration reload could not be prepared")]
    PrepareFailed,
    #[error("configuration fingerprint could not be created")]
    FingerprintFailed,
}

#[derive(Default)]
pub struct ConfigRegistry {
    specs: BTreeMap<String, ConfigSpec>,
}

impl ConfigRegistry {
    pub fn register(&mut self, spec: ConfigSpec) -> Result<(), ConfigError> {
        if spec.key.trim().is_empty()
            || spec.owner.trim().is_empty()
            || self.specs.contains_key(&spec.key)
        {
            return Err(ConfigError::InvalidSpec);
        }
        if spec.secret_reference && !spec.schema.is_object() {
            return Err(ConfigError::InvalidSpec);
        }
        self.specs.insert(spec.key.clone(), spec);
        Ok(())
    }

    pub fn resolve(&self, inputs: &ConfigInputs) -> Result<ConfigCapsule, ConfigError> {
        let mut values = BTreeMap::new();
        let mut provenance = BTreeMap::new();
        for (layer, entries) in &inputs.layers {
            for (key, value) in entries {
                let spec = self.specs.get(key).ok_or(ConfigError::UnknownKey)?;
                if spec.secret_reference && !is_secret_reference(value) {
                    return Err(ConfigError::SecretValueRejected);
                }
                let definition = ContractDefinition {
                    id: ContractId::new(format!("config.{key}"))
                        .map_err(|_| ConfigError::InvalidSpec)?,
                    version: ContractVersion::new(1, 0, 0),
                    owner: spec.owner.clone(),
                    schema: spec.schema.clone(),
                };
                let contract =
                    ValidatedContract::compile(definition).map_err(|_| ConfigError::InvalidSpec)?;
                contract
                    .validate(value)
                    .map_err(|_| ConfigError::InvalidValue)?;
                values.insert(key.clone(), value.clone());
                provenance.insert(
                    key.clone(),
                    ConfigProvenance {
                        layer: *layer,
                        owner: spec.owner.clone(),
                    },
                );
            }
        }
        let semantic_values = values
            .iter()
            .map(|(key, value)| {
                let semantic_value = if self
                    .specs
                    .get(key)
                    .is_some_and(|spec| spec.secret_reference)
                {
                    serde_json::json!({"configured": true})
                } else {
                    value.clone()
                };
                (key.clone(), semantic_value)
            })
            .collect::<BTreeMap<_, _>>();
        let fingerprint = Fingerprint::from_json(
            FingerprintKind::Config,
            &serde_json::to_value(semantic_values).map_err(|_| ConfigError::FingerprintFailed)?,
        )
        .map_err(|_| ConfigError::FingerprintFailed)?;
        Ok(ConfigCapsule {
            schema_version: 1,
            values,
            provenance,
            semantic_fingerprint: fingerprint,
        })
    }

    fn changed_impact(&self, old: &ConfigCapsule, new: &ConfigCapsule) -> ConfigImpactClass {
        let keys = old
            .values
            .keys()
            .chain(new.values.keys())
            .collect::<std::collections::BTreeSet<_>>();
        keys.into_iter()
            .filter(|key| old.values.get(*key) != new.values.get(*key))
            .filter_map(|key| self.specs.get(key).map(|spec| spec.impact))
            .max_by_key(|impact| *impact as u8)
            .unwrap_or(ConfigImpactClass::LiveSafe)
    }
}

#[derive(Clone)]
pub struct ConfigStore {
    current: Arc<RwLock<Arc<ConfigCapsule>>>,
    reload_lock: Arc<Mutex<()>>,
}

impl ConfigStore {
    pub fn new(initial: ConfigCapsule) -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::new(initial))),
            reload_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn snapshot(&self) -> Arc<ConfigCapsule> {
        Arc::clone(
            &self
                .current
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    pub fn reload(
        &self,
        registry: &ConfigRegistry,
        candidate: ConfigCapsule,
        explicit_security_approval: bool,
        prepare: impl FnOnce(&ConfigCapsule, &ConfigCapsule) -> Result<(), ()>,
    ) -> Result<ReloadDisposition, ConfigError> {
        let _serial = self
            .reload_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let old = self.snapshot();
        let impact = registry.changed_impact(&old, &candidate);
        if impact == ConfigImpactClass::SecurityCritical && !explicit_security_approval {
            return Err(ConfigError::ApprovalRequired);
        }
        if matches!(
            impact,
            ConfigImpactClass::KernelRestart | ConfigImpactClass::MigrationRequired
        ) {
            return Ok(ReloadDisposition::RequiresRestart);
        }
        prepare(&old, &candidate).map_err(|_| ConfigError::PrepareFailed)?;
        *self
            .current
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::new(candidate);
        Ok(ReloadDisposition::Applied)
    }
}

fn is_secret_reference(value: &Value) -> bool {
    value.as_str().is_some_and(|reference| {
        reference.starts_with("secret://")
            && reference.len() > "secret://".len()
            && !reference.contains("..")
            && !reference.chars().any(char::is_whitespace)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn registry() -> ConfigRegistry {
        let mut registry = ConfigRegistry::default();
        registry
            .register(ConfigSpec {
                key: "forge.runtime.workers".into(),
                owner: "forge.runtime".into(),
                schema: json!({"type": "integer", "minimum": 1, "maximum": 64}),
                secret_reference: false,
                impact: ConfigImpactClass::NewExecutionsOnly,
            })
            .expect("workers spec");
        registry
            .register(ConfigSpec {
                key: "integration.remote.token".into(),
                owner: "integration.remote".into(),
                schema: json!({"type": "string", "minLength": 1}),
                secret_reference: true,
                impact: ConfigImpactClass::SecurityCritical,
            })
            .expect("secret spec");
        registry
    }

    #[test]
    fn precedence_is_explainable_and_secret_references_do_not_affect_semantic_fingerprint() {
        let registry = registry();
        let mut first = ConfigInputs::default();
        first.insert(
            ConfigLayer::CompiledDefault,
            "forge.runtime.workers",
            json!(2),
        );
        first.insert(ConfigLayer::Project, "forge.runtime.workers", json!(4));
        first.insert(
            ConfigLayer::User,
            "integration.remote.token",
            json!("secret://providers/remote/token"),
        );
        let resolved = registry.resolve(&first).expect("resolved capsule");
        assert_eq!(resolved.values["forge.runtime.workers"], json!(4));
        assert_eq!(
            resolved.provenance["forge.runtime.workers"].layer,
            ConfigLayer::Project
        );

        let mut second = first.clone();
        second.insert(
            ConfigLayer::User,
            "integration.remote.token",
            json!("secret://providers/remote/rotated"),
        );
        let rotated = registry.resolve(&second).expect("rotated capsule");
        assert_eq!(resolved.semantic_fingerprint, rotated.semantic_fingerprint);
    }

    #[test]
    fn raw_secret_bytes_and_schema_invalid_values_are_rejected() {
        let registry = registry();
        let mut raw = ConfigInputs::default();
        raw.insert(
            ConfigLayer::User,
            "integration.remote.token",
            json!("plain-secret"),
        );
        assert_eq!(
            registry.resolve(&raw).err(),
            Some(ConfigError::SecretValueRejected)
        );
        let mut invalid = ConfigInputs::default();
        invalid.insert(ConfigLayer::Project, "forge.runtime.workers", json!(0));
        assert_eq!(
            registry.resolve(&invalid).err(),
            Some(ConfigError::InvalidValue)
        );
    }

    #[test]
    fn failed_reload_preserves_the_published_capsule() {
        let registry = registry();
        let mut initial_inputs = ConfigInputs::default();
        initial_inputs.insert(
            ConfigLayer::CompiledDefault,
            "forge.runtime.workers",
            json!(2),
        );
        let initial = registry.resolve(&initial_inputs).expect("initial config");
        let store = ConfigStore::new(initial);
        let mut changed_inputs = ConfigInputs::default();
        changed_inputs.insert(
            ConfigLayer::CompiledDefault,
            "forge.runtime.workers",
            json!(4),
        );
        let changed = registry.resolve(&changed_inputs).expect("candidate config");
        assert_eq!(
            store.reload(&registry, changed, false, |_, _| Err(())),
            Err(ConfigError::PrepareFailed)
        );
        assert_eq!(store.snapshot().values["forge.runtime.workers"], json!(2));
    }

    #[test]
    fn security_critical_reload_requires_explicit_approval() {
        let registry = registry();
        let mut initial = ConfigInputs::default();
        initial.insert(
            ConfigLayer::User,
            "integration.remote.token",
            json!("secret://provider/key"),
        );
        let store = ConfigStore::new(registry.resolve(&initial).expect("initial"));
        let mut next = ConfigInputs::default();
        next.insert(
            ConfigLayer::User,
            "integration.remote.token",
            json!("secret://provider/next"),
        );
        let candidate = registry.resolve(&next).expect("candidate");
        assert_eq!(
            store.reload(&registry, candidate, false, |_, _| Ok(())),
            Err(ConfigError::ApprovalRequired)
        );
    }
}
