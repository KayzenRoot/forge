use std::collections::BTreeSet;

use forge_contracts::{ContractId, ContractVersion};
use forge_state::{EvidenceRecord, ForgeStateStore, StateError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityDimension {
    ContractShape,
    Semantic,
    Behavioral,
    Capability,
    StateSchema,
    Migration,
    ProtocolWire,
    SdkFep,
    Configuration,
    PlatformArchitecture,
    Toolchain,
    SecurityPermission,
    PerformanceBudget,
    EvidenceProvenance,
}

impl CompatibilityDimension {
    pub const ALL: [Self; 14] = [
        Self::ContractShape,
        Self::Semantic,
        Self::Behavioral,
        Self::Capability,
        Self::StateSchema,
        Self::Migration,
        Self::ProtocolWire,
        Self::SdkFep,
        Self::Configuration,
        Self::PlatformArchitecture,
        Self::Toolchain,
        Self::SecurityPermission,
        Self::PerformanceBudget,
        Self::EvidenceProvenance,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DimensionVerdict {
    Compatible,
    AdapterRequired,
    MigrationRequired,
    Conditional,
    Incompatible,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityVerdict {
    Compatible,
    CompatibleWithAdapter,
    CompatibleWithMigration,
    ConditionallyCompatible,
    Incompatible,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityDirection {
    OldProducerToNewConsumer,
    NewProducerToOldConsumer,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DimensionEvidence {
    pub dimension: CompatibilityDimension,
    pub verdict: DimensionVerdict,
    pub evidence_fingerprint: Option<String>,
    pub reason_code: String,
    pub conditions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityRequest {
    pub proof_id: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
    pub rule_engine_version: String,
    pub direction: CompatibilityDirection,
    pub dimensions: Vec<DimensionEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityProof {
    pub proof_id: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
    pub rule_engine_version: String,
    pub direction: CompatibilityDirection,
    pub dimensions: Vec<DimensionEvidence>,
    pub verdict: CompatibilityVerdict,
    pub conditions: Vec<String>,
    pub reason_codes: Vec<String>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DirectionalCompatibilityProof {
    pub old_to_new: CompatibilityProof,
    pub new_to_old: CompatibilityProof,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityRange {
    pub contract_id: ContractId,
    pub major: u16,
    pub minimum_minor: u16,
    pub maximum_minor: Option<u16>,
    pub minimum_patch: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VersionRangeAssessment {
    pub contract_id: ContractId,
    pub required_major: u16,
    pub provider: ContractVersion,
    pub range_satisfied: bool,
    pub migration_required: bool,
    pub reason_codes: Vec<String>,
    pub fingerprint: String,
}

#[derive(Debug, Error)]
pub enum CompatibilityError {
    #[error("compatibility input or dimension evidence is invalid")]
    InvalidEvidence,
    #[error("compatibility evidence could not be fingerprinted")]
    Fingerprint,
    #[error("compatibility proof persistence failed")]
    State(#[from] StateError),
}

pub fn evaluate(request: CompatibilityRequest) -> Result<CompatibilityProof, CompatibilityError> {
    if !valid_token(&request.proof_id)
        || !valid_digest(&request.old_fingerprint)
        || !valid_digest(&request.new_fingerprint)
        || !valid_token(&request.rule_engine_version)
    {
        return Err(CompatibilityError::InvalidEvidence);
    }
    let mut dimensions = request.dimensions;
    dimensions.sort_by_key(|item| item.dimension);
    let ids = dimensions
        .iter()
        .map(|item| item.dimension)
        .collect::<BTreeSet<_>>();
    if ids.len() != dimensions.len() || dimensions.len() > CompatibilityDimension::ALL.len() {
        return Err(CompatibilityError::InvalidEvidence);
    }

    let mut reason_codes = Vec::new();
    let mut conditions = BTreeSet::new();
    let mut has_adapter = false;
    let mut has_migration = false;
    let mut has_conditional = false;
    let mut has_incompatible = false;
    let mut has_unknown = ids.len() != CompatibilityDimension::ALL.len();
    if has_unknown {
        reason_codes.push("MISSING_DIMENSION_COVERAGE".to_owned());
    }
    for dimension in &dimensions {
        if !valid_code(&dimension.reason_code)
            || dimension
                .conditions
                .iter()
                .any(|condition| !valid_code(condition))
            || dimension
                .evidence_fingerprint
                .as_deref()
                .is_some_and(|fingerprint| !valid_digest(fingerprint))
        {
            return Err(CompatibilityError::InvalidEvidence);
        }
        if dimension.verdict != DimensionVerdict::Unknown
            && dimension.evidence_fingerprint.is_none()
        {
            has_unknown = true;
            reason_codes.push("DIMENSION_EVIDENCE_MISSING".to_owned());
        }
        match dimension.verdict {
            DimensionVerdict::Compatible => {}
            DimensionVerdict::AdapterRequired => has_adapter = true,
            DimensionVerdict::MigrationRequired => has_migration = true,
            DimensionVerdict::Conditional => has_conditional = true,
            DimensionVerdict::Incompatible => has_incompatible = true,
            DimensionVerdict::Unknown => has_unknown = true,
        }
        reason_codes.push(dimension.reason_code.clone());
        conditions.extend(dimension.conditions.iter().cloned());
    }
    reason_codes.sort();
    reason_codes.dedup();
    let verdict = if has_incompatible {
        CompatibilityVerdict::Incompatible
    } else if has_unknown {
        CompatibilityVerdict::Unknown
    } else if has_migration {
        CompatibilityVerdict::CompatibleWithMigration
    } else if has_adapter {
        CompatibilityVerdict::CompatibleWithAdapter
    } else if has_conditional {
        CompatibilityVerdict::ConditionallyCompatible
    } else {
        CompatibilityVerdict::Compatible
    };
    let conditions = conditions.into_iter().collect::<Vec<_>>();
    let proof_input = serde_json::to_vec(&(
        1_u16,
        &request.proof_id,
        &request.old_fingerprint,
        &request.new_fingerprint,
        &request.rule_engine_version,
        request.direction,
        &dimensions,
        verdict,
        &conditions,
        &reason_codes,
    ))
    .map_err(|_| CompatibilityError::Fingerprint)?;
    Ok(CompatibilityProof {
        proof_id: request.proof_id,
        old_fingerprint: request.old_fingerprint,
        new_fingerprint: request.new_fingerprint,
        rule_engine_version: request.rule_engine_version,
        direction: request.direction,
        dimensions,
        verdict,
        conditions,
        reason_codes,
        fingerprint: blake3::hash(&proof_input).to_hex().to_string(),
    })
}

pub fn evaluate_directions(
    old_to_new: CompatibilityRequest,
    new_to_old: CompatibilityRequest,
) -> Result<DirectionalCompatibilityProof, CompatibilityError> {
    if old_to_new.direction != CompatibilityDirection::OldProducerToNewConsumer
        || new_to_old.direction != CompatibilityDirection::NewProducerToOldConsumer
        || old_to_new.old_fingerprint != new_to_old.old_fingerprint
        || old_to_new.new_fingerprint != new_to_old.new_fingerprint
    {
        return Err(CompatibilityError::InvalidEvidence);
    }
    let old_to_new = evaluate(old_to_new)?;
    let new_to_old = evaluate(new_to_old)?;
    let bytes = serde_json::to_vec(&(&old_to_new, &new_to_old))
        .map_err(|_| CompatibilityError::Fingerprint)?;
    Ok(DirectionalCompatibilityProof {
        old_to_new,
        new_to_old,
        fingerprint: blake3::hash(&bytes).to_hex().to_string(),
    })
}

pub async fn persist_proof(
    store: &ForgeStateStore,
    proof: &CompatibilityProof,
) -> Result<(), CompatibilityError> {
    let payload = serde_json::to_value(proof).map_err(|_| CompatibilityError::Fingerprint)?;
    store
        .record_evidence(&EvidenceRecord {
            evidence_id: format!(
                "compat.{}.{}",
                proof.proof_id,
                direction_code(proof.direction)
            ),
            kind: "compatibility_proof".to_owned(),
            fingerprint: proof.fingerprint.clone(),
            payload,
        })
        .await?;
    Ok(())
}

pub fn assess_version_range(
    range: &CompatibilityRange,
    provider: ContractVersion,
    migration_available: bool,
) -> Result<VersionRangeAssessment, CompatibilityError> {
    if range
        .maximum_minor
        .is_some_and(|maximum| maximum < range.minimum_minor)
    {
        return Err(CompatibilityError::InvalidEvidence);
    }
    let major_matches = provider.major == range.major;
    let above_minimum = provider.minor > range.minimum_minor
        || provider.minor == range.minimum_minor && provider.patch >= range.minimum_patch;
    let below_maximum = range
        .maximum_minor
        .is_none_or(|maximum| provider.minor <= maximum);
    let version_range_satisfied = major_matches && above_minimum && below_maximum;
    let migration_required = !version_range_satisfied && migration_available;
    let reason_codes = if version_range_satisfied {
        vec!["VERSION_RANGE_SATISFIED_METADATA_ONLY".to_owned()]
    } else if migration_required {
        vec!["VERSION_RANGE_REQUIRES_MIGRATION".to_owned()]
    } else if !major_matches {
        vec!["MAJOR_VERSION_MISMATCH".to_owned()]
    } else {
        vec!["VERSION_OUTSIDE_DECLARED_RANGE".to_owned()]
    };
    let bytes = serde_json::to_vec(&(range, provider, &reason_codes))
        .map_err(|_| CompatibilityError::Fingerprint)?;
    Ok(VersionRangeAssessment {
        contract_id: range.contract_id.clone(),
        required_major: range.major,
        provider,
        range_satisfied: version_range_satisfied,
        migration_required,
        reason_codes,
        fingerprint: blake3::hash(&bytes).to_hex().to_string(),
    })
}

fn direction_code(direction: CompatibilityDirection) -> &'static str {
    match direction {
        CompatibilityDirection::OldProducerToNewConsumer => "old-to-new",
        CompatibilityDirection::NewProducerToOldConsumer => "new-to-old",
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_token(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(value: &str) -> String {
        blake3::hash(value.as_bytes()).to_hex().to_string()
    }

    fn dimensions(verdict: DimensionVerdict) -> Vec<DimensionEvidence> {
        CompatibilityDimension::ALL
            .into_iter()
            .map(|dimension| DimensionEvidence {
                dimension,
                verdict,
                evidence_fingerprint: Some(digest(&format!("{dimension:?}"))),
                reason_code: "EVIDENCE_BOUND".into(),
                conditions: vec![],
            })
            .collect()
    }

    fn request(
        direction: CompatibilityDirection,
        dimensions: Vec<DimensionEvidence>,
    ) -> CompatibilityRequest {
        CompatibilityRequest {
            proof_id: "upgrade-42".into(),
            old_fingerprint: digest("old"),
            new_fingerprint: digest("new"),
            rule_engine_version: "fce-1".into(),
            direction,
            dimensions,
        }
    }

    #[test]
    fn every_dimension_needs_evidence_and_unknown_never_becomes_compatible() {
        let incomplete = request(
            CompatibilityDirection::OldProducerToNewConsumer,
            dimensions(DimensionVerdict::Compatible),
        );
        let mut incomplete = incomplete;
        incomplete.dimensions.pop();
        assert_eq!(
            evaluate(incomplete).expect("proof").verdict,
            CompatibilityVerdict::Unknown
        );

        let unknown = request(
            CompatibilityDirection::OldProducerToNewConsumer,
            dimensions(DimensionVerdict::Unknown),
        );
        assert_eq!(
            evaluate(unknown).expect("proof").verdict,
            CompatibilityVerdict::Unknown
        );
    }

    #[test]
    fn migration_and_security_dimensions_veto_automatic_compatibility() {
        let mut evidence = dimensions(DimensionVerdict::Compatible);
        let migration = evidence
            .iter_mut()
            .find(|item| item.dimension == CompatibilityDimension::Migration)
            .expect("migration dimension");
        migration.verdict = DimensionVerdict::MigrationRequired;
        migration.conditions.push("RESTORE_REHEARSED".into());
        let proof = evaluate(request(
            CompatibilityDirection::OldProducerToNewConsumer,
            evidence,
        ))
        .expect("proof");
        assert_eq!(proof.verdict, CompatibilityVerdict::CompatibleWithMigration);
        assert_eq!(proof.conditions, vec!["RESTORE_REHEARSED"]);

        let mut evidence = dimensions(DimensionVerdict::Compatible);
        evidence
            .iter_mut()
            .find(|item| item.dimension == CompatibilityDimension::SecurityPermission)
            .expect("security dimension")
            .verdict = DimensionVerdict::Incompatible;
        assert_eq!(
            evaluate(request(
                CompatibilityDirection::OldProducerToNewConsumer,
                evidence
            ))
            .expect("proof")
            .verdict,
            CompatibilityVerdict::Incompatible
        );
    }

    #[test]
    fn semver_range_is_metadata_and_directional_proofs_are_separate() {
        let range = CompatibilityRange {
            contract_id: ContractId::new("forge.event.lifecycle").expect("contract id"),
            major: 1,
            minimum_minor: 2,
            maximum_minor: Some(4),
            minimum_patch: 1,
        };
        let version = assess_version_range(&range, ContractVersion::new(1, 3, 0), false)
            .expect("version metadata");
        assert!(version.range_satisfied);
        assert!(version.reason_codes[0].contains("METADATA_ONLY"));
        let directional = evaluate_directions(
            request(
                CompatibilityDirection::OldProducerToNewConsumer,
                dimensions(DimensionVerdict::Compatible),
            ),
            request(
                CompatibilityDirection::NewProducerToOldConsumer,
                dimensions(DimensionVerdict::Unknown),
            ),
        )
        .expect("directions");
        assert_eq!(
            directional.old_to_new.verdict,
            CompatibilityVerdict::Compatible
        );
        assert_eq!(
            directional.new_to_old.verdict,
            CompatibilityVerdict::Unknown
        );
    }
}
