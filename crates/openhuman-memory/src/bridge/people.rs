//! People/contact abstraction used by memory for attribution.

/// Submodule for type compatibility with `crate::bridge::people::types::*`.
pub mod types {
    use serde::{Deserialize, Serialize};

    /// Entity type for people in the memory system.
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Person {
        pub id: String,
        pub display_name: Option<String>,
        pub primary_email: Option<String>,
    }
}

/// Minimal person reference for memory attribution.
#[derive(Clone, Debug)]
pub struct PersonRef {
    pub id: String,
    pub display_name: Option<String>,
}
