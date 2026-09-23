use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Module,
    Contract,
    Capability,
    Command,
    Event,
    Configuration,
    StateSchema,
    Permission,
    Test,
    BuildArtifact,
    Evidence,
    Environment,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Requires,
    Implements,
    Calls,
    Subscribes,
    ReadsConfig,
    OwnsState,
    RequiresPermission,
    Tests,
    ProducesArtifact,
    Proves,
    Invalidates,
    CompatibleWith,
    GeneratedFrom,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    NonSemantic,
    Implementation,
    Contract,
    Configuration,
    State,
    Security,
    Toolchain,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeConfidence {
    Declared,
    Generated,
    Observed,
    Proven,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub propagation: BTreeSet<ChangeKind>,
    pub confidence: EdgeConfidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeCone {
    pub graph_epoch: u64,
    pub seeds: Vec<String>,
    pub impacted: Vec<String>,
    pub proof_nodes_to_invalidate: Vec<String>,
    pub conservative: bool,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum GraphError {
    #[error("graph node identity is invalid")]
    InvalidNode,
    #[error("graph edge references a missing node")]
    MissingNode,
    #[error("graph edge is duplicated")]
    DuplicateEdge,
    #[error("graph node is duplicated")]
    DuplicateNode,
    #[error("graph snapshot could not be serialized")]
    SnapshotSerialization,
}

pub struct CausalityGraph {
    epoch: u64,
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeSet<GraphEdgeKey>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GraphEdgeKey {
    from: String,
    to: String,
    kind: EdgeKind,
    propagation: Vec<ChangeKind>,
    confidence: EdgeConfidence,
}

#[derive(Clone, Debug)]
pub struct GraphSnapshot {
    pub epoch: u64,
    pub digest: String,
    nodes: BTreeMap<String, GraphNode>,
    edges: Vec<GraphEdge>,
}

impl Default for CausalityGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl CausalityGraph {
    pub fn new() -> Self {
        Self {
            epoch: 0,
            nodes: BTreeMap::new(),
            edges: BTreeSet::new(),
        }
    }

    pub fn add_node(&mut self, node: GraphNode) -> Result<(), GraphError> {
        if node.id.trim().is_empty() || node.id.len() > 256 || node.id.contains("..") {
            return Err(GraphError::InvalidNode);
        }
        if self.nodes.contains_key(&node.id) {
            return Err(GraphError::DuplicateNode);
        }
        self.nodes.insert(node.id.clone(), node);
        self.epoch = self.epoch.saturating_add(1);
        Ok(())
    }

    pub fn add_edge(&mut self, edge: GraphEdge) -> Result<(), GraphError> {
        if !self.nodes.contains_key(&edge.from) || !self.nodes.contains_key(&edge.to) {
            return Err(GraphError::MissingNode);
        }
        let key = GraphEdgeKey {
            from: edge.from,
            to: edge.to,
            kind: edge.kind,
            propagation: edge.propagation.into_iter().collect(),
            confidence: edge.confidence,
        };
        if !self.edges.insert(key) {
            return Err(GraphError::DuplicateEdge);
        }
        self.epoch = self.epoch.saturating_add(1);
        Ok(())
    }

    pub fn snapshot(&self) -> Result<GraphSnapshot, GraphError> {
        let edges = self
            .edges
            .iter()
            .map(|edge| GraphEdge {
                from: edge.from.clone(),
                to: edge.to.clone(),
                kind: edge.kind,
                propagation: edge.propagation.iter().copied().collect(),
                confidence: edge.confidence,
            })
            .collect::<Vec<_>>();
        let serialized = serde_json::to_vec(&(&self.nodes, &edges))
            .map_err(|_| GraphError::SnapshotSerialization)?;
        Ok(GraphSnapshot {
            epoch: self.epoch,
            digest: blake3::hash(&serialized).to_hex().to_string(),
            nodes: self.nodes.clone(),
            edges,
        })
    }
}

impl GraphSnapshot {
    pub fn compute_change_cone(&self, seeds: &[String], kind: ChangeKind) -> ChangeCone {
        let mut impacted = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut pending = VecDeque::new();
        let mut conservative = kind == ChangeKind::Unknown;
        let mut seed_nodes = BTreeSet::new();
        for seed in seeds.iter().filter(|seed| self.nodes.contains_key(*seed)) {
            visited.insert(seed.clone());
            seed_nodes.insert(seed.clone());
            pending.push_back(seed.clone());
        }
        while let Some(current) = pending.pop_front() {
            for edge in self.edges.iter().filter(|edge| edge.from == current) {
                let low_confidence = edge.confidence < EdgeConfidence::Observed;
                if low_confidence && visited.contains(&current) {
                    conservative = true;
                }
                if !conservative && !edge.propagation.contains(&kind) && !low_confidence {
                    continue;
                }
                if !seed_nodes.contains(&edge.to) {
                    impacted.insert(edge.to.clone());
                }
                if visited.insert(edge.to.clone()) {
                    pending.push_back(edge.to.clone());
                }
            }
        }
        let proof_nodes_to_invalidate = impacted
            .iter()
            .filter(|node| {
                self.nodes.get(*node).is_some_and(|item| {
                    matches!(
                        item.kind,
                        NodeKind::Test | NodeKind::Evidence | NodeKind::BuildArtifact
                    )
                })
            })
            .cloned()
            .collect();
        ChangeCone {
            graph_epoch: self.epoch,
            seeds: seeds
                .iter()
                .filter(|seed| self.nodes.contains_key(*seed))
                .cloned()
                .collect(),
            impacted: impacted.into_iter().collect(),
            proof_nodes_to_invalidate,
            conservative,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(
        from: &str,
        to: &str,
        propagation: &[ChangeKind],
        confidence: EdgeConfidence,
    ) -> GraphEdge {
        GraphEdge {
            from: from.into(),
            to: to.into(),
            kind: EdgeKind::Tests,
            propagation: propagation.iter().copied().collect(),
            confidence,
        }
    }

    #[test]
    fn semantic_propagation_invalidates_only_affected_proofs() {
        let mut graph = CausalityGraph::new();
        graph
            .add_node(GraphNode {
                id: "src/kernel".into(),
                kind: NodeKind::Module,
            })
            .expect("node");
        graph
            .add_node(GraphNode {
                id: "test/kernel".into(),
                kind: NodeKind::Test,
            })
            .expect("node");
        graph
            .add_node(GraphNode {
                id: "test/docs".into(),
                kind: NodeKind::Test,
            })
            .expect("node");
        graph
            .add_edge(edge(
                "src/kernel",
                "test/kernel",
                &[ChangeKind::Implementation],
                EdgeConfidence::Proven,
            ))
            .expect("edge");
        graph
            .add_edge(edge(
                "src/kernel",
                "test/docs",
                &[ChangeKind::NonSemantic],
                EdgeConfidence::Proven,
            ))
            .expect("edge");
        let cone = graph
            .snapshot()
            .expect("snapshot")
            .compute_change_cone(&["src/kernel".into()], ChangeKind::Implementation);
        assert_eq!(cone.impacted, vec!["test/kernel"]);
        assert_eq!(cone.proof_nodes_to_invalidate, vec!["test/kernel"]);
    }

    #[test]
    fn unknown_changes_conservatively_expand_impact_and_cycles_terminate() {
        let mut graph = CausalityGraph::new();
        for id in ["a", "b", "evidence"] {
            graph
                .add_node(GraphNode {
                    id: id.into(),
                    kind: if id == "evidence" {
                        NodeKind::Evidence
                    } else {
                        NodeKind::Module
                    },
                })
                .expect("node");
        }
        graph
            .add_edge(edge(
                "a",
                "b",
                &[ChangeKind::NonSemantic],
                EdgeConfidence::Declared,
            ))
            .expect("edge");
        graph
            .add_edge(edge(
                "b",
                "a",
                &[ChangeKind::NonSemantic],
                EdgeConfidence::Declared,
            ))
            .expect("edge");
        graph
            .add_edge(edge(
                "b",
                "evidence",
                &[ChangeKind::Contract],
                EdgeConfidence::Proven,
            ))
            .expect("edge");
        let cone = graph
            .snapshot()
            .expect("snapshot")
            .compute_change_cone(&["a".into()], ChangeKind::Unknown);
        assert!(cone.conservative);
        assert_eq!(cone.impacted, vec!["b", "evidence"]);
        assert_eq!(cone.proof_nodes_to_invalidate, vec!["evidence"]);
    }
}
