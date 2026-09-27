use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fingerprint::{Fingerprint, FingerprintKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    Validation,
    Configuration,
    Contract,
    Compatibility,
    Authentication,
    Authorization,
    Resource,
    Capacity,
    Timeout,
    Cancelled,
    Dependency,
    Storage,
    StateConflict,
    Migration,
    Integrity,
    Security,
    Provider,
    Toolchain,
    Build,
    Test,
    Execution,
    Unavailable,
    UnknownOutcome,
    InternalBug,
    Corruption,
    Policy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retryability {
    Never,
    ImmediateSafe,
    BackoffSafe,
    AfterDependencyRecovery,
    AfterUserAction,
    AfterStateReconciliation,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorOutcome {
    NotStarted,
    NoEffect,
    Committed,
    RolledBack,
    Partial,
    UnknownOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error_code: String,
    pub error_class: ErrorClass,
    pub retryability: Retryability,
    pub outcome: ErrorOutcome,
    pub operation: String,
    pub provider: Option<String>,
    pub safe_message: String,
    pub failure_fingerprint: Fingerprint,
}

impl ErrorEnvelope {
    pub fn new(
        error_code: impl Into<String>,
        error_class: ErrorClass,
        retryability: Retryability,
        outcome: ErrorOutcome,
        operation: impl Into<String>,
        provider: Option<String>,
        safe_message: impl Into<String>,
    ) -> Self {
        let error_code = error_code.into();
        let operation = operation.into();
        let class_name = format!("{error_class:?}");
        let (provider_present, provider_value) = match provider.as_deref() {
            Some(value) => ("1", value),
            None => ("0", ""),
        };
        let mut fingerprint_bytes = Vec::new();
        for part in [
            error_code.as_bytes(),
            class_name.as_bytes(),
            operation.as_bytes(),
            provider_present.as_bytes(),
            provider_value.as_bytes(),
        ] {
            fingerprint_bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
            fingerprint_bytes.extend_from_slice(part);
        }
        let failure_fingerprint =
            Fingerprint::from_bytes(FingerprintKind::Content, &fingerprint_bytes);
        Self {
            error_code,
            error_class,
            retryability,
            outcome,
            operation,
            provider,
            safe_message: safe_message.into(),
            failure_fingerprint,
        }
    }
}

#[derive(Debug, Error)]
pub enum ForgeError {
    #[error("validation failed")]
    Validation,
    #[error("contract rejected the operation")]
    Contract,
    #[error("operation is not authorized")]
    Authorization,
    #[error("resource budget is exhausted")]
    Resource,
    #[error("operation deadline expired")]
    Timeout,
    #[error("operation was cancelled")]
    Cancelled,
    #[error("provider is unavailable")]
    Unavailable,
    #[error("external effect outcome is unknown")]
    UnknownOutcome,
    #[error("state integrity check failed")]
    Integrity,
    #[error("state operation failed")]
    Storage,
    #[error("policy rejected the operation")]
    Policy,
    #[error("internal operation failed")]
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_fingerprint_excludes_message_noise_and_payloads() {
        let first = ErrorEnvelope::new(
            "FORGE.COMMAND.REJECTED",
            ErrorClass::Policy,
            Retryability::Never,
            ErrorOutcome::NoEffect,
            "command.dispatch",
            None,
            "request 1 rejected",
        );
        let second = ErrorEnvelope::new(
            "FORGE.COMMAND.REJECTED",
            ErrorClass::Policy,
            Retryability::Never,
            ErrorOutcome::NoEffect,
            "command.dispatch",
            None,
            "request 2 rejected",
        );
        assert_eq!(first.failure_fingerprint, second.failure_fingerprint);
    }
}
