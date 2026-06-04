//! Learning module stubs — will be fully implemented when learning domain is moved in.

pub mod candidate {
    use serde::{Deserialize, Serialize};

    /// A reference to evidence supporting a learned pattern.
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct EvidenceRef {
        pub source: String,
        pub chunk_id: Option<String>,
        pub relevance: f64,
    }
}
