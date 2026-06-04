//! # Memory Store
//!
//! Core storage abstractions and implementations for the memory system.
//! Manages namespaces, documents, text chunks, vector embeddings, and
//! graph relations.

pub mod chunks;
pub mod content;
#[cfg(feature = "__full_app")]
pub mod entities;
#[cfg(feature = "__full_app")]
pub mod kinds;
pub mod kv;
#[cfg(feature = "__full_app")]
pub mod retrieval;
pub mod safety;
pub mod traits;
#[cfg(feature = "__full_app")]
pub mod trees;
pub mod types;
pub mod unified;
pub mod vectors;

// These modules have deep coupling to the full application and will be
// enabled once the Memory trait and ingestion pipeline are moved in.
#[cfg(feature = "__full_app")]
pub mod tools;
#[cfg(feature = "__full_app")]
mod client;
#[cfg(feature = "__full_app")]
pub mod factories;
#[cfg(feature = "__full_app")]
mod memory_trait;

#[cfg(feature = "__full_app")]
pub use kinds::MemoryKind;
pub use traits::{ObsidianFile, ObsidianRepresentable, VectorEmbeddable};

#[cfg(feature = "__full_app")]
pub use client::{MemoryClient, MemoryClientRef, MemoryState};

/// Compute the active embedding signature from config.
///
/// This is the canonical resolution used by the chunk store and tree store.
pub fn active_embedding_signature(
    memory_cfg: &crate::config::MemoryConfig,
    local_model_override: Option<&str>,
) -> String {
    let provider = local_model_override
        .map(|_| "ollama")
        .unwrap_or(&memory_cfg.embedding_provider);
    let model = local_model_override.unwrap_or(&memory_cfg.embedding_model);
    let dims = memory_cfg.embedding_dimensions;
    crate::embeddings::format_embedding_signature(provider, model, dims)
}

/// Return effective embedding settings from config.
pub fn effective_embedding_settings(
    memory_cfg: &crate::config::MemoryConfig,
    local_model_override: Option<&str>,
) -> (String, String, usize) {
    let provider = local_model_override
        .map(|_| "ollama".to_string())
        .unwrap_or_else(|| memory_cfg.embedding_provider.clone());
    let model = local_model_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| memory_cfg.embedding_model.clone());
    let dims = memory_cfg.embedding_dimensions;
    (provider, model, dims)
}
