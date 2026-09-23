use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintKind {
    Content,
    Semantic,
    Contract,
    Capability,
    Config,
    StateEpoch,
    GraphEpoch,
    Toolchain,
    Decision,
    TestProof,
    Build,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Fingerprint {
    pub schema_version: u16,
    pub algorithm: String,
    pub kind: FingerprintKind,
    pub value: String,
}

impl Fingerprint {
    pub fn from_bytes(kind: FingerprintKind, bytes: &[u8]) -> Self {
        Self {
            schema_version: 1,
            algorithm: "blake3".to_owned(),
            kind,
            value: blake3::hash(bytes).to_hex().to_string(),
        }
    }

    pub fn from_json(
        kind: FingerprintKind,
        value: &serde_json::Value,
    ) -> Result<Self, serde_json::Error> {
        serde_json::to_vec(value).map(|canonical| Self::from_bytes(kind, &canonical))
    }

    pub fn content(bytes: &[u8]) -> Self {
        Self::from_bytes(FingerprintKind::Content, bytes)
    }

    pub fn semantic(value: &serde_json::Value) -> Result<Self, serde_json::Error> {
        Self::from_json(FingerprintKind::Semantic, value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_key_order_does_not_change_semantic_fingerprint() {
        let left = json!({"a": 1, "b": 2});
        let right = json!({"b": 2, "a": 1});
        assert_eq!(
            Fingerprint::semantic(&left).expect("JSON serialization"),
            Fingerprint::semantic(&right).expect("JSON serialization")
        );
    }

    #[test]
    fn content_and_semantic_fingerprints_are_domain_separated() {
        let content = Fingerprint::content(b"{}");
        let semantic = Fingerprint::semantic(&serde_json::json!({})).expect("JSON serialization");
        assert_ne!(content.kind, semantic.kind);
        assert_eq!(content.algorithm, "blake3");
    }
}
