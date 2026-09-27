use std::fmt::{Display, Formatter};

use jsonschema::Validator;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::fingerprint::{Fingerprint, FingerprintKind};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractId(String);

impl ContractId {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
            && !value.contains("..")
            && !value.starts_with('/')
            && !value.ends_with('/');
        if valid {
            Ok(Self(value))
        } else {
            Err(ContractError::InvalidId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContractId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ContractVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ContractVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl Display for ContractVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDefinition {
    pub id: ContractId,
    pub version: ContractVersion,
    pub owner: String,
    pub schema: Value,
}

pub struct ValidatedContract {
    definition: ContractDefinition,
    validator: Validator,
    fingerprint: Fingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ContractViolation {
    pub contract_id: ContractId,
    pub version: ContractVersion,
    pub invalid_paths: Vec<String>,
    pub additional_error_count: usize,
}

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("contract id is invalid")]
    InvalidId,
    #[error("contract owner is empty or invalid")]
    InvalidOwner,
    #[error("contract schema contains an external reference")]
    ExternalReference,
    #[error("contract schema is invalid")]
    InvalidSchema,
    #[error("contract fingerprint serialization failed")]
    FingerprintSerialization,
}

impl ValidatedContract {
    pub fn compile(definition: ContractDefinition) -> Result<Self, ContractError> {
        if definition.owner.trim().is_empty() || definition.owner.len() > 128 {
            return Err(ContractError::InvalidOwner);
        }
        if contains_external_reference(&definition.schema) {
            return Err(ContractError::ExternalReference);
        }
        let validator = jsonschema::validator_for(&definition.schema)
            .map_err(|_| ContractError::InvalidSchema)?;
        let fingerprint = Fingerprint::from_json(
            FingerprintKind::Contract,
            &serde_json::to_value(&definition)
                .map_err(|_| ContractError::FingerprintSerialization)?,
        )
        .map_err(|_| ContractError::FingerprintSerialization)?;
        Ok(Self {
            definition,
            validator,
            fingerprint,
        })
    }

    pub fn definition(&self) -> &ContractDefinition {
        &self.definition
    }

    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    pub fn validate(&self, instance: &Value) -> Result<(), ContractViolation> {
        let mut invalid_paths = Vec::new();
        let mut additional_error_count = 0;
        for error in self.validator.iter_errors(instance) {
            if invalid_paths.len() < 32 {
                invalid_paths.push(error.instance_path().to_string());
            } else {
                additional_error_count += 1;
            }
        }
        if invalid_paths.is_empty() {
            return Ok(());
        }
        Err(ContractViolation {
            contract_id: self.definition.id.clone(),
            version: self.definition.version,
            invalid_paths,
            additional_error_count,
        })
    }
}

fn contains_external_reference(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            (key == "$ref" || key == "$dynamicRef")
                && child
                    .as_str()
                    .is_some_and(|reference| !reference.starts_with('#'))
                || contains_external_reference(child)
        }),
        Value::Array(items) => items.iter().any(contains_external_reference),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn contract(schema: Value) -> ContractDefinition {
        ContractDefinition {
            id: ContractId::new("forge.command.local").expect("valid id"),
            version: ContractVersion::new(1, 0, 0),
            owner: "forge.kernel".to_owned(),
            schema,
        }
    }

    #[test]
    fn validates_positive_and_negative_instances_without_echoing_values() {
        let contract = ValidatedContract::compile(contract(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "required": ["name"],
            "additionalProperties": false
        })))
        .expect("valid contract");

        assert!(contract.validate(&json!({"name": "safe"})).is_ok());
        let violation = contract
            .validate(&json!({"name": {"secret": "not-echoed"}}))
            .expect_err("bad type");
        assert_eq!(violation.invalid_paths, vec!["/name"]);
        assert!(!format!("{violation:?}").contains("not-echoed"));
    }

    #[test]
    fn external_schema_references_are_rejected_before_validation() {
        let definition = contract(json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$ref": "https://example.invalid/schema.json"
        }));
        assert!(matches!(
            ValidatedContract::compile(definition),
            Err(ContractError::ExternalReference)
        ));
    }

    #[test]
    fn contract_ids_reject_traversal_and_untrusted_path_shapes() {
        assert!(ContractId::new("forge.contract.v1").is_ok());
        assert!(ContractId::new("forge/../../secret").is_err());
        assert!(ContractId::new("/rooted").is_err());
    }
}
