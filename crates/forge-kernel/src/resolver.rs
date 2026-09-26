use serde::{Deserialize, Serialize};

use crate::capabilities::{
    CapabilityDescriptor, CapabilitySnapshot, EvidenceLevel, HealthState, Locality, PrivacyClass,
    SideEffectClass,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionRequest {
    pub capability_id: String,
    pub max_privacy: PrivacyClass,
    pub max_side_effect: SideEffectClass,
    pub max_latency_ms: u32,
    pub max_tokens: u32,
    pub max_cost_micros: u64,
    pub require_determinism: bool,
    pub allow_remote: bool,
    pub now_ms: u64,
    pub minimum_evidence: EvidenceLevel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CandidateDecision {
    pub provider_id: String,
    pub eligible: bool,
    pub rejection_codes: Vec<String>,
    pub score: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResolutionProof {
    pub snapshot_epoch: u64,
    pub snapshot_digest: String,
    pub request_fingerprint: String,
    pub selected_provider: Option<String>,
    pub abstained: bool,
    pub candidates: Vec<CandidateDecision>,
}

pub fn resolve(snapshot: &CapabilitySnapshot, request: &ResolutionRequest) -> ResolutionProof {
    let candidates = snapshot
        .entries
        .values()
        .filter(|descriptor| descriptor.capability_id == request.capability_id)
        .map(|descriptor| evaluate_candidate(descriptor, request))
        .collect::<Vec<_>>();
    let selected_provider = candidates
        .iter()
        .filter(|candidate| candidate.eligible)
        .max_by(|left, right| {
            left.score
                .cmp(&right.score)
                .then_with(|| right.provider_id.cmp(&left.provider_id))
        })
        .map(|candidate| candidate.provider_id.clone());
    let request_fingerprint = fingerprint_request(snapshot, request);
    ResolutionProof {
        snapshot_epoch: snapshot.epoch,
        snapshot_digest: snapshot.digest.clone(),
        request_fingerprint,
        abstained: selected_provider.is_none(),
        selected_provider,
        candidates,
    }
}

fn fingerprint_request(snapshot: &CapabilitySnapshot, request: &ResolutionRequest) -> String {
    let privacy = format!("{:?}", request.max_privacy);
    let effect = format!("{:?}", request.max_side_effect);
    let evidence = format!("{:?}", request.minimum_evidence);
    let latency = request.max_latency_ms.to_string();
    let tokens = request.max_tokens.to_string();
    let cost = request.max_cost_micros.to_string();
    let determinism = request.require_determinism.to_string();
    let remote = request.allow_remote.to_string();
    let now = request.now_ms.to_string();
    let mut bytes = Vec::new();
    for part in [
        request.capability_id.as_bytes(),
        privacy.as_bytes(),
        effect.as_bytes(),
        latency.as_bytes(),
        tokens.as_bytes(),
        cost.as_bytes(),
        determinism.as_bytes(),
        remote.as_bytes(),
        now.as_bytes(),
        evidence.as_bytes(),
        snapshot.digest.as_bytes(),
    ] {
        bytes.extend_from_slice(&(part.len() as u64).to_be_bytes());
        bytes.extend_from_slice(part);
    }
    blake3::hash(&bytes).to_hex().to_string()
}

fn evaluate_candidate(
    descriptor: &CapabilityDescriptor,
    request: &ResolutionRequest,
) -> CandidateDecision {
    let mut rejection_codes = Vec::new();
    if descriptor.evidence < request.minimum_evidence
        || descriptor.evidence == EvidenceLevel::Quarantined
    {
        rejection_codes.push("EVIDENCE_INSUFFICIENT".to_owned());
    }
    let fresh = descriptor.health_observed_at_ms <= request.now_ms
        && request.now_ms - descriptor.health_observed_at_ms <= descriptor.health_max_age_ms;
    if !fresh
        || !matches!(
            descriptor.health,
            HealthState::Ready | HealthState::Degraded
        )
    {
        rejection_codes.push("HEALTH_NOT_FRESH_READY".to_owned());
    }
    if descriptor.privacy > request.max_privacy {
        rejection_codes.push("PRIVACY_LIMIT".to_owned());
    }
    if descriptor.side_effect > request.max_side_effect {
        rejection_codes.push("SIDE_EFFECT_LIMIT".to_owned());
    }
    if descriptor.latency_ms > request.max_latency_ms {
        rejection_codes.push("DEADLINE_LIMIT".to_owned());
    }
    if descriptor.token_cost > request.max_tokens {
        rejection_codes.push("TOKEN_BUDGET".to_owned());
    }
    if descriptor.monetary_cost_micros > request.max_cost_micros {
        rejection_codes.push("COST_BUDGET".to_owned());
    }
    if request.require_determinism && !descriptor.deterministic {
        rejection_codes.push("DETERMINISM_REQUIRED".to_owned());
    }
    if !request.allow_remote && descriptor.locality == Locality::Remote {
        rejection_codes.push("REMOTE_DISALLOWED".to_owned());
    }
    let eligible = rejection_codes.is_empty();
    let score = eligible.then(|| {
        u64::from(descriptor.quality_basis_points) * 1_000_000
            + u64::from(10_000_u32.saturating_sub(descriptor.latency_ms.min(10_000))) * 100
            + u64::from(10_000_u32.saturating_sub(descriptor.token_cost.min(10_000)))
    });
    CandidateDecision {
        provider_id: descriptor.id.clone(),
        eligible,
        rejection_codes,
        score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::CapabilityRegistry;

    fn make(
        id: &str,
        privacy: PrivacyClass,
        locality: Locality,
        quality: u16,
    ) -> CapabilityDescriptor {
        CapabilityDescriptor {
            id: id.into(),
            capability_id: "decision.route".into(),
            version: "1.0.0".into(),
            contract_id: "forge.decision.test".into(),
            evidence: EvidenceLevel::ConformancePassed,
            health: HealthState::Ready,
            health_observed_at_ms: 100,
            health_max_age_ms: 500,
            privacy,
            side_effect: SideEffectClass::Pure,
            deterministic: true,
            quality_basis_points: quality,
            latency_ms: 10,
            token_cost: 0,
            monetary_cost_micros: 0,
            locality,
        }
    }

    fn request() -> ResolutionRequest {
        ResolutionRequest {
            capability_id: "decision.route".into(),
            max_privacy: PrivacyClass::Internal,
            max_side_effect: SideEffectClass::Pure,
            max_latency_ms: 100,
            max_tokens: 100,
            max_cost_micros: 100,
            require_determinism: true,
            allow_remote: false,
            now_ms: 120,
            minimum_evidence: EvidenceLevel::ConformancePassed,
        }
    }

    fn register_verified_fixture(
        registry: &CapabilityRegistry,
        mut descriptor: CapabilityDescriptor,
    ) {
        let id = descriptor.id.clone();
        let evidence = descriptor.evidence;
        let health = descriptor.health;
        let observed_at_ms = descriptor.health_observed_at_ms;
        descriptor.evidence = EvidenceLevel::Declared;
        descriptor.health = HealthState::Unknown;
        descriptor.health_observed_at_ms = 0;
        registry.register(descriptor).expect("register declaration");
        registry
            .record_verified_runtime_evidence(&id, evidence, health, observed_at_ms)
            .expect("record trusted test evidence");
    }

    #[test]
    fn hard_privacy_and_locality_constraints_are_filtered_before_scoring() {
        let registry = CapabilityRegistry::new();
        register_verified_fixture(
            &registry,
            make(
                "provider.high-but-sensitive",
                PrivacyClass::Sensitive,
                Locality::Native,
                10_000,
            ),
        );
        register_verified_fixture(
            &registry,
            make(
                "provider.remote",
                PrivacyClass::Internal,
                Locality::Remote,
                1,
            ),
        );
        register_verified_fixture(
            &registry,
            make(
                "provider.local",
                PrivacyClass::Internal,
                Locality::Native,
                9000,
            ),
        );
        let proof = resolve(&registry.snapshot().expect("snapshot"), &request());
        assert_eq!(proof.selected_provider.as_deref(), Some("provider.local"));
        assert!(
            proof
                .candidates
                .iter()
                .all(|candidate| candidate.eligible || candidate.score.is_none())
        );
        assert!(proof.candidates.iter().any(|candidate| {
            candidate
                .rejection_codes
                .contains(&"PRIVACY_LIMIT".to_owned())
        }));
    }

    #[test]
    fn stale_or_unknown_health_abstains_instead_of_guessing() {
        let registry = CapabilityRegistry::new();
        register_verified_fixture(
            &registry,
            make(
                "provider.local",
                PrivacyClass::Internal,
                Locality::Native,
                9000,
            ),
        );
        let mut stale = request();
        stale.now_ms = 10_000;
        let proof = resolve(&registry.snapshot().expect("snapshot"), &stale);
        assert!(proof.abstained);
        assert!(proof.selected_provider.is_none());
    }
}
