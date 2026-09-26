use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use forge_state::{EvidenceRecord, ForgeStateStore, StateError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::causality::{ChangeKind, GraphSnapshot, NodeKind};

const MAX_DIFF_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHANGED_PATHS: usize = 4096;
const MAX_GIT_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofBackendClaims {
    pub backend_run_id: String,
    pub proof_id: String,
    pub kind: ProofKind,
    pub proof_fingerprint: String,
    pub git_object_format: String,
    pub exact_head_sha: String,
    pub change_set_fingerprint: String,
    pub dependencies: Vec<String>,
    pub obligation_ids: Vec<String>,
    pub inputs_fingerprint: String,
    pub environment_fingerprint: String,
    pub executor_session_id: Option<String>,
    pub reviewer_session_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProofBackendReceipt {
    pub claims: ProofBackendClaims,
    pub mac: String,
}

#[derive(Clone)]
pub struct ProofBackendSigner {
    key: [u8; 32],
}

#[derive(Clone)]
pub struct ProofBackendVerifier {
    key: [u8; 32],
}

impl ProofBackendSigner {
    /// Keep the signing key inside the trusted CI or native review backend.
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn sign(&self, claims: ProofBackendClaims) -> Result<ProofBackendReceipt, ProofError> {
        validate_backend_claims(&claims)?;
        let bytes = serde_json::to_vec(&claims).map_err(|_| ProofError::Fingerprint)?;
        Ok(ProofBackendReceipt {
            claims,
            mac: blake3::keyed_hash(&self.key, &bytes).to_hex().to_string(),
        })
    }
}

impl ProofBackendVerifier {
    /// Configure this verifier only with the host that owns trusted backend receipts.
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    fn verify(&self, receipt: &ProofBackendReceipt, node: &ProofNode) -> Result<(), ProofError> {
        validate_backend_claims(&receipt.claims)?;
        let claims_bytes =
            serde_json::to_vec(&receipt.claims).map_err(|_| ProofError::Fingerprint)?;
        let expected = blake3::keyed_hash(&self.key, &claims_bytes)
            .to_hex()
            .to_string();
        if !constant_time_hex_eq(&receipt.mac, &expected)
            || node.outcome != ProofOutcome::Passed
            || receipt.claims.proof_id != node.proof_id
            || receipt.claims.kind != node.kind
            || receipt.claims.proof_fingerprint != node.fingerprint
            || !valid_git_object_id_for_format(&node.head_sha, &receipt.claims.git_object_format)
            || receipt.claims.exact_head_sha != node.head_sha
            || receipt.claims.dependencies != node.dependencies
            || receipt.claims.obligation_ids != node.obligation_ids
            || receipt.claims.executor_session_id != node.executor_session_id
            || receipt.claims.reviewer_session_id != node.reviewer_session_id
            || node.metadata.get("backendRunId").and_then(Value::as_str)
                != Some(receipt.claims.backend_run_id.as_str())
            || node
                .metadata
                .get("changeSetFingerprint")
                .and_then(Value::as_str)
                != Some(receipt.claims.change_set_fingerprint.as_str())
            || node
                .metadata
                .get("inputsFingerprint")
                .and_then(Value::as_str)
                != Some(receipt.claims.inputs_fingerprint.as_str())
            || node
                .metadata
                .get("environmentFingerprint")
                .and_then(Value::as_str)
                != Some(receipt.claims.environment_fingerprint.as_str())
        {
            return Err(ProofError::BackendAttestation);
        }
        Ok(())
    }
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

#[derive(Clone, Debug)]
pub struct ChangeAssessment {
    base_commit_sha: String,
    git_object_format: String,
    exact_head_sha: String,
    changed_paths: Vec<String>,
    source_fingerprint: Option<String>,
    graph: Option<GraphSnapshot>,
}

impl ChangeAssessment {
    /// Resolve the admitted base and current exact HEAD from Git, then derive the changed path set
    /// from that exact pair. Callers cannot construct a partial changed-path list directly.
    pub fn from_git(
        repository: impl AsRef<Path>,
        base_revision: &str,
        exact_head_revision: &str,
        source_fingerprint: Option<String>,
        graph: Option<GraphSnapshot>,
    ) -> Result<Self, ProofError> {
        let repository = repository
            .as_ref()
            .canonicalize()
            .map_err(|_| ProofError::Git)?;
        if !repository.is_dir()
            || repository.to_str().is_none()
            || !valid_git_revision(base_revision)
            || !valid_git_revision(exact_head_revision)
            || source_fingerprint
                .as_deref()
                .is_some_and(|value| !valid_digest(value))
        {
            return Err(ProofError::InvalidNode);
        }

        let format = run_git_bounded(&repository, &["rev-parse", "--show-object-format"], 32)?;
        let git_object_format = std::str::from_utf8(&format)
            .map_err(|_| ProofError::Git)?
            .trim()
            .to_owned();
        if !matches!(git_object_format.as_str(), "sha1" | "sha256") {
            return Err(ProofError::Git);
        }

        let base_commit_sha = resolve_git_commit(&repository, base_revision, &git_object_format)?;
        let exact_head_sha =
            resolve_git_commit(&repository, exact_head_revision, &git_object_format)?;
        let current_head = resolve_git_commit(&repository, "HEAD", &git_object_format)?;
        if exact_head_sha != current_head {
            return Err(ProofError::InvalidNode);
        }
        run_git_bounded(
            &repository,
            &[
                "merge-base",
                "--is-ancestor",
                &base_commit_sha,
                &exact_head_sha,
            ],
            32,
        )?;
        let diff = run_git_bounded(
            &repository,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                "--name-only",
                "-z",
                &base_commit_sha,
                &exact_head_sha,
            ],
            MAX_DIFF_OUTPUT_BYTES,
        )?;
        let changed_paths = parse_git_paths(&diff)?;
        Ok(Self {
            base_commit_sha,
            git_object_format,
            exact_head_sha,
            changed_paths,
            source_fingerprint,
            graph,
        })
    }

    pub fn exact_head_sha(&self) -> &str {
        &self.exact_head_sha
    }

    pub fn git_object_format(&self) -> &str {
        &self.git_object_format
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledObligations {
    pub base_commit_sha: String,
    pub git_object_format: String,
    pub exact_head_sha: String,
    pub digest: String,
    pub risk: ProofRisk,
    pub obligations: Vec<ProofObligation>,
    pub broadened_for_unknown_impact: bool,
    pub change_set_fingerprint: String,
    pub source_fingerprint: Option<String>,
    pub graph_digest: Option<String>,
    pub impacted_nodes: Vec<String>,
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
    #[error("proof pass lacks a valid trusted backend receipt bound to its exact inputs")]
    BackendAttestation,
    #[error("proof evidence persistence failed")]
    State(#[from] StateError),
    #[error("proof selection exceeded its bounded exact-search budget")]
    SearchLimit,
    #[error("exact Git change assessment could not be verified")]
    Git,
    #[error("Git change assessment exceeded its time, output, or path budget")]
    ResourceLimit,
}

#[derive(Default)]
pub struct ProofObligationCompiler;

impl ProofObligationCompiler {
    pub fn compile(&self, assessment: ChangeAssessment) -> Result<CompiledObligations, ProofError> {
        if !valid_git_object_id_for_format(
            &assessment.base_commit_sha,
            &assessment.git_object_format,
        ) || !valid_git_object_id_for_format(
            &assessment.exact_head_sha,
            &assessment.git_object_format,
        ) || assessment.changed_paths.len() > MAX_CHANGED_PATHS
            || assessment
                .source_fingerprint
                .as_deref()
                .is_some_and(|value| !valid_digest(value))
        {
            return Err(ProofError::InvalidNode);
        }
        let mut changed_paths = assessment.changed_paths;
        changed_paths.sort();
        if changed_paths.iter().any(|path| !valid_change_path(path))
            || changed_paths.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(ProofError::InvalidNode);
        }

        let mut impacted_nodes = BTreeSet::new();
        let mut graph_digest = None;
        let mut impact_unknown = changed_paths.is_empty()
            || assessment.source_fingerprint.is_none()
            || assessment
                .graph
                .as_ref()
                .is_none_or(|graph| !graph.proof_coverage_verified());
        if let Some(graph) = assessment.graph.as_ref() {
            graph_digest = Some(graph.digest.clone());
            if changed_paths.is_empty() {
                impact_unknown = true;
            } else {
                let kinds = changed_paths
                    .iter()
                    .map(|path| classify_change_kind(path))
                    .collect::<BTreeSet<_>>();
                for kind in kinds {
                    let cone = graph.compute_change_cone(&changed_paths, kind);
                    impact_unknown |= cone.conservative;
                    impacted_nodes.extend(cone.impacted);
                    impacted_nodes.extend(cone.proof_nodes_to_invalidate);
                }
            }
        } else {
            impact_unknown = true;
        }

        let risk = derive_risk(&changed_paths, impact_unknown);
        let mut obligations = BTreeMap::<String, BTreeSet<ProofKind>>::new();
        if changed_paths.is_empty() {
            add_obligation(
                &mut obligations,
                "change.source_integrity",
                [ProofKind::Static, ProofKind::Build],
            );
        }
        for path in &changed_paths {
            add_obligation(
                &mut obligations,
                &format!("change.path.{}", short_digest(path)),
                kinds_for_path(path),
            );
        }
        if let Some(graph) = assessment.graph.as_ref() {
            for node_id in &impacted_nodes {
                let required_kinds = kinds_for_node(graph.node_kind(node_id));
                add_obligation(
                    &mut obligations,
                    &format!("change.node.{}", short_digest(node_id)),
                    required_kinds,
                );
            }
        }
        if risk == ProofRisk::High {
            add_obligation(
                &mut obligations,
                "policy.high_assurance",
                [
                    ProofKind::Contract,
                    ProofKind::FaultRecovery,
                    ProofKind::Security,
                    ProofKind::Performance,
                    ProofKind::ReplayDeterminism,
                    ProofKind::Review,
                ],
            );
        }
        if impact_unknown {
            add_obligation(
                &mut obligations,
                "changecone.unknown_impact",
                all_proof_kinds(),
            );
        }
        let obligations = obligations
            .into_iter()
            .map(|(obligation_id, required_kinds)| ProofObligation {
                obligation_id,
                required_kinds,
            })
            .collect::<Vec<_>>();
        if obligations.is_empty() {
            return Err(ProofError::InvalidNode);
        }
        let impacted_nodes = impacted_nodes.into_iter().collect::<Vec<_>>();
        let change_set_fingerprint = blake3::hash(
            &serde_json::to_vec(&(
                &assessment.base_commit_sha,
                &assessment.git_object_format,
                &assessment.exact_head_sha,
                &changed_paths,
                &assessment.source_fingerprint,
                &graph_digest,
            ))
            .map_err(|_| ProofError::Fingerprint)?,
        )
        .to_hex()
        .to_string();
        let canonical = (
            &assessment.base_commit_sha,
            &assessment.git_object_format,
            &assessment.exact_head_sha,
            &change_set_fingerprint,
            risk,
            impact_unknown,
            &obligations,
            &assessment.source_fingerprint,
            &graph_digest,
            &impacted_nodes,
        );
        let bytes = serde_json::to_vec(&canonical).map_err(|_| ProofError::Fingerprint)?;
        Ok(CompiledObligations {
            base_commit_sha: assessment.base_commit_sha,
            git_object_format: assessment.git_object_format,
            exact_head_sha: assessment.exact_head_sha,
            digest: blake3::hash(&bytes).to_hex().to_string(),
            risk,
            obligations,
            broadened_for_unknown_impact: impact_unknown,
            change_set_fingerprint,
            source_fingerprint: assessment.source_fingerprint,
            graph_digest,
            impacted_nodes,
        })
    }
}

fn valid_git_revision(revision: &str) -> bool {
    !revision.is_empty()
        && revision.len() <= 256
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte))
}

fn resolve_git_commit(
    repository: &Path,
    revision: &str,
    object_format: &str,
) -> Result<String, ProofError> {
    let expression = format!("{revision}^{{commit}}");
    let output = run_git_bounded(
        repository,
        &["rev-parse", "--verify", "--end-of-options", &expression],
        128,
    )?;
    let object_id = std::str::from_utf8(&output)
        .map_err(|_| ProofError::Git)?
        .trim()
        .to_ascii_lowercase();
    if !valid_git_object_id_for_format(&object_id, object_format) {
        return Err(ProofError::Git);
    }
    Ok(object_id)
}

fn parse_git_paths(output: &[u8]) -> Result<Vec<String>, ProofError> {
    if output.len() > MAX_DIFF_OUTPUT_BYTES || output.last().is_some_and(|byte| *byte != 0) {
        return Err(ProofError::ResourceLimit);
    }
    let mut paths = Vec::new();
    for raw_path in output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        if paths.len() >= MAX_CHANGED_PATHS {
            return Err(ProofError::ResourceLimit);
        }
        let path = std::str::from_utf8(raw_path).map_err(|_| ProofError::Git)?;
        if !valid_change_path(path) {
            return Err(ProofError::Git);
        }
        paths.push(path.to_owned());
    }
    if paths.iter().collect::<BTreeSet<_>>().len() != paths.len() {
        return Err(ProofError::Git);
    }
    Ok(paths)
}

fn run_git_bounded(
    repository: &Path,
    arguments: &[&str],
    maximum_output: usize,
) -> Result<Vec<u8>, ProofError> {
    if maximum_output == 0 || maximum_output > MAX_GIT_OUTPUT_BYTES {
        return Err(ProofError::ResourceLimit);
    }
    let repository_text = repository.to_str().ok_or(ProofError::Git)?;
    let mut command = Command::new("git");
    command
        .arg("-c")
        .arg(format!("safe.directory={repository_text}"))
        .arg("--no-replace-objects")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_OPTIONAL_LOCKS", "0");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CEILING_DIRECTORIES",
        "GIT_PREFIX",
    ] {
        command.env_remove(key);
    }
    let mut child = command.spawn().map_err(|_| ProofError::Git)?;
    let stdout = child.stdout.take().ok_or(ProofError::Git)?;
    let (sender, receiver) = mpsc::channel::<Result<Vec<u8>, bool>>();
    let reader = thread::spawn(move || {
        let mut stdout = stdout;
        let mut output = Vec::new();
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) => {
                    let _ = sender.send(Ok(output));
                    return;
                }
                Ok(count) => {
                    if output.len().saturating_add(count) > maximum_output {
                        let _ = sender.send(Err(true));
                        return;
                    }
                    output.extend_from_slice(&chunk[..count]);
                }
                Err(_) => {
                    let _ = sender.send(Err(false));
                    return;
                }
            }
        }
    });

    let started = Instant::now();
    let mut captured = None;
    let status = loop {
        if captured.is_none() {
            match receiver.try_recv() {
                Ok(Ok(output)) => captured = Some(output),
                Ok(Err(too_large)) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(if too_large {
                        ProofError::ResourceLimit
                    } else {
                        ProofError::Git
                    });
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(ProofError::Git);
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        match child.try_wait().map_err(|_| ProofError::Git)? {
            Some(status) => break status,
            None if started.elapsed() >= GIT_COMMAND_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(ProofError::ResourceLimit);
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    };
    let output = captured
        .or_else(|| receiver.recv_timeout(Duration::from_secs(1)).ok()?.ok())
        .ok_or(ProofError::Git)?;
    reader.join().map_err(|_| ProofError::Git)?;
    if !status.success() {
        return Err(ProofError::Git);
    }
    Ok(output)
}

fn add_obligation(
    obligations: &mut BTreeMap<String, BTreeSet<ProofKind>>,
    id: &str,
    kinds: impl IntoIterator<Item = ProofKind>,
) {
    obligations.entry(id.to_owned()).or_default().extend(kinds);
}

fn all_proof_kinds() -> [ProofKind; 18] {
    [
        ProofKind::Static,
        ProofKind::Contract,
        ProofKind::Unit,
        ProofKind::Property,
        ProofKind::Integration,
        ProofKind::StateMigration,
        ProofKind::Concurrency,
        ProofKind::FaultRecovery,
        ProofKind::Compatibility,
        ProofKind::ReplayDeterminism,
        ProofKind::OfflineSovereign,
        ProofKind::EndToEnd,
        ProofKind::ManualPolicy,
        ProofKind::Test,
        ProofKind::Security,
        ProofKind::Performance,
        ProofKind::Build,
        ProofKind::Review,
    ]
}

fn valid_git_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_git_object_id_for_format(value: &str, format: &str) -> bool {
    let expected_length = match format {
        "sha1" => 40,
        "sha256" => 64,
        _ => return false,
    };
    value.len() == expected_length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_change_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 4096
        && !path.starts_with('/')
        && !path.contains(['\\', ':'])
        && !path
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == 0x7f)
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn short_digest(value: &str) -> String {
    blake3::hash(value.as_bytes()).to_hex()[..16].to_owned()
}

fn classify_change_kind(path: &str) -> ChangeKind {
    let normalized = path.to_ascii_lowercase();
    if normalized.contains("security")
        || normalized.contains("permission")
        || normalized.contains("auth")
        || normalized.contains("proof")
    {
        ChangeKind::Security
    } else if normalized.contains("migration") || normalized.contains("state") {
        ChangeKind::State
    } else if normalized.contains("config") || normalized.ends_with(".toml") {
        ChangeKind::Configuration
    } else if normalized.contains(".github/workflows")
        || normalized.ends_with("cargo.lock")
        || normalized.contains("toolchain")
    {
        ChangeKind::Toolchain
    } else if normalized.starts_with("docs/") || normalized.ends_with(".md") {
        ChangeKind::NonSemantic
    } else {
        ChangeKind::Implementation
    }
}

fn derive_risk(paths: &[String], impact_unknown: bool) -> ProofRisk {
    if impact_unknown
        || paths.iter().any(|path| {
            let path = path.to_ascii_lowercase();
            path.contains("security")
                || path.contains("permission")
                || path.contains("auth")
                || path.contains("proof")
                || path.contains("migration")
                || path.contains("forge-state")
                || path.contains("crates/forge-kernel")
                || path.contains(".github/workflows")
                || path.contains("source-fingerprints")
        })
    {
        ProofRisk::High
    } else if paths.iter().any(|path| {
        !path.to_ascii_lowercase().starts_with("docs/")
            && !path.to_ascii_lowercase().ends_with(".md")
    }) {
        ProofRisk::Elevated
    } else {
        ProofRisk::Low
    }
}

fn kinds_for_path(path: &str) -> BTreeSet<ProofKind> {
    let path = path.to_ascii_lowercase();
    let mut kinds = BTreeSet::from([ProofKind::Static, ProofKind::Contract]);
    if !path.starts_with("docs/") && !path.ends_with(".md") {
        kinds.extend([ProofKind::Unit, ProofKind::Test]);
    }
    if path.contains("state") || path.contains("migration") {
        kinds.extend([
            ProofKind::StateMigration,
            ProofKind::Concurrency,
            ProofKind::FaultRecovery,
        ]);
    }
    if path.contains("security")
        || path.contains("permission")
        || path.contains("auth")
        || path.contains("proof")
    {
        kinds.extend([ProofKind::Security, ProofKind::Review]);
    }
    if path.contains(".github/workflows")
        || path.ends_with("cargo.toml")
        || path.ends_with("cargo.lock")
        || path.contains("toolchain")
    {
        kinds.extend([ProofKind::Build, ProofKind::OfflineSovereign]);
    }
    if path.contains("bench") || path.contains("performance") {
        kinds.insert(ProofKind::Performance);
    }
    if path.contains("hive") || path.contains("integration") {
        kinds.extend([ProofKind::Integration, ProofKind::Compatibility]);
    }
    kinds
}

fn kinds_for_node(kind: Option<NodeKind>) -> BTreeSet<ProofKind> {
    match kind {
        Some(NodeKind::Contract) => BTreeSet::from([ProofKind::Contract, ProofKind::Unit]),
        Some(NodeKind::Capability | NodeKind::Command | NodeKind::Event) => {
            BTreeSet::from([ProofKind::Contract, ProofKind::Unit, ProofKind::Integration])
        }
        Some(NodeKind::StateSchema) => BTreeSet::from([
            ProofKind::StateMigration,
            ProofKind::Concurrency,
            ProofKind::FaultRecovery,
        ]),
        Some(NodeKind::Permission) => {
            BTreeSet::from([ProofKind::Security, ProofKind::Contract, ProofKind::Review])
        }
        Some(NodeKind::Test) => BTreeSet::from([ProofKind::Unit, ProofKind::Property]),
        Some(NodeKind::BuildArtifact) => BTreeSet::from([ProofKind::Build]),
        Some(NodeKind::Evidence) => BTreeSet::from([ProofKind::Static, ProofKind::Review]),
        Some(NodeKind::Environment) => {
            BTreeSet::from([ProofKind::Compatibility, ProofKind::OfflineSovereign])
        }
        Some(NodeKind::Configuration) => {
            BTreeSet::from([ProofKind::Contract, ProofKind::Compatibility])
        }
        Some(NodeKind::Module) | None => {
            BTreeSet::from([ProofKind::Static, ProofKind::Unit, ProofKind::Integration])
        }
    }
}

#[derive(Default)]
pub struct ProofGraph {
    nodes: BTreeMap<String, ProofNode>,
}

impl ProofGraph {
    pub fn add(&mut self, node: ProofNode) -> Result<(), ProofError> {
        if node.outcome == ProofOutcome::Passed {
            return Err(ProofError::BackendAttestation);
        }
        self.insert(node)
    }

    pub fn add_verified(
        &mut self,
        mut node: ProofNode,
        receipt: ProofBackendReceipt,
        verifier: &ProofBackendVerifier,
    ) -> Result<(), ProofError> {
        verifier.verify(&receipt, &node)?;
        let receipt_value = serde_json::to_value(&receipt).map_err(|_| ProofError::Fingerprint)?;
        let metadata = node
            .metadata
            .as_object_mut()
            .ok_or(ProofError::BackendAttestation)?;
        metadata.insert("proofBackendReceipt".into(), receipt_value);
        self.insert(node)
    }

    fn insert(&mut self, node: ProofNode) -> Result<(), ProofError> {
        if !valid_token(&node.proof_id)
            || !valid_digest(&node.fingerprint)
            || !valid_git_object_id(&node.head_sha)
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

    pub fn invalidate_stale_proofs(&mut self, compiled: &CompiledObligations) -> usize {
        let mut invalidated = 0;
        for node in self.nodes.values_mut() {
            if node.outcome == ProofOutcome::Passed
                && (node.head_sha != compiled.exact_head_sha
                    || node
                        .metadata
                        .get("changeSetFingerprint")
                        .and_then(Value::as_str)
                        != Some(compiled.change_set_fingerprint.as_str()))
            {
                node.outcome = ProofOutcome::Invalidated;
                invalidated += 1;
            }
        }
        invalidated
    }

    pub fn snapshot(&self, exact_head_sha: &str) -> Result<ProofSnapshot, ProofError> {
        if !valid_git_object_id(exact_head_sha) {
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
        if !valid_git_object_id_for_format(&compiled.exact_head_sha, &compiled.git_object_format)
            || !valid_git_object_id_for_format(
                &compiled.base_commit_sha,
                &compiled.git_object_format,
            )
            || !valid_digest(&compiled.digest)
            || !valid_digest(&compiled.change_set_fingerprint)
            || compiled
                .source_fingerprint
                .as_deref()
                .is_some_and(|fingerprint| !valid_digest(fingerprint))
            || compiled
                .graph_digest
                .as_deref()
                .is_some_and(|fingerprint| !valid_digest(fingerprint))
        {
            return Err(ProofError::InvalidNode);
        }
        let canonical_bytes = serde_json::to_vec(&(
            &compiled.base_commit_sha,
            &compiled.git_object_format,
            &compiled.exact_head_sha,
            &compiled.change_set_fingerprint,
            compiled.risk,
            compiled.broadened_for_unknown_impact,
            &compiled.obligations,
            &compiled.source_fingerprint,
            &compiled.graph_digest,
            &compiled.impacted_nodes,
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
                    && node
                        .metadata
                        .get("changeSetFingerprint")
                        .and_then(Value::as_str)
                        == Some(compiled.change_set_fingerprint.as_str())
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

fn validate_backend_claims(claims: &ProofBackendClaims) -> Result<(), ProofError> {
    if !valid_token(&claims.backend_run_id)
        || !valid_token(&claims.proof_id)
        || !valid_digest(&claims.proof_fingerprint)
        || !valid_git_object_id_for_format(&claims.exact_head_sha, &claims.git_object_format)
        || !valid_digest(&claims.change_set_fingerprint)
        || !valid_digest(&claims.inputs_fingerprint)
        || !valid_digest(&claims.environment_fingerprint)
        || claims.dependencies.len() > 128
        || claims.dependencies.iter().any(|value| !valid_token(value))
        || claims.dependencies.iter().collect::<BTreeSet<_>>().len() != claims.dependencies.len()
        || claims.obligation_ids.len() > 256
        || claims
            .obligation_ids
            .iter()
            .any(|value| !valid_token(value))
        || claims.obligation_ids.iter().collect::<BTreeSet<_>>().len()
            != claims.obligation_ids.len()
        || claims.kind == ProofKind::Review
            && (claims
                .executor_session_id
                .as_deref()
                .is_none_or(|id| !valid_token(id))
                || claims
                    .reviewer_session_id
                    .as_deref()
                    .is_none_or(|id| !valid_token(id))
                || claims.executor_session_id == claims.reviewer_session_id)
        || claims.kind != ProofKind::Review
            && (claims.executor_session_id.is_some() || claims.reviewer_session_id.is_some())
    {
        return Err(ProofError::BackendAttestation);
    }
    Ok(())
}

fn constant_time_hex_eq(left: &str, right: &str) -> bool {
    if left.len() != 64 || right.len() != 64 {
        return false;
    }
    left.bytes()
        .zip(right.bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | left.to_ascii_lowercase() ^ right.to_ascii_lowercase()
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::causality::{CausalityGraph, GraphNode};

    fn digest(value: &str) -> String {
        blake3::hash(value.as_bytes()).to_hex().to_string()
    }

    fn assessment(
        head: &str,
        paths: &[&str],
        graph_present: bool,
        source_present: bool,
    ) -> ChangeAssessment {
        let git_object_format = if head.len() == 40 { "sha1" } else { "sha256" };
        let base_commit_sha = if git_object_format == "sha1" {
            "a".repeat(40)
        } else {
            "a".repeat(64)
        };
        let mut graph = CausalityGraph::new();
        if graph_present {
            for path in paths {
                graph
                    .add_node(GraphNode {
                        id: (*path).into(),
                        kind: NodeKind::Module,
                    })
                    .expect("graph seed");
            }
            graph.mark_proof_coverage_verified();
        }
        ChangeAssessment {
            base_commit_sha,
            git_object_format: git_object_format.into(),
            exact_head_sha: head.to_owned(),
            changed_paths: paths.iter().map(|path| (*path).to_owned()).collect(),
            source_fingerprint: source_present.then(|| digest("source")),
            graph: graph_present.then(|| graph.snapshot().expect("graph snapshot")),
        }
    }

    fn fixture_git(repository: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("Git fixture command starts");
        assert!(
            output.status.success(),
            "Git fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git fixture output is UTF-8")
            .trim()
            .to_owned()
    }

    fn create_git_fixture(object_format: &str) -> (tempfile::TempDir, String, String) {
        let temporary = tempfile::tempdir().expect("Git fixture directory");
        let repository = temporary.path();
        fixture_git(
            repository,
            &[
                "init",
                "--quiet",
                "--initial-branch=main",
                "--object-format",
                object_format,
            ],
        );
        fixture_git(repository, &["config", "user.name", "Proof Fixture"]);
        fixture_git(
            repository,
            &["config", "user.email", "proof@example.invalid"],
        );
        let source = repository.join("crates/forge-kernel/src/fixture.rs");
        std::fs::create_dir_all(source.parent().expect("fixture parent"))
            .expect("fixture source directory");
        std::fs::write(&source, b"pub const VALUE: u8 = 1;\n").expect("base fixture source");
        fixture_git(repository, &["add", "--all"]);
        fixture_git(repository, &["commit", "--quiet", "-m", "base fixture"]);
        let base = fixture_git(repository, &["rev-parse", "HEAD"]);
        std::fs::write(&source, b"pub const VALUE: u8 = 2;\n").expect("changed fixture source");
        fixture_git(repository, &["add", "--all"]);
        fixture_git(repository, &["commit", "--quiet", "-m", "head fixture"]);
        let head = fixture_git(repository, &["rev-parse", "HEAD"]);
        (temporary, base, head)
    }

    #[test]
    fn git_assessment_derives_exact_paths_and_checks_sha1_and_sha256_references() {
        for object_format in ["sha1", "sha256"] {
            let (temporary, base, head) = create_git_fixture(object_format);
            let assessment = ChangeAssessment::from_git(
                temporary.path(),
                &base,
                &head,
                Some(digest("verified-source-set")),
                None,
            )
            .expect("exact Git assessment");
            assert_eq!(assessment.git_object_format(), object_format);
            assert_eq!(assessment.exact_head_sha(), head);
            assert_eq!(
                assessment.changed_paths,
                vec!["crates/forge-kernel/src/fixture.rs"]
            );
            let compiled = ProofObligationCompiler
                .compile(assessment)
                .expect("compiled exact Git obligations");
            assert_eq!(compiled.git_object_format, object_format);
            assert!(compiled.obligations.iter().any(|obligation| {
                obligation.obligation_id
                    == format!(
                        "change.path.{}",
                        short_digest("crates/forge-kernel/src/fixture.rs")
                    )
            }));

            assert!(matches!(
                ChangeAssessment::from_git(
                    temporary.path(),
                    &base,
                    &base,
                    Some(digest("verified-source-set")),
                    None,
                ),
                Err(ProofError::InvalidNode)
            ));
            let fabricated = if object_format == "sha1" {
                "f".repeat(40)
            } else {
                "f".repeat(64)
            };
            assert!(
                ChangeAssessment::from_git(
                    temporary.path(),
                    &fabricated,
                    &head,
                    Some(digest("verified-source-set")),
                    None,
                )
                .is_err()
            );
            assert!(!valid_git_object_id_for_format(
                &"a".repeat(if object_format == "sha1" { 64 } else { 40 }),
                object_format
            ));
        }
    }

    #[test]
    fn omitted_git_change_range_broadens_to_the_full_proof_universe() {
        let (temporary, _base, head) = create_git_fixture("sha1");
        let assessment = ChangeAssessment::from_git(
            temporary.path(),
            &head,
            &head,
            Some(digest("verified-source-set")),
            None,
        )
        .expect("empty exact range");
        assert!(assessment.changed_paths.is_empty());
        let compiled = ProofObligationCompiler
            .compile(assessment)
            .expect("conservative empty-range obligations");
        assert!(compiled.broadened_for_unknown_impact);
        assert!(compiled.obligations.iter().any(|obligation| {
            obligation.obligation_id == "changecone.unknown_impact"
                && obligation.required_kinds.len() == all_proof_kinds().len()
        }));
    }

    struct ProofFixtureBindings {
        dependencies: Vec<String>,
        obligation_ids: Vec<String>,
        executor_session_id: Option<String>,
        reviewer_session_id: Option<String>,
    }

    impl ProofFixtureBindings {
        fn new(
            dependencies: Vec<String>,
            obligation_ids: Vec<String>,
            executor_session_id: Option<&str>,
            reviewer_session_id: Option<&str>,
        ) -> Self {
            Self {
                dependencies,
                obligation_ids,
                executor_session_id: executor_session_id.map(str::to_owned),
                reviewer_session_id: reviewer_session_id.map(str::to_owned),
            }
        }
    }

    fn add_passed(
        graph: &mut ProofGraph,
        proof_id: &str,
        kind: ProofKind,
        head: &str,
        bindings: ProofFixtureBindings,
    ) {
        add_attested(
            graph,
            proof_id,
            kind,
            head,
            &digest("fixture-change-set"),
            bindings,
        );
    }

    fn add_compiled_passed(
        graph: &mut ProofGraph,
        compiled: &CompiledObligations,
        proof_id: &str,
        kind: ProofKind,
        bindings: ProofFixtureBindings,
    ) {
        add_attested(
            graph,
            proof_id,
            kind,
            &compiled.exact_head_sha,
            &compiled.change_set_fingerprint,
            bindings,
        );
    }

    fn add_attested(
        graph: &mut ProofGraph,
        proof_id: &str,
        kind: ProofKind,
        head: &str,
        change_set_fingerprint: &str,
        bindings: ProofFixtureBindings,
    ) {
        let key = [0x6a; 32];
        let signer = ProofBackendSigner::new(key);
        let verifier = ProofBackendVerifier::new(key);
        let inputs_fingerprint = digest(&format!("inputs:{proof_id}"));
        let environment_fingerprint = digest(&format!("environment:{proof_id}"));
        let backend_run_id = format!("backend.{proof_id}");
        let node = ProofNode {
            proof_id: proof_id.into(),
            kind,
            outcome: ProofOutcome::Passed,
            fingerprint: digest(&format!("artifact:{proof_id}")),
            head_sha: head.into(),
            dependencies: bindings.dependencies,
            obligation_ids: bindings.obligation_ids,
            executor_session_id: bindings.executor_session_id,
            reviewer_session_id: bindings.reviewer_session_id,
            metadata: serde_json::json!({
                "backendRunId": backend_run_id,
                "changeSetFingerprint": change_set_fingerprint,
                "inputsFingerprint": inputs_fingerprint,
                "environmentFingerprint": environment_fingerprint,
            }),
        };
        let receipt = signer
            .sign(ProofBackendClaims {
                backend_run_id,
                proof_id: node.proof_id.clone(),
                kind: node.kind,
                proof_fingerprint: node.fingerprint.clone(),
                git_object_format: if node.head_sha.len() == 40 {
                    "sha1".into()
                } else {
                    "sha256".into()
                },
                exact_head_sha: node.head_sha.clone(),
                change_set_fingerprint: change_set_fingerprint.into(),
                dependencies: node.dependencies.clone(),
                obligation_ids: node.obligation_ids.clone(),
                inputs_fingerprint,
                environment_fingerprint,
                executor_session_id: node.executor_session_id.clone(),
                reviewer_session_id: node.reviewer_session_id.clone(),
            })
            .expect("trusted backend receipt");
        graph
            .add_verified(node, receipt, &verifier)
            .expect("verified proof node");
    }

    fn obligations_for(compiled: &CompiledObligations, kind: ProofKind) -> Vec<String> {
        compiled
            .obligations
            .iter()
            .filter(|obligation| obligation.required_kinds.contains(&kind))
            .map(|obligation| obligation.obligation_id.clone())
            .collect()
    }

    #[test]
    fn proof_snapshot_binds_exact_head_and_requires_all_dependencies_to_pass() {
        let head = "a".repeat(40);
        let mut graph = ProofGraph::default();
        add_passed(
            &mut graph,
            "build",
            ProofKind::Build,
            &head,
            ProofFixtureBindings::new(vec![], vec![], None, None),
        );
        add_passed(
            &mut graph,
            "test",
            ProofKind::Test,
            &head,
            ProofFixtureBindings::new(vec!["build".into()], vec![], None, None),
        );
        assert!(graph.snapshot(&head).expect("snapshot").all_nodes_passed);
        assert!(
            !graph
                .snapshot(&"b".repeat(40))
                .expect("new head")
                .all_nodes_passed
        );
        assert!(matches!(
            graph.add(ProofNode {
                proof_id: "caller-forged".into(),
                kind: ProofKind::Test,
                outcome: ProofOutcome::Passed,
                fingerprint: digest("caller-forged"),
                head_sha: head,
                dependencies: vec![],
                obligation_ids: vec![],
                executor_session_id: None,
                reviewer_session_id: None,
                metadata: serde_json::json!({}),
            }),
            Err(ProofError::BackendAttestation)
        ));
    }

    #[test]
    fn unknown_dependencies_cannot_be_attached_to_a_proof() {
        let head = digest("head");
        let mut graph = ProofGraph::default();
        assert!(matches!(
            graph.add(ProofNode {
                proof_id: "test".into(),
                kind: ProofKind::Test,
                outcome: ProofOutcome::Failed,
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
        let same_session = ProofBackendClaims {
            backend_run_id: "backend.review.same".into(),
            proof_id: "review.same".into(),
            kind: ProofKind::Review,
            proof_fingerprint: digest("review.same"),
            git_object_format: "sha256".into(),
            exact_head_sha: head.clone(),
            change_set_fingerprint: digest("fixture-change-set"),
            dependencies: vec![],
            obligation_ids: vec!["policy.high_assurance".into()],
            inputs_fingerprint: digest("review.inputs"),
            environment_fingerprint: digest("review.environment"),
            executor_session_id: Some("same-session".into()),
            reviewer_session_id: Some("same-session".into()),
        };
        let signer = ProofBackendSigner::new([0x6a; 32]);
        let mut graph = ProofGraph::default();
        assert!(matches!(
            signer.sign(same_session),
            Err(ProofError::BackendAttestation)
        ));
        add_passed(
            &mut graph,
            "review.independent",
            ProofKind::Review,
            &head,
            ProofFixtureBindings::new(
                vec![],
                vec!["policy.high_assurance".into()],
                Some("codex.executor.session"),
                Some("codex.reviewer.session"),
            ),
        );
        assert!(
            graph
                .snapshot(&head)
                .expect("review snapshot")
                .all_nodes_passed
        );
    }

    #[test]
    fn backend_receipt_rejects_stale_or_mutated_proof_bindings() {
        let key = [0x31; 32];
        let signer = ProofBackendSigner::new(key);
        let verifier = ProofBackendVerifier::new(key);
        let node = ProofNode {
            proof_id: "proof.exact".into(),
            kind: ProofKind::Integration,
            outcome: ProofOutcome::Passed,
            fingerprint: digest("proof artifact"),
            head_sha: "c".repeat(40),
            dependencies: vec![],
            obligation_ids: vec!["change.path.001".into()],
            executor_session_id: None,
            reviewer_session_id: None,
            metadata: serde_json::json!({
                "backendRunId": "ci.run.001",
                "changeSetFingerprint": digest("change set"),
                "inputsFingerprint": digest("inputs"),
                "environmentFingerprint": digest("environment"),
            }),
        };
        let receipt = signer
            .sign(ProofBackendClaims {
                backend_run_id: "ci.run.001".into(),
                proof_id: node.proof_id.clone(),
                kind: node.kind,
                proof_fingerprint: node.fingerprint.clone(),
                git_object_format: "sha1".into(),
                exact_head_sha: node.head_sha.clone(),
                change_set_fingerprint: digest("change set"),
                dependencies: vec![],
                obligation_ids: node.obligation_ids.clone(),
                inputs_fingerprint: digest("inputs"),
                environment_fingerprint: digest("environment"),
                executor_session_id: None,
                reviewer_session_id: None,
            })
            .expect("trusted receipt");

        let mut stale_head = node.clone();
        stale_head.head_sha = "d".repeat(40);
        let mut omitted_obligation = node.clone();
        omitted_obligation.obligation_ids.clear();
        let mut wrong_inputs = node.clone();
        wrong_inputs.metadata["inputsFingerprint"] = serde_json::json!(digest("other inputs"));
        let mut wrong_environment = node.clone();
        wrong_environment.metadata["environmentFingerprint"] =
            serde_json::json!(digest("other environment"));
        let mut wrong_change_set = node.clone();
        wrong_change_set.metadata["changeSetFingerprint"] =
            serde_json::json!(digest("other change set"));
        for mutated in [
            stale_head,
            omitted_obligation,
            wrong_inputs,
            wrong_environment,
            wrong_change_set,
        ] {
            assert!(matches!(
                ProofGraph::default().add_verified(mutated, receipt.clone(), &verifier),
                Err(ProofError::BackendAttestation)
            ));
        }

        let mut graph = ProofGraph::default();
        graph
            .add_verified(node, receipt, &verifier)
            .expect("valid exact-head receipt");
        assert!(
            graph
                .snapshot(&"c".repeat(40))
                .expect("verified snapshot")
                .all_nodes_passed
        );
    }

    #[test]
    fn high_risk_and_missing_change_sources_compile_fail_closed_obligations() {
        let compiled = ProofObligationCompiler
            .compile(assessment(
                &digest("head"),
                &["crates/forge-kernel/src/commands.rs"],
                false,
                false,
            ))
            .expect("compiled obligations");
        assert!(compiled.broadened_for_unknown_impact);
        assert_eq!(compiled.risk, ProofRisk::High);
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
        assert!(compiled.obligations.iter().any(|obligation| {
            obligation.obligation_id == "changecone.unknown_impact"
                && obligation.required_kinds.contains(&ProofKind::Review)
        }));
    }

    #[test]
    fn certification_selects_the_smallest_exact_head_proof_set() {
        let head = digest("candidate-head");
        let compiled = ProofObligationCompiler
            .compile(assessment(
                &head,
                &["crates/api.rs", "crates/cache.rs"],
                true,
                true,
            ))
            .expect("compiled obligations");
        let mut graph = ProofGraph::default();
        for kind in [ProofKind::Static, ProofKind::Unit, ProofKind::Test] {
            add_compiled_passed(
                &mut graph,
                &compiled,
                &format!("{}.both", format!("{kind:?}").to_ascii_lowercase()),
                kind,
                ProofFixtureBindings::new(vec![], obligations_for(&compiled, kind), None, None),
            );
        }
        add_compiled_passed(
            &mut graph,
            &compiled,
            "contract.both",
            ProofKind::Contract,
            ProofFixtureBindings::new(
                vec![],
                obligations_for(&compiled, ProofKind::Contract),
                None,
                None,
            ),
        );
        let path_obligations = compiled
            .obligations
            .iter()
            .filter(|obligation| obligation.obligation_id.starts_with("change.path."))
            .collect::<Vec<_>>();
        for (index, obligation) in path_obligations.iter().enumerate() {
            add_compiled_passed(
                &mut graph,
                &compiled,
                &format!("contract.single.{index}"),
                ProofKind::Contract,
                ProofFixtureBindings::new(
                    vec![],
                    vec![obligation.obligation_id.clone()],
                    None,
                    None,
                ),
            );
        }

        let report = graph.certify(&compiled).expect("certification report");
        assert!(report.complete);
        assert_eq!(report.gaps, Vec::<ProofGap>::new());
        assert_eq!(report.selected_proof_ids.len(), 4);
        assert!(
            report
                .selected_proof_ids
                .contains(&"contract.both".to_owned())
        );
        assert!(
            !report
                .selected_proof_ids
                .iter()
                .any(|id| id.starts_with("contract.single."))
        );
    }

    #[test]
    fn certification_reports_missing_or_stale_proofs_as_gaps() {
        let compiled = ProofObligationCompiler
            .compile(assessment(
                &digest("head"),
                &["docs/native-offline.md"],
                true,
                true,
            ))
            .expect("compiled obligations");
        let mut stale_graph = ProofGraph::default();
        add_passed(
            &mut stale_graph,
            "stale.head",
            ProofKind::Contract,
            &digest("old head"),
            ProofFixtureBindings::new(
                vec![],
                obligations_for(&compiled, ProofKind::Contract),
                None,
                None,
            ),
        );
        add_passed(
            &mut stale_graph,
            "stale.change_set",
            ProofKind::Static,
            &compiled.exact_head_sha,
            ProofFixtureBindings::new(
                vec![],
                obligations_for(&compiled, ProofKind::Static),
                None,
                None,
            ),
        );
        assert_eq!(stale_graph.invalidate_stale_proofs(&compiled), 2);
        let report = stale_graph
            .certify(&compiled)
            .expect("gaps are evidence, not errors");
        assert!(!report.complete);
        assert!(
            report
                .gaps
                .iter()
                .any(|gap| gap.missing_kinds.contains(&ProofKind::Contract))
        );
    }

    #[test]
    fn certification_rejects_mutated_compiled_obligations() {
        let mut compiled = ProofObligationCompiler
            .compile(assessment(
                &digest("head"),
                &["docs/api-contract.md"],
                true,
                true,
            ))
            .expect("compiled obligations");
        compiled.obligations.clear();
        assert!(matches!(
            ProofGraph::default().certify(&compiled),
            Err(ProofError::Fingerprint)
        ));
    }
}
