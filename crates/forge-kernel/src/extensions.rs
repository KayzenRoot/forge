use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capabilities::SideEffectClass;
use crate::resources::ResourceVector;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionClass {
    NativeModule,
    FirstPartyAdapter,
    LocalWorker,
    McpSkillAdapter,
    ThirdPartyPlugin,
    RemoteProvider,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationTier {
    T0TrustedNative,
    T1RestrictedInProcess,
    T2SeparateWorker,
    T3OperatingSystemSandbox,
    T4RemoteBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdapterManifest {
    pub adapter_id: String,
    pub publisher_id: String,
    pub version: String,
    pub sdk_version: u16,
    pub protocol_version: u16,
    pub contract_id: String,
    pub extension_class: ExtensionClass,
    pub isolation_tier: IsolationTier,
    pub provided_capabilities: Vec<String>,
    pub required_capabilities: BTreeSet<String>,
    pub requested_permissions: BTreeSet<String>,
    pub network_destinations: BTreeSet<String>,
    pub filesystem_scopes: BTreeSet<String>,
    pub process_scopes: BTreeSet<String>,
    pub resource_budget: ResourceVector,
    pub side_effect: SideEffectClass,
    pub artifact_digest: String,
    pub provenance_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionPolicy {
    pub sdk_version: u16,
    pub protocol_version: u16,
    pub maximum_capabilities: usize,
    pub minimum_isolation_tier: IsolationTier,
    pub maximum_side_effect: SideEffectClass,
    pub maximum_resources: ResourceVector,
    pub allowed_permissions: BTreeSet<String>,
    pub allowed_network_destinations: BTreeSet<String>,
    pub allowed_filesystem_scopes: BTreeSet<String>,
    pub allowed_process_scopes: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterStatus {
    PreparedDormant,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AdapterReceipt {
    pub adapter_id: String,
    pub manifest_fingerprint: String,
    pub isolation_tier: IsolationTier,
    pub status: AdapterStatus,
    pub granted_permissions: Vec<String>,
    pub runtime_loaded: bool,
}

pub struct PermissionLease {
    pub permissions: BTreeSet<String>,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub fingerprint: String,
    revoked: Arc<AtomicBool>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ExtensionError {
    #[error("adapter manifest is invalid")]
    InvalidManifest,
    #[error("adapter requests a permission or scope outside policy")]
    PermissionDenied,
    #[error("adapter exceeds the configured capability, resource, protocol, or isolation boundary")]
    PolicyViolation,
    #[error("adapter manifest fingerprint failed")]
    Fingerprint,
    #[error("permission lease is invalid or exceeds its parent")]
    InvalidLease,
}

pub fn prepare_adapter(
    manifest: &AdapterManifest,
    policy: &ExtensionPolicy,
) -> Result<AdapterReceipt, ExtensionError> {
    let capabilities = manifest
        .provided_capabilities
        .iter()
        .collect::<BTreeSet<_>>();
    if !valid_identity(&manifest.adapter_id)
        || !valid_identity(&manifest.publisher_id)
        || !valid_identity(&manifest.contract_id)
        || manifest.version.trim().is_empty()
        || manifest.version.len() > 64
        || manifest.artifact_digest.len() != 64
        || !manifest
            .artifact_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || manifest.provenance_fingerprint.len() != 64
        || !manifest
            .provenance_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || manifest.provided_capabilities.is_empty()
        || manifest.provided_capabilities.len() > policy.maximum_capabilities
        || capabilities.len() != manifest.provided_capabilities.len()
        || manifest
            .provided_capabilities
            .iter()
            .any(|id| !valid_identity(id))
        || manifest
            .required_capabilities
            .iter()
            .any(|id| !valid_identity(id))
        || manifest
            .requested_permissions
            .iter()
            .any(|permission| !valid_permission(permission))
        || manifest
            .network_destinations
            .iter()
            .any(|origin| !valid_origin(origin))
        || manifest
            .filesystem_scopes
            .iter()
            .any(|scope| !valid_path_scope(scope))
        || manifest
            .process_scopes
            .iter()
            .any(|scope| !valid_identity(scope))
        || manifest.resource_budget == ResourceVector::default()
    {
        return Err(ExtensionError::InvalidManifest);
    }
    if manifest.sdk_version != policy.sdk_version
        || manifest.protocol_version != policy.protocol_version
        || manifest.side_effect > policy.maximum_side_effect
        || !manifest
            .resource_budget
            .fits_within(policy.maximum_resources)
    {
        return Err(ExtensionError::PolicyViolation);
    }
    let required_isolation = required_isolation(manifest.extension_class, manifest.side_effect)
        .max(policy.minimum_isolation_tier);
    if manifest.isolation_tier < required_isolation {
        return Err(ExtensionError::PolicyViolation);
    }
    if !manifest
        .requested_permissions
        .is_subset(&policy.allowed_permissions)
        || !manifest
            .network_destinations
            .is_subset(&policy.allowed_network_destinations)
        || !manifest
            .filesystem_scopes
            .is_subset(&policy.allowed_filesystem_scopes)
        || !manifest
            .process_scopes
            .is_subset(&policy.allowed_process_scopes)
    {
        return Err(ExtensionError::PermissionDenied);
    }
    let bytes = serde_json::to_vec(manifest).map_err(|_| ExtensionError::Fingerprint)?;
    Ok(AdapterReceipt {
        adapter_id: manifest.adapter_id.clone(),
        manifest_fingerprint: blake3::hash(&bytes).to_hex().to_string(),
        isolation_tier: manifest.isolation_tier,
        status: AdapterStatus::PreparedDormant,
        granted_permissions: manifest.requested_permissions.iter().cloned().collect(),
        runtime_loaded: false,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn grant_permission_lease(
    requested: &BTreeSet<String>,
    adapter_permissions: &BTreeSet<String>,
    caller_permissions: &BTreeSet<String>,
    issued_at_ms: u64,
    requested_ttl_ms: u64,
    parent_expires_at_ms: u64,
) -> Result<PermissionLease, ExtensionError> {
    let expires_at_ms = issued_at_ms
        .checked_add(requested_ttl_ms)
        .ok_or(ExtensionError::InvalidLease)?;
    if requested_ttl_ms == 0
        || expires_at_ms > parent_expires_at_ms
        || requested
            .iter()
            .any(|permission| !valid_permission(permission))
        || !requested.is_subset(adapter_permissions)
        || !requested.is_subset(caller_permissions)
    {
        return Err(ExtensionError::InvalidLease);
    }
    let bytes = serde_json::to_vec(&(1_u16, requested, issued_at_ms, expires_at_ms))
        .map_err(|_| ExtensionError::Fingerprint)?;
    Ok(PermissionLease {
        permissions: requested.clone(),
        issued_at_ms,
        expires_at_ms,
        fingerprint: blake3::hash(&bytes).to_hex().to_string(),
        revoked: Arc::new(AtomicBool::new(false)),
    })
}

impl PermissionLease {
    pub fn allows(&self, permission: &str, now_ms: u64) -> bool {
        !self.revoked.load(Ordering::Acquire)
            && now_ms < self.expires_at_ms
            && self.permissions.contains(permission)
    }

    pub fn revoke(&self) {
        self.revoked.store(true, Ordering::Release);
    }
}

fn required_isolation(class: ExtensionClass, effect: SideEffectClass) -> IsolationTier {
    let class_minimum = match class {
        ExtensionClass::NativeModule => IsolationTier::T0TrustedNative,
        ExtensionClass::FirstPartyAdapter => IsolationTier::T1RestrictedInProcess,
        ExtensionClass::LocalWorker
        | ExtensionClass::McpSkillAdapter
        | ExtensionClass::ThirdPartyPlugin => IsolationTier::T2SeparateWorker,
        ExtensionClass::RemoteProvider => IsolationTier::T4RemoteBoundary,
    };
    let effect_minimum = if matches!(
        effect,
        SideEffectClass::IrreversibleExternal | SideEffectClass::HighAssurance
    ) {
        IsolationTier::T3OperatingSystemSandbox
    } else {
        IsolationTier::T0TrustedNative
    };
    class_minimum.max(effect_minimum)
}

fn valid_identity(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
        && !value.contains("..")
}

fn valid_permission(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 192
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
        && value.contains('.')
        && !value.contains("..")
}

fn valid_origin(value: &str) -> bool {
    let origin = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://localhost:"));
    origin.is_some_and(|authority| {
        !authority.is_empty()
            && authority.len() <= 255
            && !authority.contains('/')
            && !authority.contains('@')
            && !authority.contains('*')
    })
}

fn valid_path_scope(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 512
        && (value.starts_with('/') || value.as_bytes().get(1) == Some(&b':'))
        && !value.contains("..")
        && !value.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ExtensionPolicy {
        ExtensionPolicy {
            sdk_version: 1,
            protocol_version: 1,
            maximum_capabilities: 4,
            minimum_isolation_tier: IsolationTier::T0TrustedNative,
            maximum_side_effect: SideEffectClass::LocalMutation,
            maximum_resources: ResourceVector {
                cpu_millis: 500,
                memory_bytes: 1_000_000,
                ..Default::default()
            },
            allowed_permissions: ["state.read".to_owned()].into_iter().collect(),
            allowed_network_destinations: BTreeSet::new(),
            allowed_filesystem_scopes: BTreeSet::new(),
            allowed_process_scopes: BTreeSet::new(),
        }
    }

    fn manifest() -> AdapterManifest {
        AdapterManifest {
            adapter_id: "adapter.local.index".into(),
            publisher_id: "forge.first-party".into(),
            version: "1.0.0".into(),
            sdk_version: 1,
            protocol_version: 1,
            contract_id: "forge.adapter.index".into(),
            extension_class: ExtensionClass::FirstPartyAdapter,
            isolation_tier: IsolationTier::T1RestrictedInProcess,
            provided_capabilities: vec!["index.read".into()],
            required_capabilities: BTreeSet::new(),
            requested_permissions: ["state.read".to_owned()].into_iter().collect(),
            network_destinations: BTreeSet::new(),
            filesystem_scopes: BTreeSet::new(),
            process_scopes: BTreeSet::new(),
            resource_budget: ResourceVector {
                cpu_millis: 20,
                memory_bytes: 16_384,
                ..Default::default()
            },
            side_effect: SideEffectClass::Pure,
            artifact_digest: blake3::hash(b"artifact").to_hex().to_string(),
            provenance_fingerprint: blake3::hash(b"provenance metadata").to_hex().to_string(),
        }
    }

    #[test]
    fn validated_adapter_is_prepared_dormant_at_or_above_its_required_isolation_tier() {
        let receipt = prepare_adapter(&manifest(), &policy()).expect("validated metadata");
        assert_eq!(receipt.status, AdapterStatus::PreparedDormant);
        assert!(!receipt.runtime_loaded);

        let mut untrusted = manifest();
        untrusted.extension_class = ExtensionClass::ThirdPartyPlugin;
        untrusted.isolation_tier = IsolationTier::T1RestrictedInProcess;
        assert_eq!(
            prepare_adapter(&untrusted, &policy()),
            Err(ExtensionError::PolicyViolation)
        );
    }

    #[test]
    fn permissions_are_intersected_with_adapter_and_caller_authority_and_can_be_revoked() {
        let permissions = ["state.read".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        let lease = grant_permission_lease(&permissions, &permissions, &permissions, 100, 50, 200)
            .expect("permission lease");
        assert!(lease.allows("state.read", 120));
        assert!(!lease.allows("state.read", 150));
        lease.revoke();
        assert!(!lease.allows("state.read", 120));
        assert!(
            grant_permission_lease(&permissions, &permissions, &BTreeSet::new(), 100, 10, 200)
                .is_err()
        );
    }

    #[test]
    fn extensions_cannot_request_network_permissions_outside_the_exact_origin_allowlist() {
        let mut manifest = manifest();
        manifest.network_destinations =
            ["https://api.example.com".to_owned()].into_iter().collect();
        let policy = policy();
        assert_eq!(
            prepare_adapter(&manifest, &policy),
            Err(ExtensionError::PermissionDenied)
        );
    }
}
