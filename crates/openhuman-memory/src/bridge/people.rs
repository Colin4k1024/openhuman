//! People/contact abstraction used by memory for attribution.

/// Minimal person reference for memory attribution.
#[derive(Clone, Debug)]
pub struct PersonRef {
    pub id: String,
    pub display_name: Option<String>,
}
