use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleState {
    Registered,
    Staged,
    Starting,
    Ready,
    Degraded,
    Quiescing,
    Stopped,
    Failed,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TransitionEvidence {
    pub contract_valid: bool,
    pub compatibility_proven: bool,
    pub health_ready: bool,
    pub migration_rehearsed: bool,
    pub rollback_available: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum LifecycleError {
    #[error("module lifecycle transition is not permitted")]
    InvalidTransition,
    #[error("required lifecycle evidence is missing")]
    EvidenceMissing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleLifecycle {
    pub module_id: String,
    pub state: ModuleState,
    pub generation: u64,
}

impl ModuleLifecycle {
    pub fn new(module_id: impl Into<String>) -> Result<Self, LifecycleError> {
        let module_id = module_id.into();
        if module_id.trim().is_empty() || module_id.len() > 128 {
            return Err(LifecycleError::InvalidTransition);
        }
        Ok(Self {
            module_id,
            state: ModuleState::Registered,
            generation: 0,
        })
    }

    pub fn transition(
        &mut self,
        next: ModuleState,
        evidence: TransitionEvidence,
    ) -> Result<(), LifecycleError> {
        let permitted = matches!(
            (self.state, next),
            (ModuleState::Registered, ModuleState::Staged)
                | (ModuleState::Staged, ModuleState::Starting)
                | (ModuleState::Starting, ModuleState::Ready)
                | (ModuleState::Starting, ModuleState::Degraded)
                | (ModuleState::Starting, ModuleState::Failed)
                | (ModuleState::Ready, ModuleState::Degraded)
                | (ModuleState::Ready, ModuleState::Quiescing)
                | (ModuleState::Degraded, ModuleState::Ready)
                | (ModuleState::Degraded, ModuleState::Quiescing)
                | (ModuleState::Degraded, ModuleState::Failed)
                | (ModuleState::Quiescing, ModuleState::Stopped)
                | (ModuleState::Quiescing, ModuleState::Failed)
                | (ModuleState::Failed, ModuleState::Staged)
                | (ModuleState::Failed, ModuleState::Quarantined)
                | (ModuleState::Quarantined, ModuleState::Stopped)
                | (ModuleState::Stopped, ModuleState::Staged)
        );
        if !permitted {
            return Err(LifecycleError::InvalidTransition);
        }
        if next == ModuleState::Ready
            && !(evidence.contract_valid && evidence.compatibility_proven && evidence.health_ready)
        {
            return Err(LifecycleError::EvidenceMissing);
        }
        if matches!(
            (self.state, next),
            (
                ModuleState::Ready | ModuleState::Degraded,
                ModuleState::Quiescing
            )
        ) && !evidence.rollback_available
        {
            return Err(LifecycleError::EvidenceMissing);
        }
        if next == ModuleState::Starting
            && self.state == ModuleState::Staged
            && !evidence.contract_valid
        {
            return Err(LifecycleError::EvidenceMissing);
        }
        if self.state == ModuleState::Quiescing
            && next == ModuleState::Stopped
            && !evidence.migration_rehearsed
        {
            return Err(LifecycleError::EvidenceMissing);
        }
        self.state = next;
        self.generation = self.generation.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_requires_contract_compatibility_and_health_evidence() {
        let mut module = ModuleLifecycle::new("forge.state").expect("module id");
        module
            .transition(ModuleState::Staged, TransitionEvidence::default())
            .expect("stage");
        module
            .transition(
                ModuleState::Starting,
                TransitionEvidence {
                    contract_valid: true,
                    ..Default::default()
                },
            )
            .expect("start");
        assert_eq!(
            module.transition(ModuleState::Ready, TransitionEvidence::default()),
            Err(LifecycleError::EvidenceMissing)
        );
        module
            .transition(
                ModuleState::Ready,
                TransitionEvidence {
                    contract_valid: true,
                    compatibility_proven: true,
                    health_ready: true,
                    ..Default::default()
                },
            )
            .expect("ready");
    }

    #[test]
    fn active_module_cannot_quiesce_without_rollback_boundary() {
        let mut module = ModuleLifecycle::new("forge.kernel").expect("module id");
        module
            .transition(ModuleState::Staged, TransitionEvidence::default())
            .expect("stage");
        module
            .transition(
                ModuleState::Starting,
                TransitionEvidence {
                    contract_valid: true,
                    ..Default::default()
                },
            )
            .expect("start");
        module
            .transition(
                ModuleState::Ready,
                TransitionEvidence {
                    contract_valid: true,
                    compatibility_proven: true,
                    health_ready: true,
                    ..Default::default()
                },
            )
            .expect("ready");
        assert_eq!(
            module.transition(ModuleState::Quiescing, TransitionEvidence::default()),
            Err(LifecycleError::EvidenceMissing)
        );
    }
}
