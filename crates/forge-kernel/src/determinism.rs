use forge_contracts::Fingerprint;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capabilities::SideEffectClass;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExecutionEnvelope {
    pub schema_version: u16,
    pub command_contract_id: String,
    pub command_contract_version: String,
    pub input_fingerprint: String,
    pub config_fingerprint: String,
    pub capability_snapshot_fingerprint: String,
    pub dependency_graph_fingerprint: String,
    pub toolchain_fingerprint: String,
    pub logical_time_ms: u64,
    pub deterministic_seed: u64,
    pub side_effect: SideEffectClass,
    pub envelope_fingerprint: String,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DeterminismError {
    #[error("execution envelope identity is invalid")]
    InvalidIdentity,
    #[error("execution envelope could not be fingerprinted")]
    Fingerprint,
}

impl ExecutionEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_contract_id: impl Into<String>,
        command_contract_version: impl Into<String>,
        input_fingerprint: impl Into<String>,
        config_fingerprint: impl Into<String>,
        capability_snapshot_fingerprint: impl Into<String>,
        dependency_graph_fingerprint: impl Into<String>,
        toolchain_fingerprint: impl Into<String>,
        logical_time_ms: u64,
        deterministic_seed: u64,
        side_effect: SideEffectClass,
    ) -> Result<Self, DeterminismError> {
        let command_contract_id = command_contract_id.into();
        let command_contract_version = command_contract_version.into();
        let input_fingerprint = input_fingerprint.into();
        let config_fingerprint = config_fingerprint.into();
        let capability_snapshot_fingerprint = capability_snapshot_fingerprint.into();
        let dependency_graph_fingerprint = dependency_graph_fingerprint.into();
        let toolchain_fingerprint = toolchain_fingerprint.into();
        if command_contract_id.trim().is_empty()
            || command_contract_version.trim().is_empty()
            || [
                &input_fingerprint,
                &config_fingerprint,
                &capability_snapshot_fingerprint,
                &dependency_graph_fingerprint,
                &toolchain_fingerprint,
            ]
            .iter()
            .any(|digest| !valid_digest(digest))
        {
            return Err(DeterminismError::InvalidIdentity);
        }
        let mut envelope = Self {
            schema_version: 1,
            command_contract_id,
            command_contract_version,
            input_fingerprint,
            config_fingerprint,
            capability_snapshot_fingerprint,
            dependency_graph_fingerprint,
            toolchain_fingerprint,
            logical_time_ms,
            deterministic_seed,
            side_effect,
            envelope_fingerprint: String::new(),
        };
        let canonical = serde_json::to_vec(&(
            envelope.schema_version,
            &envelope.command_contract_id,
            &envelope.command_contract_version,
            &envelope.input_fingerprint,
            &envelope.config_fingerprint,
            &envelope.capability_snapshot_fingerprint,
            &envelope.dependency_graph_fingerprint,
            &envelope.toolchain_fingerprint,
            envelope.logical_time_ms,
            envelope.deterministic_seed,
            envelope.side_effect,
        ))
        .map_err(|_| DeterminismError::Fingerprint)?;
        envelope.envelope_fingerprint =
            Fingerprint::from_bytes(forge_contracts::FingerprintKind::Decision, &canonical).value;
        Ok(envelope)
    }

    pub fn replay_is_safe(
        &self,
        observed_effect: SideEffectClass,
        output_digest: &str,
        expected_output_digest: &str,
    ) -> bool {
        observed_effect == SideEffectClass::Pure
            && self.side_effect == SideEffectClass::Pure
            && valid_digest(output_digest)
            && output_digest == expected_output_digest
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(value: &str) -> String {
        blake3::hash(value.as_bytes()).to_hex().to_string()
    }

    #[test]
    fn envelope_fingerprint_binds_semantics_and_external_effects_are_not_replayable() {
        let envelope = ExecutionEnvelope::new(
            "forge.command.build",
            "1.0.0",
            digest("input"),
            digest("config"),
            digest("caps"),
            digest("graph"),
            digest("toolchain"),
            10,
            42,
            SideEffectClass::Pure,
        )
        .expect("envelope");
        assert!(envelope.replay_is_safe(
            SideEffectClass::Pure,
            &digest("result"),
            &digest("result")
        ));
        assert!(!envelope.replay_is_safe(
            SideEffectClass::IrreversibleExternal,
            &digest("result"),
            &digest("result")
        ));
        assert_eq!(envelope.envelope_fingerprint.len(), 64);
    }

    #[test]
    fn raw_or_malformed_hashes_are_rejected() {
        assert!(
            ExecutionEnvelope::new(
                "cmd",
                "1.0.0",
                "secret",
                digest("c"),
                digest("p"),
                digest("g"),
                digest("t"),
                0,
                0,
                SideEffectClass::Pure
            )
            .is_err()
        );
    }
}
