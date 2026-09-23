use std::collections::{BTreeMap, BTreeSet};

use forge_state::{EvidenceRecord, ForgeStateStore, StateError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofKind {
    Static,
    Contract,
    Unit,
    Property,
    Integration,
    StateMigration,
    Concurrency,
    FaultRecovery,
    Compatibility,
    ReplayDeterminism,
    OfflineSovereign,
    EndToEnd,
    ManualPolicy,
    Test,
    Security,
    Performance,
    Build,
    Review,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofOutcome {
    Passed,
    Failed,
    Unknown,
    Invalidated,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofNode {
    pub proof_id: String,
    pub kind: ProofKind,
    pub outcome: ProofOutcome,
    pub fingerprint: String,
    pub head_sha: String,
    pub dependencies: Vec<String>,
    pub obligation_ids: Vec<String>,
    pub executor_session_id: Option<String>,
    pub reviewer_session_id: Option<String>,
    pub metadata: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofRisk {
    Low,
    Standard,
    Elevated,
    High,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofObligation {
    pub obligation_id: String,
    pub required_kinds: BTreeSet<ProofKind>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChangeAssessment {
    pub exact_head_sha: String,
    pub risk: ProofRisk,
    pub impact_unknown: bool,
    pub obligations: Vec<ProofObligation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledObligations {
    pub exact_head_sha: String,
    pub digest: String,
    pub risk: ProofRisk,
    pub obligations: Vec<ProofObligation>,
    pub broadened_for_unknown_impact: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofGap {
    pub obligation_id: String,
    pub missing_kinds: Vec<ProofKind>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CertificationReport {
    pub exact_head_sha: String,
    pub selected_proof_ids: Vec<String>,
    pub gaps: Vec<ProofGap>,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProofSnapshot {
    pub head_sha: String,
    pub graph_digest: String,
    pub nodes: Vec<ProofNode>,
    pub all_nodes_passed: bool,
}

#[derive(Debug, Error)]
pub enum ProofError {
    #[error("proof node is invalid or duplicated")]
    InvalidNode,
    #[error("proof dependency is missing or cyclic")]
    MissingDependency,
    #[error("proof serialization or hashing failed")]
    Fingerprint,
    #[error("proof evidence persistence failed")]
    State(#[from] StateError),
    #[error("proof selection exceeded its bounded exact-search budget")]
    SearchLimit,
}

#[derive(Default)]
pub struct ProofObligationCompiler;

impl ProofObligationCompiler {
    pub fn compile(&self, assessment: ChangeAssessment) -> Result<CompiledObligations, ProofError> {
        if !valid_digest(&assessment.exact_head_sha)
            || assessment.obligations.is_empty()
            || assessment.obligations.len() > 256
        {
            return Err(ProofError::InvalidNode);
        }
        let mut obligations = assessment.obligations;
        let mut ids = BTreeSet::new();
        if obligations.iter().any(|obligation| {
            !valid_token(&obligation.obligation_id)
                || obligation.required_kinds.is_empty()
                || !ids.insert(obligation.obligation_id.clone())
        }) {
            return Err(ProofError::InvalidNode);
        }
        if assessment.risk == ProofRisk::High {
            obligations.push(ProofObligation {
                obligation_id: "policy.high_assurance".into(),
                required_kinds: [
                    ProofKind::Contract,
                    ProofKind::FaultRecovery,
                    ProofKind::Security,
                    ProofKind::Performance,
                    ProofKind::ReplayDeterminism,
                    ProofKind::Review,
                ]
                .into_iter()
                .collect(),
            });
        }
        if assessment.impact_unknown {
            obligations.push(ProofObligation {
                obligation_id: "changecone.unknown_impact".into(),
                required_kinds: [
                    ProofKind::Static,
                    ProofKind::Contract,
                    ProofKind::Unit,
                    ProofKind::Property,
                    ProofKind::Integration,
                    ProofKind::StateMigration,
                    ProofKind::Concurrency,
                    ProofKind::FaultRecovery,
                    ProofKind::Security,
                    ProofKind::Compatibility,
                    ProofKind::Performance,
                    ProofKind::ReplayDeterminism,
                    ProofKind::OfflineSovereign,
                    ProofKind::EndToEnd,
                ]
                .into_iter()
                .collect(),
            });
        }
        if obligations
            .iter()
            .map(|obligation| &obligation.obligation_id)
            .collect::<BTreeSet<_>>()
            .len()
            != obligations.len()
        {
            return Err(ProofError::InvalidNode);
        }
        let bytes = serde_json::to_vec(&(
            &assessment.exact_head_sha,
            assessment.risk,
            assessment.impact_unknown,
            &obligations,
        ))
        .map_err(|_| ProofError::Fingerprint)?;
        Ok(CompiledObligations {
            exact_head_sha: assessment.exact_head_sha,
            digest: blake3::hash(&bytes).to_hex().to_string(),
            risk: assessment.risk,
            obligations,
            broadened_for_unknown_impact: assessment.impact_unknown,
        })
    }
}

#[derive(Default)]
pub struct ProofGraph {
    nodes: BTreeMap<String, ProofNode>,
}

impl ProofGraph {
    pub fn add(&mut self, node: ProofNode) -> Result<(), ProofError> {
        if !valid_token(&node.proof_id)
            || !valid_digest(&node.fingerprint)
            || !valid_digest(&node.head_sha)
            || node.dependencies.len() > 128
            || node.dependencies.iter().collect::<BTreeSet<_>>().len() != node.dependencies.len()
            || node.obligation_ids.len() > 256
            || node.obligation_ids.iter().any(|id| !valid_token(id))
            || node.obligation_ids.iter().collect::<BTreeSet<_>>().len()
                != node.obligation_ids.len()
            || node.kind == ProofKind::Review
                && (node
                    .executor_session_id
                    .as_deref()
                    .is_none_or(|id| !valid_token(id))
                    || node
                        .reviewer_session_id
                        .as_deref()
                        .is_none_or(|id| !valid_token(id))
                    || node.executor_session_id == node.reviewer_session_id)
            || node.kind != ProofKind::Review
                && (node.executor_session_id.is_some() || node.reviewer_session_id.is_some())
            || node
                .dependencies
                .iter()
                .any(|id| !self.nodes.contains_key(id))
            || self.nodes.contains_key(&node.proof_id)
        {
            return Err(
                if node
                    .dependencies
                    .iter()
                    .any(|id| !self.nodes.contains_key(id))
                {
                    ProofError::MissingDependency
                } else {
                    ProofError::InvalidNode
                },
            );
        }
        self.nodes.insert(node.proof_id.clone(), node);
        Ok(())
    }

    pub fn snapshot(&self, exact_head_sha: &str) -> Result<ProofSnapshot, ProofError> {
        if !valid_digest(exact_head_sha) {
            return Err(ProofError::InvalidNode);
        }
        let nodes = self.nodes.values().cloned().collect::<Vec<_>>();
        let all_nodes_passed = !nodes.is_empty()
            && nodes.iter().all(|node| {
                node.head_sha == exact_head_sha && node.outcome == ProofOutcome::Passed
            });
        let bytes =
            serde_json::to_vec(&(&nodes, exact_head_sha)).map_err(|_| ProofError::Fingerprint)?;
        Ok(ProofSnapshot {
            head_sha: exact_head_sha.to_owned(),
            graph_digest: blake3::hash(&bytes).to_hex().to_string(),
            nodes,
            all_nodes_passed,
        })
    }

    pub fn certify(
        &self,
        compiled: &CompiledObligations,
    ) -> Result<CertificationReport, ProofError> {
        if !valid_digest(&compiled.exact_head_sha) || !valid_digest(&compiled.digest) {
            return Err(ProofError::InvalidNode);
        }
        let canonical_bytes = serde_json::to_vec(&(
            &compiled.exact_head_sha,
            compiled.risk,
            compiled.broadened_for_unknown_impact,
            &compiled.obligations,
        ))
        .map_err(|_| ProofError::Fingerprint)?;
        if blake3::hash(&canonical_bytes).to_hex().as_str() != compiled.digest {
            return Err(ProofError::Fingerprint);
        }
        if compiled.obligations.is_empty() {
            return Err(ProofError::InvalidNode);
        }
        let mut pairs = Vec::new();
        for (obligation_index, obligation) in compiled.obligations.iter().enumerate() {
            for kind in &obligation.required_kinds {
                pairs.push((obligation_index, *kind));
            }
        }
        if pairs.len() > 512 {
            return Err(ProofError::SearchLimit);
        }
        let candidates = self
            .nodes
            .values()
            .filter(|node| {
                node.head_sha == compiled.exact_head_sha
                    && node.outcome == ProofOutcome::Passed
                    && self.dependencies_passed(node, &compiled.exact_head_sha)
            })
            .map(|node| {
                let coverage = pairs
                    .iter()
                    .enumerate()
                    .filter_map(|(pair_index, (obligation_index, kind))| {
                        let obligation = &compiled.obligations[*obligation_index];
                        (node.kind == *kind
                            && node.obligation_ids.contains(&obligation.obligation_id))
                        .then_some(pair_index)
                    })
                    .collect::<Vec<_>>();
                (node.proof_id.clone(), coverage)
            })
            .filter(|(_, coverage)| !coverage.is_empty())
            .collect::<Vec<_>>();

        let mut gaps = Vec::new();
        for obligation in &compiled.obligations {
            let missing_kinds = obligation
                .required_kinds
                .iter()
                .copied()
                .filter(|kind| {
                    !candidates.iter().any(|(id, _)| {
                        self.nodes.get(id).is_some_and(|node| {
                            node.kind == *kind
                                && node.obligation_ids.contains(&obligation.obligation_id)
                        })
                    })
                })
                .collect::<Vec<_>>();
            if !missing_kinds.is_empty() {
                gaps.push(ProofGap {
                    obligation_id: obligation.obligation_id.clone(),
                    missing_kinds,
                });
            }
        }
        if !gaps.is_empty() {
            return Ok(CertificationReport {
                exact_head_sha: compiled.exact_head_sha.clone(),
                selected_proof_ids: Vec::new(),
                gaps,
                complete: false,
            });
        }

        let mut best = None;
        let mut explored = 0_usize;
        let mut selected = Vec::new();
        let mut covered = vec![false; pairs.len()];
        select_minimal_proofs(
            &pairs,
            &candidates,
            &mut covered,
            &mut selected,
            &mut best,
            &mut explored,
        )?;
        let selected_proof_ids = best.unwrap_or_default();
        Ok(CertificationReport {
            exact_head_sha: compiled.exact_head_sha.clone(),
            complete: true,
            selected_proof_ids,
            gaps: Vec::new(),
        })
    }

    fn dependencies_passed(&self, node: &ProofNode, exact_head_sha: &str) -> bool {
        node.dependencies.iter().all(|dependency| {
            self.nodes.get(dependency).is_some_and(|proof| {
                proof.outcome == ProofOutcome::Passed
                    && proof.head_sha == exact_head_sha
                    && self.dependencies_passed(proof, exact_head_sha)
            })
        })
    }

    pub async fn persist(
        &self,
        store: &ForgeStateStore,
        exact_head_sha: &str,
    ) -> Result<ProofSnapshot, ProofError> {
        let snapshot = self.snapshot(exact_head_sha)?;
        for node in &snapshot.nodes {
            let payload = serde_json::to_value(node).map_err(|_| ProofError::Fingerprint)?;
            store
                .record_evidence(&EvidenceRecord {
                    evidence_id: format!("proof.{}", node.proof_id),
                    kind: format!("proof.{:?}", node.kind).to_ascii_lowercase(),
                    fingerprint: node.fingerprint.clone(),
                    payload,
                })
                .await?;
        }
        Ok(snapshot)
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn select_minimal_proofs(
    pairs: &[(usize, ProofKind)],
    candidates: &[(String, Vec<usize>)],
    covered: &mut [bool],
    selected: &mut Vec<String>,
    best: &mut Option<Vec<String>>,
    explored: &mut usize,
) -> Result<(), ProofError> {
    *explored += 1;
    if *explored > 100_000 {
        return Err(ProofError::SearchLimit);
    }
    if let Some(current_best) = best.as_ref()
        && selected.len() >= current_best.len()
    {
        return Ok(());
    }
    if covered.iter().all(|value| *value) {
        let mut result = selected.clone();
        result.sort();
        *best = Some(result);
        return Ok(());
    }

    let pivot = (0..pairs.len())
        .filter(|index| !covered[*index])
        .min_by_key(|index| {
            candidates
                .iter()
                .filter(|(id, coverage)| !selected.contains(id) && coverage.contains(index))
                .count()
        })
        .ok_or(ProofError::InvalidNode)?;
    let mut choices = candidates
        .iter()
        .filter(|(id, coverage)| !selected.contains(id) && coverage.contains(&pivot))
        .collect::<Vec<_>>();
    choices.sort_by_key(|(_, coverage)| {
        std::cmp::Reverse(coverage.iter().filter(|index| !covered[**index]).count())
    });
    for (id, coverage) in choices {
        selected.push(id.clone());
        let mut newly_covered = Vec::new();
        for index in coverage {
            if !covered[*index] {
                covered[*index] = true;
                newly_covered.push(*index);
            }
        }
        select_minimal_proofs(pairs, candidates, covered, selected, best, explored)?;
        for index in newly_covered {
            covered[index] = false;
        }
        selected.pop();
    }
    Ok(())
}

fn valid_token(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(value: &str) -> String {
        blake3::hash(value.as_bytes()).to_hex().to_string()
    }

    #[test]
    fn proof_snapshot_binds_exact_head_and_requires_all_dependencies_to_pass() {
        let head = digest("head");
        let mut graph = ProofGraph::default();
        graph
            .add(ProofNode {
                proof_id: "build".into(),
                kind: ProofKind::Build,
                outcome: ProofOutcome::Passed,
                fingerprint: digest("build"),
                head_sha: head.clone(),
                dependencies: vec![],
                obligation_ids: vec![],
                executor_session_id: None,
                reviewer_session_id: None,
                metadata: serde_json::json!({}),
            })
            .expect("build proof");
        graph
            .add(ProofNode {
                proof_id: "test".into(),
                kind: ProofKind::Test,
                outcome: ProofOutcome::Passed,
                fingerprint: digest("test"),
                head_sha: head.clone(),
                dependencies: vec!["build".into()],
                obligation_ids: vec![],
                executor_session_id: None,
                reviewer_session_id: None,
                metadata: serde_json::json!({}),
            })
            .expect("test proof");
        assert!(graph.snapshot(&head).expect("snapshot").all_nodes_passed);
        assert!(
            !graph
                .snapshot(&digest("later head"))
                .expect("new head")
                .all_nodes_passed
        );
    }

    #[test]
    fn unknown_dependencies_cannot_be_attached_to_a_proof() {
        let head = digest("head");
        let mut graph = ProofGraph::default();
        assert!(matches!(
            graph.add(ProofNode {
                proof_id: "test".into(),
                kind: ProofKind::Test,
                outcome: ProofOutcome::Passed,
                fingerprint: digest("test"),
                head_sha: head,
                dependencies: vec!["missing".into()],
                obligation_ids: vec![],
                executor_session_id: None,
                reviewer_session_id: None,
                metadata: serde_json::json!({})
            }),
            Err(ProofError::MissingDependency)
        ));
    }

    #[test]
    fn independent_review_proof_requires_distinct_sessions() {
        let head = digest("head");
        let make_review = |executor: &str, reviewer: &str| ProofNode {
            proof_id: "review.independent".into(),
            kind: ProofKind::Review,
            outcome: ProofOutcome::Passed,
            fingerprint: digest("review"),
            head_sha: head.clone(),
            dependencies: vec![],
            obligation_ids: vec!["policy.high_assurance".into()],
            executor_session_id: Some(executor.into()),
            reviewer_session_id: Some(reviewer.into()),
            metadata: serde_json::json!({}),
        };
        let mut graph = ProofGraph::default();
        assert!(matches!(
            graph.add(make_review("implementer", "implementer")),
            Err(ProofError::InvalidNode)
        ));
        assert!(
            graph
                .add(make_review("implementer", "independent-reviewer"))
                .is_ok()
        );
    }

    #[test]
    fn high_risk_and_unknown_impact_compile_stronger_obligations() {
        let compiled = ProofObligationCompiler
            .compile(ChangeAssessment {
                exact_head_sha: digest("head"),
                risk: ProofRisk::High,
                impact_unknown: true,
                obligations: vec![ProofObligation {
                    obligation_id: "invariant.i01".into(),
                    required_kinds: [ProofKind::OfflineSovereign].into_iter().collect(),
                }],
            })
            .expect("compiled obligations");
        assert!(compiled.broadened_for_unknown_impact);
        assert!(
            compiled
                .obligations
                .iter()
                .any(
                    |obligation| obligation.obligation_id == "policy.high_assurance"
                        && obligation.required_kinds.contains(&ProofKind::Review)
                )
        );
        assert!(
            compiled
                .obligations
                .iter()
                .any(
                    |obligation| obligation.obligation_id == "changecone.unknown_impact"
                        && obligation.required_kinds.contains(&ProofKind::Security)
                )
        );
    }

    #[test]
    fn certification_selects_the_smallest_exact_head_proof_set() {
        let head = digest("candidate-head");
        let compiled = ProofObligationCompiler
            .compile(ChangeAssessment {
                exact_head_sha: head.clone(),
                risk: ProofRisk::Low,
                impact_unknown: false,
                obligations: vec![
                    ProofObligation {
                        obligation_id: "contract.api".into(),
                        required_kinds: [ProofKind::Contract, ProofKind::Static]
                            .into_iter()
                            .collect(),
                    },
                    ProofObligation {
                        obligation_id: "contract.cache".into(),
                        required_kinds: [ProofKind::Contract, ProofKind::Security]
                            .into_iter()
                            .collect(),
                    },
                ],
            })
            .expect("compiled obligations");
        let mut graph = ProofGraph::default();
        for (proof_id, kind, obligation_ids) in [
            (
                "contract-both",
                ProofKind::Contract,
                vec!["contract.api", "contract.cache"],
            ),
            ("static-api", ProofKind::Static, vec!["contract.api"]),
            (
                "security-cache",
                ProofKind::Security,
                vec!["contract.cache"],
            ),
            ("contract-api", ProofKind::Contract, vec!["contract.api"]),
            (
                "contract-cache",
                ProofKind::Contract,
                vec!["contract.cache"],
            ),
        ] {
            graph
                .add(ProofNode {
                    proof_id: proof_id.into(),
                    kind,
                    outcome: ProofOutcome::Passed,
                    fingerprint: digest(proof_id),
                    head_sha: head.clone(),
                    dependencies: vec![],
                    obligation_ids: obligation_ids.into_iter().map(str::to_owned).collect(),
                    executor_session_id: None,
                    reviewer_session_id: None,
                    metadata: serde_json::json!({}),
                })
                .expect("proof node");
        }

        let report = graph.certify(&compiled).expect("certification report");
        assert!(report.complete);
        assert_eq!(report.gaps, Vec::<ProofGap>::new());
        assert_eq!(report.selected_proof_ids.len(), 3);
        assert!(
            report
                .selected_proof_ids
                .contains(&"contract-both".to_owned())
        );
    }

    #[test]
    fn certification_reports_missing_or_stale_proofs_as_gaps() {
        let compiled = ProofObligationCompiler
            .compile(ChangeAssessment {
                exact_head_sha: digest("head"),
                risk: ProofRisk::Low,
                impact_unknown: false,
                obligations: vec![ProofObligation {
                    obligation_id: "offline.native".into(),
                    required_kinds: [ProofKind::OfflineSovereign].into_iter().collect(),
                }],
            })
            .expect("compiled obligations");
        let report = ProofGraph::default()
            .certify(&compiled)
            .expect("gaps are evidence, not errors");
        assert!(!report.complete);
        assert_eq!(
            report.gaps[0].missing_kinds,
            vec![ProofKind::OfflineSovereign]
        );
    }

    #[test]
    fn certification_rejects_mutated_compiled_obligations() {
        let mut compiled = ProofObligationCompiler
            .compile(ChangeAssessment {
                exact_head_sha: digest("head"),
                risk: ProofRisk::Low,
                impact_unknown: false,
                obligations: vec![ProofObligation {
                    obligation_id: "contract.api".into(),
                    required_kinds: [ProofKind::Contract].into_iter().collect(),
                }],
            })
            .expect("compiled obligations");
        compiled.obligations.clear();
        assert!(matches!(
            ProofGraph::default().certify(&compiled),
            Err(ProofError::Fingerprint)
        ));
    }
}
