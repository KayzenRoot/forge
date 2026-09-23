use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use forge_contracts::{ContractId, ContractVersion, ValidatedContract};
use forge_state::{ForgeStateStore, IdempotencyState, StateError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::time::timeout_at;

use crate::capabilities::SideEffectClass;
use crate::determinism::{DeterminismError, ExecutionEnvelope};
use crate::events::{EventBus, EventClass, EventEnvelope, EventError};
use crate::resources::{ResourceError, ResourceLease};
use crate::runtime::{CancellationToken, Deadline};

pub type HandlerFuture =
    Pin<Box<dyn Future<Output = Result<HandlerOutput, CommandFailure>> + Send + 'static>>;
pub type CommandHandler =
    Arc<dyn Fn(CommandContext, Value) -> HandlerFuture + Send + Sync + 'static>;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdempotencyMode {
    None,
    CallerKeyed,
    ForgeDerived,
    StateTransitionGuarded,
    ProviderNative,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitivityClass {
    Public,
    Internal,
    Sensitive,
    Secret,
}

#[derive(Clone)]
pub struct CommandRegistration {
    pub contract: Arc<ValidatedContract>,
    pub required_permissions: BTreeSet<String>,
    pub side_effect: SideEffectClass,
    pub idempotency: IdempotencyMode,
    pub emits_events: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StateTransitionGuard {
    pub owner: String,
    pub key: String,
    pub expected_version: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct CommandRequest {
    pub command_id: String,
    pub principal_id: String,
    pub contract_id: ContractId,
    pub contract_version: ContractVersion,
    pub idempotency_key: Option<String>,
    pub config_fingerprint: String,
    pub capability_snapshot_fingerprint: String,
    pub dependency_graph_fingerprint: String,
    pub toolchain_fingerprint: String,
    pub logical_time_ms: u64,
    pub deterministic_seed: u64,
    pub sensitivity: SensitivityClass,
    pub state_guard: Option<StateTransitionGuard>,
    pub payload: Value,
}

#[derive(Clone, Debug)]
pub struct AuthorizationContext {
    principal_id: String,
    permissions: BTreeSet<String>,
}

impl AuthorizationContext {
    /// Construct this value only from the host application's authenticated permission decision.
    pub fn from_trusted_host(
        principal_id: impl Into<String>,
        permissions: BTreeSet<String>,
    ) -> Result<Self, CommandError> {
        let principal_id = principal_id.into();
        if !valid_token(&principal_id)
            || permissions
                .iter()
                .any(|permission| !valid_permission(permission))
        {
            return Err(CommandError::InvalidRequest);
        }
        Ok(Self {
            principal_id,
            permissions,
        })
    }
}

#[derive(Clone)]
pub struct CommandContext {
    pub command_id: String,
    pub cancellation: CancellationToken,
    pub deadline: Deadline,
    pub execution: ExecutionEnvelope,
    pub resource_lease: Option<ResourceLease>,
}

pub struct HandlerOutput {
    pub value: Value,
    pub commit_evidence_fingerprint: Option<String>,
    pub pending_events: Vec<EventEnvelope>,
    pub state_update: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandFailure {
    pub code: String,
    pub retryable: bool,
    pub outcome_unknown: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutcome {
    pub value: Option<Value>,
    pub replayed: bool,
    pub identity_fingerprint: Option<String>,
    pub result_fingerprint: String,
    pub event_ids: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("command request is invalid")]
    InvalidRequest,
    #[error("command bus must be sealed before dispatch")]
    NotSealed,
    #[error("command contract is not registered")]
    NoHandler,
    #[error("command registration is invalid or duplicated")]
    Registration,
    #[error("caller is not authorized for this command")]
    PermissionDenied,
    #[error("command payload violates its contract")]
    ContractViolation,
    #[error("command was already admitted or has a recorded outcome")]
    Duplicate,
    #[error("command has an unresolved external or local outcome")]
    UnknownOutcome,
    #[error("command deadline expired before admission")]
    Deadline,
    #[error("command was cancelled before admission")]
    Cancelled,
    #[error("command lease was revoked")]
    Lease(#[from] ResourceError),
    #[error("side-effecting command requires explicit idempotency semantics")]
    IdempotencyRequired,
    #[error("secret payloads cannot be written to the local idempotency ledger")]
    SecretPersistenceDenied,
    #[error("command result lacks required commit evidence")]
    CommitEvidenceMissing,
    #[error("command execution envelope could not be created")]
    Determinism(#[from] DeterminismError),
    #[error("command state could not be persisted")]
    State(#[from] StateError),
    #[error("command handler failed with a stable code")]
    HandlerFailure(String),
    #[error("command state transition did not match its expected version")]
    StateConflict,
    #[error("command state transition output is missing or invalid")]
    StateTransitionMissing,
    #[error("command declares events but no event bus is configured")]
    EventBusUnavailable,
    #[error("command event could not be safely published")]
    Event(#[from] EventError),
}

#[derive(Default)]
struct Registry {
    sealed: bool,
    handlers: BTreeMap<String, (CommandRegistration, CommandHandler)>,
}

pub struct CommandBus {
    registry: RwLock<Registry>,
    maximum_handlers: usize,
    event_bus: Option<Arc<EventBus>>,
}

impl CommandBus {
    pub fn new(maximum_handlers: usize) -> Result<Self, CommandError> {
        if maximum_handlers == 0 || maximum_handlers > 4096 {
            return Err(CommandError::Registration);
        }
        Ok(Self {
            registry: RwLock::new(Registry::default()),
            maximum_handlers,
            event_bus: None,
        })
    }

    pub fn with_event_bus(
        maximum_handlers: usize,
        event_bus: Arc<EventBus>,
    ) -> Result<Self, CommandError> {
        let mut bus = Self::new(maximum_handlers)?;
        bus.event_bus = Some(event_bus);
        Ok(bus)
    }

    pub fn register(
        &self,
        registration: CommandRegistration,
        handler: CommandHandler,
    ) -> Result<(), CommandError> {
        let contract_id = registration.contract.definition().id.to_string();
        if registration
            .required_permissions
            .iter()
            .any(|permission| !valid_permission(permission))
            || registration.idempotency == IdempotencyMode::None
                && registration.side_effect != SideEffectClass::Pure
            || registration.idempotency == IdempotencyMode::StateTransitionGuarded
                && registration.side_effect != SideEffectClass::LocalMutation
            || registration.emits_events && registration.idempotency == IdempotencyMode::None
        {
            return Err(CommandError::Registration);
        }
        let mut registry = self
            .registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry.sealed
            || registry.handlers.len() >= self.maximum_handlers
            || registry.handlers.contains_key(&contract_id)
        {
            return Err(CommandError::Registration);
        }
        registry
            .handlers
            .insert(contract_id, (registration, handler));
        Ok(())
    }

    pub fn seal(&self) {
        self.registry
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .sealed = true;
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn execute(
        &self,
        store: &ForgeStateStore,
        request: CommandRequest,
        authorization: &AuthorizationContext,
        cancellation: CancellationToken,
        deadline: Deadline,
        lease: Option<ResourceLease>,
    ) -> Result<CommandOutcome, CommandError> {
        let (registration, handler) = {
            let registry = self
                .registry
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !registry.sealed {
                return Err(CommandError::NotSealed);
            }
            registry
                .handlers
                .get(request.contract_id.as_str())
                .cloned()
                .ok_or(CommandError::NoHandler)?
        };
        if !valid_token(&request.command_id)
            || request.principal_id != authorization.principal_id
            || !valid_token(&request.principal_id)
            || request.contract_id != registration.contract.definition().id
            || request.contract_version != registration.contract.definition().version
            || request.sensitivity == SensitivityClass::Secret
                && registration.idempotency != IdempotencyMode::None
            || registration.idempotency == IdempotencyMode::StateTransitionGuarded
                && (request.sensitivity > SensitivityClass::Internal
                    || request
                        .state_guard
                        .as_ref()
                        .is_none_or(|guard| !valid_state_guard(guard)))
            || registration.idempotency != IdempotencyMode::StateTransitionGuarded
                && request.state_guard.is_some()
            || registration.emits_events && request.sensitivity > SensitivityClass::Internal
            || matches!(
                registration.idempotency,
                IdempotencyMode::None
                    | IdempotencyMode::ForgeDerived
                    | IdempotencyMode::StateTransitionGuarded
            ) && request.idempotency_key.is_some()
        {
            return Err(if request.sensitivity == SensitivityClass::Secret {
                CommandError::SecretPersistenceDenied
            } else {
                CommandError::InvalidRequest
            });
        }
        if registration.emits_events && self.event_bus.is_none() {
            return Err(CommandError::EventBusUnavailable);
        }
        if !registration
            .required_permissions
            .is_subset(&authorization.permissions)
        {
            return Err(CommandError::PermissionDenied);
        }
        if registration.contract.validate(&request.payload).is_err() {
            return Err(CommandError::ContractViolation);
        }
        if serde_json::to_vec(&request.payload)
            .map_err(|_| CommandError::InvalidRequest)?
            .len()
            > 1_048_576
        {
            return Err(CommandError::InvalidRequest);
        }
        if cancellation.is_cancelled() {
            return Err(CommandError::Cancelled);
        }
        if deadline.is_expired() {
            return Err(CommandError::Deadline);
        }
        if let Some(lease) = &lease {
            lease.ensure_active()?;
        }

        let input_fingerprint = blake3::hash(
            &serde_json::to_vec(&request.payload).map_err(|_| CommandError::InvalidRequest)?,
        )
        .to_hex()
        .to_string();
        let execution = ExecutionEnvelope::new(
            request.contract_id.to_string(),
            request.contract_version.to_string(),
            input_fingerprint.clone(),
            request.config_fingerprint.clone(),
            request.capability_snapshot_fingerprint.clone(),
            request.dependency_graph_fingerprint.clone(),
            request.toolchain_fingerprint.clone(),
            request.logical_time_ms,
            request.deterministic_seed,
            registration.side_effect,
        )?;
        let identity = idempotency_identity(&request, &registration, &input_fingerprint)?;
        if let Some(identity) = identity.as_deref()
            && let Some(prior) = store.begin_idempotent(identity).await?
        {
            if prior.state == IdempotencyState::UnknownOutcome {
                return Err(CommandError::UnknownOutcome);
            }
            if prior.state == IdempotencyState::Committed {
                if let Some(value) = prior.result {
                    return replayed_outcome(value, identity);
                }
                return Err(CommandError::Duplicate);
            }
            if prior.state != IdempotencyState::FailedRetryable
                || registration.side_effect != SideEffectClass::Pure
                || !store.retry_idempotent(identity).await?
            {
                return Err(CommandError::Duplicate);
            }
        }

        let context = CommandContext {
            command_id: request.command_id,
            cancellation: cancellation.clone(),
            deadline,
            execution,
            resource_lease: lease.clone(),
        };
        let execution_fingerprint = context.execution.envelope_fingerprint.clone();
        let execution_result = tokio::select! {
            result = timeout_at(deadline.instant(), handler(context, request.payload)) => {
                match result {
                    Ok(output) => output,
                    Err(_) => {
                        let pure = registration.side_effect == SideEffectClass::Pure;
                        record_failure(store, identity.as_deref(), registration.side_effect, "FORGE.COMMAND.DEADLINE_UNKNOWN", pure, !pure).await?;
                        return Err(if registration.side_effect == SideEffectClass::Pure { CommandError::Deadline } else { CommandError::UnknownOutcome });
                    }
                }
            }
            _ = cancellation.cancelled() => {
                let pure = registration.side_effect == SideEffectClass::Pure;
                record_failure(store, identity.as_deref(), registration.side_effect, "FORGE.COMMAND.CANCELLED_UNKNOWN", pure, !pure).await?;
                return Err(if registration.side_effect == SideEffectClass::Pure { CommandError::Cancelled } else { CommandError::UnknownOutcome });
            }
        };

        if let Some(lease) = &lease
            && let Err(error) = lease.ensure_active()
        {
            let pure = registration.side_effect == SideEffectClass::Pure;
            record_failure(
                store,
                identity.as_deref(),
                registration.side_effect,
                "FORGE.COMMAND.LEASE_REVOKED",
                pure,
                !pure,
            )
            .await?;
            return Err(if registration.side_effect == SideEffectClass::Pure {
                CommandError::Lease(error)
            } else {
                CommandError::UnknownOutcome
            });
        }

        let output = match execution_result {
            Ok(output) => output,
            Err(failure) => {
                if !valid_code(&failure.code) {
                    record_failure(
                        store,
                        identity.as_deref(),
                        registration.side_effect,
                        "FORGE.COMMAND.INVALID_FAILURE_CODE",
                        false,
                        true,
                    )
                    .await?;
                    return Err(CommandError::UnknownOutcome);
                }
                let unknown_effect =
                    failure.outcome_unknown && registration.side_effect != SideEffectClass::Pure;
                record_failure(
                    store,
                    identity.as_deref(),
                    registration.side_effect,
                    &failure.code,
                    failure.retryable,
                    unknown_effect,
                )
                .await?;
                return Err(if failure.outcome_unknown {
                    CommandError::UnknownOutcome
                } else {
                    CommandError::HandlerFailure(failure.code)
                });
            }
        };
        if registration.side_effect != SideEffectClass::Pure
            && output
                .commit_evidence_fingerprint
                .as_deref()
                .is_none_or(|digest| !valid_digest(digest))
        {
            record_failure(
                store,
                identity.as_deref(),
                registration.side_effect,
                "FORGE.COMMAND.COMMIT_EVIDENCE_MISSING",
                false,
                true,
            )
            .await?;
            return Err(CommandError::CommitEvidenceMissing);
        }
        let result_bytes =
            serde_json::to_vec(&output.value).map_err(|_| CommandError::InvalidRequest)?;
        if result_bytes.len() > 4_194_304 {
            record_failure(
                store,
                identity.as_deref(),
                registration.side_effect,
                "FORGE.COMMAND.RESULT_TOO_LARGE",
                false,
                registration.side_effect != SideEffectClass::Pure,
            )
            .await?;
            return Err(CommandError::InvalidRequest);
        }
        if registration.idempotency == IdempotencyMode::StateTransitionGuarded {
            let guard = request
                .state_guard
                .as_ref()
                .ok_or(CommandError::InvalidRequest)?;
            let Some(state_update) = output.state_update.as_ref() else {
                record_failure(
                    store,
                    identity.as_deref(),
                    registration.side_effect,
                    "FORGE.COMMAND.STATE_TRANSITION_MISSING",
                    false,
                    false,
                )
                .await?;
                return Err(CommandError::StateTransitionMissing);
            };
            if !store
                .compare_and_set_canonical(
                    &guard.owner,
                    &guard.key,
                    guard.expected_version,
                    state_update,
                )
                .await?
            {
                record_failure(
                    store,
                    identity.as_deref(),
                    registration.side_effect,
                    "FORGE.COMMAND.STATE_CONFLICT",
                    false,
                    false,
                )
                .await?;
                return Err(CommandError::StateConflict);
            }
        } else if output.state_update.is_some() {
            record_failure(
                store,
                identity.as_deref(),
                registration.side_effect,
                "FORGE.COMMAND.UNEXPECTED_STATE_TRANSITION",
                false,
                registration.side_effect != SideEffectClass::Pure,
            )
            .await?;
            return Err(CommandError::InvalidRequest);
        }
        if output.pending_events.len() > 1024
            || !registration.emits_events && !output.pending_events.is_empty()
        {
            record_failure(
                store,
                identity.as_deref(),
                registration.side_effect,
                "FORGE.COMMAND.EVENTS_NOT_DECLARED",
                false,
                true,
            )
            .await?;
            return Err(CommandError::InvalidRequest);
        }
        let mut event_ids = Vec::with_capacity(output.pending_events.len());
        if !output.pending_events.is_empty() {
            let event_bus = self
                .event_bus
                .as_ref()
                .ok_or(CommandError::EventBusUnavailable)?;
            let command_identity = identity
                .as_deref()
                .ok_or(CommandError::IdempotencyRequired)?;
            for (index, mut event) in output.pending_events.into_iter().enumerate() {
                event.event_id = format!("command.{command_identity}.{index}");
                let result = match event.class {
                    EventClass::EphemeralLocal | EventClass::Telemetry => event_bus
                        .publish_ephemeral(event)
                        .map(|receipt| receipt.event_id),
                    EventClass::DurableLocal
                    | EventClass::Integration
                    | EventClass::AuditEvidence => event_bus
                        .publish_durable(store, event)
                        .await
                        .map(|receipt| receipt.event_id),
                };
                match result {
                    Ok(event_id) => event_ids.push(event_id),
                    Err(error) => {
                        record_failure(
                            store,
                            identity.as_deref(),
                            registration.side_effect,
                            "FORGE.COMMAND.EVENT_OUTCOME_UNKNOWN",
                            false,
                            true,
                        )
                        .await?;
                        return Err(CommandError::Event(error));
                    }
                }
            }
        }
        let result_fingerprint = blake3::hash(&result_bytes).to_hex().to_string();
        if let Some(identity) = identity.as_deref() {
            let sensitive = request.sensitivity == SensitivityClass::Sensitive;
            let persisted = serde_json::json!({
                "_forgeCommandReceipt": {
                    "schemaVersion": 1,
                    "resultFingerprint": result_fingerprint,
                    "commitEvidenceFingerprint": output.commit_evidence_fingerprint,
                    "eventIds": event_ids,
                    "sensitive": sensitive,
                    "executionFingerprint": execution_fingerprint,
                },
                "value": if sensitive { Value::Null } else { output.value.clone() },
            });
            store.complete_idempotent(identity, &persisted).await?;
        }
        Ok(CommandOutcome {
            value: Some(output.value),
            replayed: false,
            identity_fingerprint: identity,
            result_fingerprint,
            event_ids,
        })
    }
}

fn idempotency_identity(
    request: &CommandRequest,
    registration: &CommandRegistration,
    input_fingerprint: &str,
) -> Result<Option<String>, CommandError> {
    if registration.idempotency == IdempotencyMode::None {
        return Ok(None);
    }
    let key = match registration.idempotency {
        IdempotencyMode::CallerKeyed | IdempotencyMode::ProviderNative => {
            let key = request
                .idempotency_key
                .as_deref()
                .ok_or(CommandError::IdempotencyRequired)?;
            if !valid_token(key) {
                return Err(CommandError::InvalidRequest);
            }
            key
        }
        IdempotencyMode::ForgeDerived => input_fingerprint,
        IdempotencyMode::StateTransitionGuarded => input_fingerprint,
        IdempotencyMode::None => return Ok(None),
    };
    let bytes = serde_json::to_vec(&(
        request.principal_id.as_str(),
        request.contract_id.as_str(),
        request.contract_version,
        key,
        input_fingerprint,
        request.config_fingerprint.as_str(),
        request.capability_snapshot_fingerprint.as_str(),
        request.dependency_graph_fingerprint.as_str(),
        request.toolchain_fingerprint.as_str(),
        request.deterministic_seed,
        registration.contract.fingerprint().value.as_str(),
        registration.idempotency,
        registration.side_effect,
        &request.state_guard,
    ))
    .map_err(|_| CommandError::InvalidRequest)?;
    Ok(Some(blake3::hash(&bytes).to_hex().to_string()))
}

fn replayed_outcome(value: Value, identity: &str) -> Result<CommandOutcome, CommandError> {
    if let Some(receipt) = value.get("_forgeCommandReceipt") {
        if receipt.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
            return Err(CommandError::Duplicate);
        }
        let result_fingerprint = receipt
            .get("resultFingerprint")
            .and_then(Value::as_str)
            .ok_or(CommandError::Duplicate)?;
        if !valid_digest(result_fingerprint) {
            return Err(CommandError::Duplicate);
        }
        let sensitive = receipt
            .get("sensitive")
            .and_then(Value::as_bool)
            .ok_or(CommandError::Duplicate)?;
        let event_ids = receipt
            .get("eventIds")
            .and_then(Value::as_array)
            .ok_or(CommandError::Duplicate)?
            .iter()
            .map(Value::as_str)
            .map(|item| item.ok_or(CommandError::Duplicate).map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        if event_ids.iter().any(|id| !valid_token(id)) {
            return Err(CommandError::Duplicate);
        }
        return Ok(CommandOutcome {
            value: (!sensitive).then(|| value.get("value").cloned()).flatten(),
            replayed: true,
            identity_fingerprint: Some(identity.to_owned()),
            result_fingerprint: result_fingerprint.to_owned(),
            event_ids,
        });
    }
    let result_fingerprint =
        blake3::hash(&serde_json::to_vec(&value).map_err(|_| CommandError::Duplicate)?)
            .to_hex()
            .to_string();
    Ok(CommandOutcome {
        value: Some(value),
        replayed: true,
        identity_fingerprint: Some(identity.to_owned()),
        result_fingerprint,
        event_ids: Vec::new(),
    })
}

async fn record_failure(
    store: &ForgeStateStore,
    identity: Option<&str>,
    effect: SideEffectClass,
    code: &str,
    retryable: bool,
    unknown: bool,
) -> Result<(), CommandError> {
    if let Some(identity) = identity {
        let state = if unknown || effect != SideEffectClass::Pure && retryable {
            IdempotencyState::UnknownOutcome
        } else if retryable {
            IdempotencyState::FailedRetryable
        } else {
            IdempotencyState::FailedFinal
        };
        store.fail_idempotent(identity, state, code).await?;
    }
    Ok(())
}

fn valid_token(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn valid_permission(value: &str) -> bool {
    valid_token(value) && value.contains('.')
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_state_guard(guard: &StateTransitionGuard) -> bool {
    valid_token(&guard.owner)
        && valid_token(&guard.key)
        && guard
            .expected_version
            .is_none_or(|version| version > 0 && version <= i64::MAX as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_contracts::{ContractDefinition, ContractId};
    use serde_json::json;

    fn digest(value: &str) -> String {
        blake3::hash(value.as_bytes()).to_hex().to_string()
    }

    fn registration(mode: IdempotencyMode, side_effect: SideEffectClass) -> CommandRegistration {
        CommandRegistration {
            contract: Arc::new(ValidatedContract::compile(ContractDefinition {
                id: ContractId::new("forge.command.set-state").expect("id"),
                version: ContractVersion::new(1, 0, 0),
                owner: "forge.kernel".into(),
                schema: json!({"type":"object","properties":{"value":{"type":"integer"}},"required":["value"],"additionalProperties":false}),
            }).expect("contract")),
            required_permissions: ["state.write".to_owned()].into_iter().collect(),
            side_effect,
            idempotency: mode,
            emits_events: false,
        }
    }

    fn request() -> CommandRequest {
        CommandRequest {
            command_id: "cmd-1".into(),
            principal_id: "user-1".into(),
            contract_id: ContractId::new("forge.command.set-state").expect("id"),
            contract_version: ContractVersion::new(1, 0, 0),
            idempotency_key: Some("caller-key-1".into()),
            config_fingerprint: digest("config"),
            capability_snapshot_fingerprint: digest("capability"),
            dependency_graph_fingerprint: digest("graph"),
            toolchain_fingerprint: digest("toolchain"),
            logical_time_ms: 10,
            deterministic_seed: 12,
            sensitivity: SensitivityClass::Internal,
            state_guard: None,
            payload: json!({"value": 2}),
        }
    }

    #[tokio::test]
    async fn permission_contract_and_durable_idempotency_precede_side_effect_execution() {
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let bus = CommandBus::new(4).expect("bus");
        let called = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let called_handler = Arc::clone(&called);
        bus.register(
            registration(IdempotencyMode::CallerKeyed, SideEffectClass::LocalMutation),
            Arc::new(move |_context, payload| {
                called_handler.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Box::pin(async move {
                    Ok(HandlerOutput {
                        value: payload,
                        commit_evidence_fingerprint: Some(digest("committed")),
                        pending_events: vec![],
                        state_update: None,
                    })
                })
            }),
        )
        .expect("register");
        bus.seal();
        let permissions = ["state.write".to_owned()].into_iter().collect();
        let authorization =
            AuthorizationContext::from_trusted_host("user-1", permissions).expect("auth");
        let first = bus
            .execute(
                &store,
                request(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(2)),
                None,
            )
            .await
            .expect("first execution");
        let replay = bus
            .execute(
                &store,
                request(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(2)),
                None,
            )
            .await
            .expect("replay");
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(called.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn side_effects_without_idempotency_and_missing_permissions_are_rejected() {
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let bus = CommandBus::new(4).expect("bus");
        let handler: CommandHandler = Arc::new(|_context, _payload| {
            Box::pin(async {
                Ok(HandlerOutput {
                    value: Value::Null,
                    commit_evidence_fingerprint: None,
                    pending_events: vec![],
                    state_update: None,
                })
            })
        });
        assert!(matches!(
            bus.register(
                registration(IdempotencyMode::None, SideEffectClass::LocalMutation),
                handler
            ),
            Err(CommandError::Registration)
        ));
        bus.register(
            registration(IdempotencyMode::CallerKeyed, SideEffectClass::LocalMutation),
            Arc::new(|_, _| {
                Box::pin(async {
                    Ok(HandlerOutput {
                        value: Value::Null,
                        commit_evidence_fingerprint: Some(digest("ok")),
                        pending_events: vec![],
                        state_update: None,
                    })
                })
            }),
        )
        .expect("register");
        bus.seal();
        let authorization =
            AuthorizationContext::from_trusted_host("user-1", BTreeSet::new()).expect("auth");
        assert!(matches!(
            bus.execute(
                &store,
                request(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None
            )
            .await,
            Err(CommandError::PermissionDenied)
        ));
    }

    #[tokio::test]
    async fn only_a_declared_pure_retryable_failure_can_return_to_in_flight() {
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let bus = CommandBus::new(4).expect("bus");
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let handler_attempts = Arc::clone(&attempts);
        bus.register(
            registration(IdempotencyMode::CallerKeyed, SideEffectClass::Pure),
            Arc::new(move |_context, payload| {
                let attempt = handler_attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Box::pin(async move {
                    if attempt == 0 {
                        Err(CommandFailure {
                            code: "TEMPORARY_FAILURE".into(),
                            retryable: true,
                            outcome_unknown: false,
                        })
                    } else {
                        Ok(HandlerOutput {
                            value: payload,
                            commit_evidence_fingerprint: None,
                            pending_events: vec![],
                            state_update: None,
                        })
                    }
                })
            }),
        )
        .expect("register");
        bus.seal();
        let authorization = AuthorizationContext::from_trusted_host(
            "user-1",
            ["state.write".to_owned()].into_iter().collect(),
        )
        .expect("auth");
        assert!(matches!(
            bus.execute(
                &store,
                request(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None
            )
            .await,
            Err(CommandError::HandlerFailure(_))
        ));
        let outcome = bus
            .execute(
                &store,
                request(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None,
            )
            .await
            .expect("safe retry");
        assert!(!outcome.replayed);
        assert_eq!(attempts.load(std::sync::atomic::Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn sensitive_results_are_not_saved_as_replayable_payloads() {
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let bus = CommandBus::new(4).expect("bus");
        bus.register(
            registration(IdempotencyMode::CallerKeyed, SideEffectClass::Pure),
            Arc::new(|_context, _payload| {
                Box::pin(async {
                    Ok(HandlerOutput {
                        value: json!({"secret": "do-not-persist"}),
                        commit_evidence_fingerprint: None,
                        pending_events: vec![],
                        state_update: None,
                    })
                })
            }),
        )
        .expect("register");
        bus.seal();
        let authorization = AuthorizationContext::from_trusted_host(
            "user-1",
            ["state.write".to_owned()].into_iter().collect(),
        )
        .expect("auth");
        let mut request = request();
        request.sensitivity = SensitivityClass::Sensitive;
        bus.execute(
            &store,
            request.clone(),
            &authorization,
            CancellationToken::new(),
            Deadline::after(std::time::Duration::from_secs(1)),
            None,
        )
        .await
        .expect("first call");
        let input_fingerprint =
            blake3::hash(&serde_json::to_vec(&request.payload).expect("payload bytes"))
                .to_hex()
                .to_string();
        let identity = idempotency_identity(
            &request,
            &registration(IdempotencyMode::CallerKeyed, SideEffectClass::Pure),
            &input_fingerprint,
        )
        .expect("identity")
        .expect("durable key");
        let record = store
            .begin_idempotent(&identity)
            .await
            .expect("read")
            .expect("record");
        let serialized = serde_json::to_string(&record.result).expect("encode receipt");
        assert!(!serialized.contains("do-not-persist"));
        let replay = bus
            .execute(
                &store,
                request,
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None,
            )
            .await
            .expect("deduplicated");
        assert!(replay.replayed);
        assert_eq!(replay.value, None);
    }

    #[tokio::test]
    async fn state_transition_guard_commits_once_and_rejects_a_stale_absent_version() {
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let bus = CommandBus::new(4).expect("bus");
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let handler_calls = Arc::clone(&calls);
        bus.register(
            registration(
                IdempotencyMode::StateTransitionGuarded,
                SideEffectClass::LocalMutation,
            ),
            Arc::new(move |_context, payload| {
                handler_calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Box::pin(async move {
                    Ok(HandlerOutput {
                        value: payload.clone(),
                        commit_evidence_fingerprint: Some(digest("state-transition")),
                        pending_events: vec![],
                        state_update: Some(payload),
                    })
                })
            }),
        )
        .expect("register");
        bus.seal();
        let authorization = AuthorizationContext::from_trusted_host(
            "user-1",
            ["state.write".to_owned()].into_iter().collect(),
        )
        .expect("auth");
        let mut request = request();
        request.idempotency_key = None;
        request.state_guard = Some(StateTransitionGuard {
            owner: "project-a".into(),
            key: "settings".into(),
            expected_version: None,
        });
        let first = bus
            .execute(
                &store,
                request.clone(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None,
            )
            .await
            .expect("state transition");
        let replay = bus
            .execute(
                &store,
                request.clone(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None,
            )
            .await
            .expect("replay");
        assert!(!first.replayed);
        assert!(replay.replayed);
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(
            store
                .get_canonical("project-a", "settings")
                .await
                .expect("read state"),
            Some(json!({"value": 2}))
        );

        request.payload = json!({"value": 3});
        assert!(matches!(
            bus.execute(
                &store,
                request,
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None,
            )
            .await,
            Err(CommandError::StateConflict)
        ));
        assert_eq!(
            store
                .get_canonical("project-a", "settings")
                .await
                .expect("state remains unchanged"),
            Some(json!({"value": 2}))
        );
    }

    #[tokio::test]
    async fn durable_command_events_are_published_after_handler_completion_and_replayed_once() {
        let root = tempfile::tempdir().expect("temp dir");
        let store = ForgeStateStore::open(root.path()).await.expect("store");
        let event_bus = Arc::new(
            EventBus::new([(crate::events::EventLane::DurableDomain, 8)]).expect("event bus"),
        );
        let bus = CommandBus::with_event_bus(4, Arc::clone(&event_bus)).expect("command bus");
        let mut registration =
            registration(IdempotencyMode::CallerKeyed, SideEffectClass::LocalMutation);
        registration.emits_events = true;
        bus.register(
            registration,
            Arc::new(|_context, payload| {
                Box::pin(async move {
                    Ok(HandlerOutput {
                        value: payload,
                        commit_evidence_fingerprint: Some(digest("committed")),
                        pending_events: vec![EventEnvelope {
                            event_id: "command.completed".into(),
                            contract_id: "forge.event.command.completed".into(),
                            contract_version: "1.0.0".into(),
                            class: EventClass::DurableLocal,
                            lane: crate::events::EventLane::DurableDomain,
                            producer: "forge.kernel".into(),
                            privacy: crate::capabilities::PrivacyClass::Internal,
                            causation_id: Some("cmd-1".into()),
                            correlation_id: None,
                            execution_id: Some("exec-1".into()),
                            occurred_at_ms: 10,
                            payload: json!({"completed": true}),
                        }],
                        state_update: None,
                    })
                })
            }),
        )
        .expect("register");
        bus.seal();
        let authorization = AuthorizationContext::from_trusted_host(
            "user-1",
            ["state.write".to_owned()].into_iter().collect(),
        )
        .expect("auth");
        let first = bus
            .execute(
                &store,
                request(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None,
            )
            .await
            .expect("command");
        let replay = bus
            .execute(
                &store,
                request(),
                &authorization,
                CancellationToken::new(),
                Deadline::after(std::time::Duration::from_secs(1)),
                None,
            )
            .await
            .expect("replay");
        assert_eq!(first.event_ids.len(), 1);
        assert_eq!(replay.event_ids, first.event_ids);
        assert_eq!(store.pending_events(10).await.expect("outbox").len(), 1);
    }
}
