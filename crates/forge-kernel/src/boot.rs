use std::collections::BTreeMap;
use std::path::PathBuf;

use forge_state::{ForgeStateStore, StateError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::capabilities::{
    CapabilityDescriptor, CapabilityRegistry, EvidenceLevel, HealthState, Locality, PrivacyClass,
    SideEffectClass,
};
use crate::health::{HealthRegistry, ProbeObservation, ProbePolicy};
use crate::runtime::{KernelRuntime, RuntimeConfig, RuntimeError};
use crate::telemetry::HotPathRegistry;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootState {
    NativeReady,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionalServiceState {
    Disabled,
    NotConfigured,
}

#[derive(Clone, Debug)]
pub struct NativeBootConfig {
    pub data_directory: PathBuf,
    pub runtime: RuntimeConfig,
    pub telemetry_metric_capacity: usize,
}

impl NativeBootConfig {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            data_directory: data_directory.into(),
            runtime: RuntimeConfig::default(),
            telemetry_metric_capacity: 256,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BootReport {
    pub state: BootState,
    pub readiness: HealthState,
    pub readiness_fingerprint: String,
    pub offline_ready: bool,
    pub state_root: String,
    pub database_integrity: String,
    pub database_schema_version: u16,
    pub capability_epoch: u64,
    pub capability_snapshot_fingerprint: String,
    pub optional_services: BTreeMap<String, OptionalServiceState>,
    pub semantic_embeddings: String,
    pub boot_fingerprint: String,
}

pub struct NativeBoot {
    pub report: BootReport,
    pub state_store: ForgeStateStore,
    pub runtime: KernelRuntime,
    pub capabilities: CapabilityRegistry,
    pub telemetry: HotPathRegistry,
}

#[derive(Debug, Error)]
pub enum BootError {
    #[error("native boot configuration is invalid")]
    InvalidConfiguration,
    #[error("native state store failed to open or validate")]
    State(#[from] StateError),
    #[error("native runtime could not start")]
    Runtime(#[from] RuntimeError),
    #[error("native boot evidence could not be fingerprinted")]
    Fingerprint,
}

pub fn boot_native(config: NativeBootConfig) -> Result<NativeBoot, BootError> {
    if config.data_directory.as_os_str().is_empty()
        || config.telemetry_metric_capacity == 0
        || config.telemetry_metric_capacity > 10_000
    {
        return Err(BootError::InvalidConfiguration);
    }
    let telemetry = HotPathRegistry::new(config.telemetry_metric_capacity)
        .map_err(|_| BootError::InvalidConfiguration)?;
    let runtime = KernelRuntime::new(config.runtime)?;
    let state_store = runtime.block_on(async {
        let store = ForgeStateStore::open(&config.data_directory).await?;
        store.integrity_check().await?;
        store
            .put_canonical("kernel", "boot.phase", &json!("native_ready"))
            .await?;
        Ok::<_, StateError>(store)
    })?;

    let observed_at_ms = now_ms();
    let mut health = HealthRegistry::default();
    health
        .register(ProbePolicy {
            check_id: "forge.state.integrity".into(),
            required: true,
            maximum_age_ms: 30_000,
            degradation_after_failures: 1,
        })
        .map_err(|_| BootError::InvalidConfiguration)?;
    health
        .record(ProbeObservation {
            check_id: "forge.state.integrity".into(),
            passed: true,
            observed_at_ms,
            latency_ms: 0,
            failure_code: None,
        })
        .map_err(|_| BootError::InvalidConfiguration)?;
    let readiness = health
        .readiness(observed_at_ms)
        .map_err(|_| BootError::Fingerprint)?;

    let capabilities = CapabilityRegistry::new();
    capabilities
        .register(CapabilityDescriptor {
            id: "forge.native.state.sqlite".into(),
            capability_id: "forge.state.local".into(),
            version: "1.0.0".into(),
            contract_id: "forge.contract.state-store".into(),
            evidence: EvidenceLevel::RuntimeVerified,
            health: HealthState::Ready,
            health_observed_at_ms: observed_at_ms,
            health_max_age_ms: 30_000,
            privacy: PrivacyClass::Internal,
            side_effect: SideEffectClass::LocalMutation,
            deterministic: true,
            quality_basis_points: 10_000,
            latency_ms: 0,
            token_cost: 0,
            monetary_cost_micros: 0,
            locality: Locality::Native,
        })
        .map_err(|_| BootError::InvalidConfiguration)?;
    let capability_snapshot = capabilities
        .snapshot()
        .map_err(|_| BootError::Fingerprint)?;
    let state_root = state_store.root().to_string_lossy().into_owned();
    let optional_services = BTreeMap::from([
        ("hive".to_owned(), OptionalServiceState::NotConfigured),
        (
            "semantic_embeddings".to_owned(),
            OptionalServiceState::Disabled,
        ),
    ]);
    let semantic_embeddings = "disabled";
    let state = if readiness.ready {
        BootState::NativeReady
    } else {
        BootState::Degraded
    };
    let database_schema_version = 1;
    let offline_ready = true;
    let report_input = serde_json::to_vec(&(
        "forge-native-boot-v1",
        state,
        readiness.state,
        &readiness.digest,
        offline_ready,
        &state_root,
        "ok",
        database_schema_version,
        capability_snapshot.epoch,
        &capability_snapshot.digest,
        &optional_services,
        semantic_embeddings,
    ))
    .map_err(|_| BootError::Fingerprint)?;
    let report = BootReport {
        state,
        readiness: readiness.state,
        readiness_fingerprint: readiness.digest,
        offline_ready,
        state_root,
        database_integrity: "ok".to_owned(),
        database_schema_version,
        capability_epoch: capability_snapshot.epoch,
        capability_snapshot_fingerprint: capability_snapshot.digest,
        optional_services,
        semantic_embeddings: semantic_embeddings.to_owned(),
        boot_fingerprint: blake3::hash(&report_input).to_hex().to_string(),
    };
    Ok(NativeBoot {
        report,
        state_store,
        runtime,
        capabilities,
        telemetry,
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_boot_is_offline_and_does_not_require_hive_or_embeddings() {
        let root = tempfile::tempdir().expect("temporary state");
        let boot = boot_native(NativeBootConfig::new(root.path())).expect("native boot");
        assert_eq!(boot.report.state, BootState::NativeReady);
        assert!(boot.report.offline_ready);
        assert_eq!(boot.report.semantic_embeddings, "disabled");
        assert_eq!(
            boot.report.optional_services["hive"],
            OptionalServiceState::NotConfigured
        );
        assert_eq!(boot.report.database_integrity, "ok");
    }
}
