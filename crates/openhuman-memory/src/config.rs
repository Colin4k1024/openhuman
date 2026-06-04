//! Configuration types for the memory system.
//!
//! These mirror the relevant portions of OpenHuman's `Config` struct but are
//! self-contained so the memory crate has no dependency on the main application
//! config machinery.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Top-level configuration for the memory system.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// Storage backend: "sqlite" (default) or "agentmemory".
    pub backend: String,
    /// Whether to auto-save ingested content.
    pub auto_save: bool,
    /// Embedding provider slug: "cloud", "ollama", "openai", "cohere", "voyage", "custom", "none".
    pub embedding_provider: String,
    /// Model identifier for the embedding provider.
    pub embedding_model: String,
    /// Embedding vector dimensions.
    pub embedding_dimensions: usize,
    /// Rate limit for cloud embedding requests (requests/min). 0 = disabled.
    pub embedding_rate_limit_per_min: u32,
    /// Minimum relevance score for retrieval results.
    pub min_relevance_score: f64,
    /// SQLite busy-timeout in seconds.
    pub sqlite_open_timeout_secs: Option<u64>,
    /// Base URL for the agentmemory REST backend.
    pub agentmemory_url: Option<String>,
    /// Bearer token for agentmemory REST backend.
    pub agentmemory_secret: Option<String>,
    /// Per-request timeout for agentmemory (ms).
    pub agentmemory_timeout_ms: Option<u64>,
    /// Root directory for memory data (SQLite DBs, raw files, trees).
    pub data_dir: PathBuf,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: "sqlite".into(),
            auto_save: true,
            embedding_provider: "cloud".into(),
            embedding_model: "embedding-v1".into(),
            embedding_dimensions: 1024,
            embedding_rate_limit_per_min: 60,
            min_relevance_score: 0.4,
            sqlite_open_timeout_secs: None,
            agentmemory_url: None,
            agentmemory_secret: None,
            agentmemory_timeout_ms: None,
            data_dir: default_data_dir(),
        }
    }
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openhuman")
        .join("memory")
}

/// Embedding route configuration — maps provider slugs to endpoints/keys.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbeddingRouteConfig {
    pub provider: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub dimensions: Option<usize>,
}
