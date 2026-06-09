//! OpenHuman Memory — standalone memory system.
//!
//! Provides storage, semantic retrieval, ingestion pipeline, and
//! summarization tree capabilities. Can be used as a library or
//! deployed as an independent service via `openhuman-memory-server`.
//!
//! # Feature flags
//!
//! - `__full_app` — enables modules that depend on the full OpenHuman
//!   application context (inference providers, orchestration, RPC schemas).
//!   Without this flag, only the core storage, scoring, and queue primitives
//!   are available.
//! - `__compile_store` — legacy alias, enables store submodules.

pub mod bridge;
pub mod config;
pub mod core_types;
pub mod queue;
pub mod rpc;
pub mod store;
pub mod tree;

// Modules gated behind __full_app — require orchestration layer + external deps.
#[cfg(feature = "__full_app")]
pub mod archivist;
#[cfg(feature = "__full_app")]
pub mod conversations;
#[cfg(feature = "__full_app")]
pub mod entities;
#[cfg(feature = "__full_app")]
pub mod graph;
#[cfg(feature = "__full_app")]
pub mod learning_full;
#[cfg(feature = "__full_app")]
pub mod sources;
#[cfg(feature = "__full_app")]
pub mod sync;
#[cfg(feature = "__full_app")]
pub mod tools_impl;
#[cfg(feature = "__full_app")]
pub mod orchestration;

// Stub learning module (standalone, lightweight).
#[cfg(not(feature = "__full_app"))]
pub mod learning;

// Re-export foundational crates
pub use openhuman_embeddings as embeddings;
pub use openhuman_memory_types as types;

/// Embedding extensions (factory functions not in the base embeddings crate).
pub mod embedding_ext {
    use crate::config::Config;

    pub const DEFAULT_CLOUD_EMBEDDING_MODEL: &str = "text-embedding-3-small";
    pub const DEFAULT_CLOUD_EMBEDDING_DIMENSIONS: usize = 1536;

    /// Resolve API key for an embedding provider (stub).
    pub fn resolve_api_key(_config: &Config, _provider: &str) -> Option<String> {
        None
    }

    /// Create embedding provider by name/model/dimensions (stub).
    /// Used by factory code that resolves provider slug + model explicitly.
    pub fn create_embedding_provider(
        _provider: &str,
        _model: &str,
        _dims: usize,
    ) -> anyhow::Result<Box<dyn openhuman_embeddings::EmbeddingProvider>> {
        anyhow::bail!("create_embedding_provider not available standalone")
    }

    /// Create embedding provider from config (stub).
    pub fn create_embedding_provider_from_config(
        _config: &Config,
    ) -> anyhow::Result<Box<dyn openhuman_embeddings::EmbeddingProvider>> {
        anyhow::bail!("create_embedding_provider not available standalone")
    }

    /// Create embedding provider with credentials (stub).
    pub fn create_embedding_provider_with_credentials(
        _provider: &str,
        _model: &str,
        _dims: usize,
        _base_url: Option<&str>,
        _api_key: Option<&str>,
    ) -> anyhow::Result<Box<dyn openhuman_embeddings::EmbeddingProvider>> {
        anyhow::bail!("create_embedding_provider_with_credentials not available standalone")
    }

    /// Null embedding provider — returns empty vectors (stub, for compilation only).
    struct NullEmbeddingProvider;

    #[async_trait::async_trait]
    impl openhuman_embeddings::EmbeddingProvider for NullEmbeddingProvider {
        fn name(&self) -> &str { "null" }
        fn model_id(&self) -> &str { "null" }
        fn dimensions(&self) -> usize { 0 }
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|_| vec![]).collect())
        }
    }

    /// Default embedding provider (stub — returns a null provider that errors on actual use).
    pub fn default_embedding_provider() -> std::sync::Arc<dyn openhuman_embeddings::EmbeddingProvider> {
        std::sync::Arc::new(NullEmbeddingProvider)
    }

    /// Cloud module stub.
    pub mod cloud {
        pub use super::DEFAULT_CLOUD_EMBEDDING_DIMENSIONS;
        pub use super::DEFAULT_CLOUD_EMBEDDING_MODEL;

        pub struct OpenHumanCloudEmbedding;

        impl OpenHumanCloudEmbedding {
            pub fn new(
                _api_url: Option<&str>,
                _openhuman_dir: Option<std::path::PathBuf>,
                _secrets_encrypt: bool,
                _model: &str,
                _dimensions: usize,
            ) -> Self {
                Self
            }

            pub async fn embed_one(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
                anyhow::bail!("cloud embeddings not available in standalone mode")
            }
        }
    }
}
