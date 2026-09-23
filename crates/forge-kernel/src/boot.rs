use std::collections::BTreeMap;
use std::path::PathBuf;

use forge_state::{
    ForgeStateStore, PersistedResourceUsageRecord, PersistedResourceVector, StateError,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::capabilities::{
    CapabilityDescriptor, CapabilityRegistry, EvidenceLevel, HealthState, Locality, PrivacyClass,
    SideEffectClass,
};
use crate::health::{HealthRegistry, ProbeObservation, ProbePolicy};
use crate::resources::{ResourceError, ResourceGovernor, ResourceLease, ResourceVector};
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
    pub resource_limits: ResourceVector,
    pub survival_reserve: ResourceVector,
}

impl NativeBootConfig {
    pub fn new(data_directory: impl Into<PathBuf>) -> Self {
        Self {
            data_directory: data_directory.into(),
            runtime: RuntimeConfig::default(),
            telemetry_metric_capacity: 256,
            resource_limits: ResourceVector::default(),
            survival_reserve: ResourceVector::default(),
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
    pub resource_usage_fingerprint: String,
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
    resource_governor: ResourceGovernor,
    usage_write_lock: tokio::sync::Mutex<()>,
}

impl NativeBoot {
    pub fn try_resource_lease(
        &self,
        owner: impl Into<String>,
        requested: ResourceVector,
    ) -> Result<ResourceLease, ResourceError> {
        self.resource_governor.try_lease(owner, requested)
    }

    pub fn try_survival_lease(
        &self,
        owner: impl Into<String>,
        requested: ResourceVector,
    ) -> Result<ResourceLease, ResourceError> {
        self.resource_governor.try_survival_lease(owner, requested)
    }

    pub fn consumed_resource_budget(&self) -> ResourceVector {
        self.resource_governor.consumed_budget()
    }

    pub async fn account_resource_usage(
        &self,
        lease: &ResourceLease,
        record: &PersistedResourceUsageRecord,
    ) -> Result<bool, BootError> {
        if record.owner != lease.owner() || record.pool != lease.pool() {
            return Err(BootError::InvalidConfiguration);
        }
        let used = ResourceVector {
            cpu_millis: record.used.cpu_millis,
            memory_bytes: record.used.memory_bytes,
            disk_bytes: record.used.disk_bytes,
            network_bytes: record.used.network_bytes,
            tokens: record.used.tokens,
            cost_micros: record.used.cost_micros,
        };
        let _guard = self.usage_write_lock.lock().await;
        if let Some(existing) = self
            .state_store
            .get_resource_usage_record(&record.usage_id)
            .await?
        {
            if existing != *record {
                return Err(StateError::ResourceUsageConflict.into());
            }
            return Ok(false);
        }
        lease.validate_charge(&record.category, used, record.cache_tokens_reused)?;
        let pool_limit = lease.pool_limit()?;
        let durable_pool_limit = PersistedResourceVector {
            cpu_millis: pool_limit.cpu_millis,
            memory_bytes: pool_limit.memory_bytes,
            disk_bytes: pool_limit.disk_bytes,
            network_bytes: pool_limit.network_bytes,
            tokens: pool_limit.tokens,
            cost_micros: pool_limit.cost_micros,
        };
        let inserted = match self
            .state_store
            .record_resource_usage(record, &durable_pool_limit)
            .await
        {
            Ok(inserted) => inserted,
            Err(StateError::ResourceBudgetExceeded) => {
                return Err(ResourceError::BudgetExceeded.into());
            }
            Err(error) => return Err(error.into()),
        };
        if !inserted {
            return Ok(false);
        }
        lease.charge(&record.category, used, record.cache_tokens_reused)?;
        Ok(true)
    }
}

#[derive(Debug, Error)]
pub enum BootError {
    #[error("native boot configuration is invalid")]
    InvalidConfiguration,
    #[error("native state store failed to open or validate")]
    State(#[from] StateError),
    #[error("native runtime could not start")]
    Runtime(#[from] RuntimeError),
    #[error("native resource budget configuration or usage is invalid")]
    Resource(#[from] ResourceError),
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
    let persisted_usage = runtime.block_on(state_store.resource_usage_records())?;
    let usage_bytes = serde_json::to_vec(&persisted_usage).map_err(|_| BootError::Fingerprint)?;
    let resource_usage_fingerprint = blake3::hash(&usage_bytes).to_hex().to_string();
    let resource_governor = ResourceGovernor::from_persisted_usage(
        config.resource_limits,
        config.survival_reserve,
        &persisted_usage,
    )?;
    let database_schema_version = state_store.schema_version();
    let offline_ready = true;
    let report_input = serde_json::to_vec(&(
        "forge-native-boot-v2",
        state,
        readiness.state,
        &readiness.digest,
        offline_ready,
        &state_root,
        "ok",
        database_schema_version,
        &resource_usage_fingerprint,
        capability_snapshot.epoch,
        &capability_snapshot.digest,
        &optional_services,
        semantic_embeddings,
        config.resource_limits,
        config.survival_reserve,
        resource_governor.consumed_budget(),
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
        resource_usage_fingerprint,
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
        resource_governor,
        usage_write_lock: tokio::sync::Mutex::new(()),
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
    use forge_state::{ResourceUsageAttribution, ResourceUsagePool};

    fn usage_record(id: &str) -> PersistedResourceUsageRecord {
        PersistedResourceUsageRecord {
            usage_id: blake3::hash(id.as_bytes()).to_hex().to_string(),
            pool: ResourceUsagePool::Ordinary,
            owner: "forge.command".into(),
            category: "provider.inference".into(),
            used: PersistedResourceVector {
                tokens: 25,
                cost_micros: 200,
                ..Default::default()
            },
            cache_tokens_reused: 0,
            attribution: ResourceUsageAttribution {
                project_id: "hive-forge".into(),
                work_order_id: "FGE-004-M00".into(),
                execution_id: "run-001".into(),
                capability_id: "forge.ai.inference".into(),
                provider_id: Some("provider.test".into()),
            },
        }
    }

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
        assert_eq!(boot.report.database_schema_version, 2);
    }

    #[test]
    fn live_resource_accounting_survives_restart_and_duplicate_replay_is_not_charged_twice() {
        let root = tempfile::tempdir().expect("temporary state");
        let mut config = NativeBootConfig::new(root.path());
        config.resource_limits = ResourceVector {
            tokens: 100,
            cost_micros: 1_000,
            ..Default::default()
        };
        let record = usage_record("provider-request-1");
        {
            let boot = boot_native(config.clone()).expect("first boot");
            let lease = boot
                .try_resource_lease(
                    "forge.command",
                    ResourceVector {
                        tokens: 100,
                        cost_micros: 1_000,
                        ..Default::default()
                    },
                )
                .expect("budget lease");
            assert!(
                boot.runtime
                    .block_on(boot.account_resource_usage(&lease, &record))
                    .expect("persist and account usage")
            );
            assert!(
                !boot
                    .runtime
                    .block_on(boot.account_resource_usage(&lease, &record))
                    .expect("deduplicate replay")
            );
            assert_eq!(
                boot.consumed_resource_budget(),
                ResourceVector {
                    tokens: 25,
                    cost_micros: 200,
                    ..Default::default()
                }
            );
        }

        let boot = boot_native(config).expect("restart from durable state");
        assert_eq!(
            boot.consumed_resource_budget(),
            ResourceVector {
                tokens: 25,
                cost_micros: 200,
                ..Default::default()
            }
        );
        let records = boot
            .runtime
            .block_on(boot.state_store.resource_usage_records())
            .expect("read recovered ledger");
        assert_eq!(records.len(), 1);
        let replay_lease = boot
            .try_resource_lease(
                "forge.command",
                ResourceVector {
                    tokens: 1,
                    ..Default::default()
                },
            )
            .expect("small lease for replay");
        assert!(
            !boot
                .runtime
                .block_on(boot.account_resource_usage(&replay_lease, &record))
                .expect("replay does not need a second budget allocation")
        );
        assert_eq!(
            boot.try_resource_lease(
                "remaining-budget",
                ResourceVector {
                    tokens: 76,
                    ..Default::default()
                }
            )
            .err(),
            Some(ResourceError::BudgetExceeded)
        );
    }

    #[test]
    fn concurrent_boots_share_the_durable_resource_budget_ceiling() {
        let root = tempfile::tempdir().expect("temporary state");
        let mut config = NativeBootConfig::new(root.path());
        config.resource_limits = ResourceVector {
            tokens: 37,
            cost_micros: 125,
            ..Default::default()
        };
        let first_boot = boot_native(config.clone()).expect("first runtime");
        let second_boot = boot_native(config.clone()).expect("second runtime");
        let first_lease = first_boot
            .try_resource_lease(
                "forge.command",
                ResourceVector {
                    tokens: 37,
                    cost_micros: 125,
                    ..Default::default()
                },
            )
            .expect("first local grant");
        let second_lease = second_boot
            .try_resource_lease(
                "forge.command",
                ResourceVector {
                    tokens: 37,
                    cost_micros: 125,
                    ..Default::default()
                },
            )
            .expect("second local grant before shared accounting");
        let mut first_record = usage_record("runtime-budget-race-a");
        first_record.used.tokens = 37;
        first_record.used.cost_micros = 125;
        let mut second_record = usage_record("runtime-budget-race-b");
        second_record.used.tokens = 37;
        second_record.used.cost_micros = 125;
        let scheduler = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test scheduler");
        let (first, second) = scheduler.block_on(async {
            tokio::join!(
                first_boot.account_resource_usage(&first_lease, &first_record),
                second_boot.account_resource_usage(&second_lease, &second_record)
            )
        });
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
                .filter(|result| {
                    matches!(
                        result,
                        Err(BootError::Resource(ResourceError::BudgetExceeded))
                    )
                })
                .count(),
            1
        );
        drop(first_boot);
        drop(second_boot);

        let restored = boot_native(config).expect("restart shared resource state");
        assert_eq!(
            restored.consumed_resource_budget(),
            ResourceVector {
                tokens: 37,
                cost_micros: 125,
                ..Default::default()
            }
        );
    }
}
