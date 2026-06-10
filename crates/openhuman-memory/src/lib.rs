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

// Standalone modules — no external app dependencies.
pub mod entities;
pub mod graph;

// Modules requiring orchestration, sync, or deep app integration.
#[cfg(feature = "__full_app")]
pub mod archivist;
#[cfg(feature = "__full_app")]
pub mod conversations;
#[cfg(feature = "__full_app")]
pub mod learning_full;
#[cfg(feature = "__full_app")]
pub mod orchestration;
#[cfg(feature = "__full_app")]
pub mod sources;
#[cfg(feature = "__full_app")]
pub mod sync;
#[cfg(feature = "__full_app")]
pub mod tools_impl;

// Stub learning module (standalone, lightweight).
#[cfg(not(feature = "__full_app"))]
pub mod learning;

// Re-export foundational crates
pub use openhuman_embeddings as embeddings;
pub use openhuman_memory_types as types;

/// Embedding extensions — factory functions for creating embedding providers.
///
/// In standalone mode, this wires directly to `openhuman_embeddings::create_provider`.
/// Supports Ollama (local), OpenAI, Cohere, Voyage, and any OpenAI-compatible endpoint.
pub mod embedding_ext {
    use crate::config::Config;
    use openhuman_embeddings::{EmbeddingProvider, OllamaEmbedding, DEFAULT_OLLAMA_DIMENSIONS, DEFAULT_OLLAMA_MODEL};
    use std::sync::Arc;

    pub const DEFAULT_CLOUD_EMBEDDING_MODEL: &str = "text-embedding-3-small";
    pub const DEFAULT_CLOUD_EMBEDDING_DIMENSIONS: usize = 1536;

    const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";

    /// Resolve API key for an embedding provider from config.
    pub fn resolve_api_key(config: &Config, provider: &str) -> Option<String> {
        // Check embedding routes first
        for route in &config.embedding_routes {
            if route.provider == provider {
                if let Some(ref key) = route.api_key {
                    return Some(key.clone());
                }
            }
        }
        // Fallback to provider-specific keys in secrets
        match provider {
            "openai" => config.secrets.openai_api_key.clone(),
            "cohere" => config.secrets.cohere_api_key.clone(),
            "voyage" => config.secrets.voyage_api_key.clone(),
            _ => None,
        }
    }

    /// Create embedding provider by name/model/dimensions.
    /// For providers that need an API key, reads from environment `OPENAI_API_KEY` etc.
    pub fn create_embedding_provider(
        provider: &str,
        model: &str,
        dims: usize,
    ) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
        let api_key = resolve_api_key_from_env(provider);
        let ollama_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string());
        openhuman_embeddings::create_provider(
            provider, model, dims, &api_key, &ollama_url, None,
        )
    }

    /// Create embedding provider from a Config struct.
    pub fn create_embedding_provider_from_config(
        config: &Config,
    ) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
        let provider = config.embeddings_provider.as_deref()
            .unwrap_or(&config.memory.embedding_provider);
        let model = &config.memory.embedding_model;
        let dims = config.memory.embedding_dimensions;
        let api_key = resolve_api_key(config, provider)
            .unwrap_or_default();
        let ollama_url = config.local_ai.ollama_base_url.as_deref()
            .unwrap_or(DEFAULT_OLLAMA_URL);
        openhuman_embeddings::create_provider(
            provider, model, dims, &api_key, ollama_url, None,
        )
    }

    /// Create embedding provider with explicit credentials.
    pub fn create_embedding_provider_with_credentials(
        provider: &str,
        model: &str,
        dims: usize,
        base_url: Option<&str>,
        api_key: Option<&str>,
    ) -> anyhow::Result<Box<dyn EmbeddingProvider>> {
        let key = api_key.unwrap_or_default();
        let effective_provider = if let Some(url) = base_url {
            // Use custom: prefix for OpenAI-compatible endpoints at a custom URL
            if provider == "openai" || provider == "custom" {
                format!("custom:{}", url)
            } else {
                provider.to_string()
            }
        } else {
            provider.to_string()
        };
        let ollama_url = base_url.unwrap_or(DEFAULT_OLLAMA_URL);
        openhuman_embeddings::create_provider(
            &effective_provider, model, dims, key, ollama_url, None,
        )
    }

    /// Default embedding provider — Ollama localhost with default model.
    /// Falls back to a no-op provider if Ollama is not configured.
    pub fn default_embedding_provider() -> Arc<dyn EmbeddingProvider> {
        let ollama_url = std::env::var("OLLAMA_BASE_URL")
            .unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string());
        match OllamaEmbedding::try_new(&ollama_url, DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_DIMENSIONS) {
            Ok(p) => Arc::new(p),
            Err(_) => Arc::new(openhuman_embeddings::NoopEmbedding),
        }
    }

    /// Cloud embedding provider (OpenAI-compatible endpoint).
    pub mod cloud {
        pub use super::DEFAULT_CLOUD_EMBEDDING_DIMENSIONS;
        pub use super::DEFAULT_CLOUD_EMBEDDING_MODEL;
        use openhuman_embeddings::OpenAiEmbedding;

        pub struct OpenHumanCloudEmbedding {
            provider: OpenAiEmbedding,
        }

        impl OpenHumanCloudEmbedding {
            pub fn new(
                api_url: Option<&str>,
                _openhuman_dir: Option<std::path::PathBuf>,
                _secrets_encrypt: bool,
                model: &str,
                dimensions: usize,
            ) -> Self {
                let base_url = api_url.unwrap_or("https://api.openai.com");
                let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
                Self {
                    provider: OpenAiEmbedding::new(base_url, &api_key, model, dimensions)
                        .with_send_dimensions(model.starts_with("text-embedding-3-")),
                }
            }

            pub async fn embed_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
                use openhuman_embeddings::EmbeddingProvider;
                let results = self.provider.embed(&[text]).await?;
                results.into_iter().next()
                    .ok_or_else(|| anyhow::anyhow!("empty embedding result"))
            }
        }
    }

    /// Resolve API key from environment variables.
    fn resolve_api_key_from_env(provider: &str) -> String {
        match provider {
            "openai" => std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            "cohere" => std::env::var("COHERE_API_KEY").unwrap_or_default(),
            "voyage" => std::env::var("VOYAGE_API_KEY").unwrap_or_default(),
            _ => String::new(),
        }
    }
}
