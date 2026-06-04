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

/// Top-level config struct that the memory store expects.
///
/// In the full OpenHuman app, this is constructed from the main `Config`.
/// When running standalone, it's loaded from a TOML file or env vars.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub memory: MemoryConfig,
    pub storage: StorageConfig,
    pub embedding_routes: Vec<EmbeddingRouteConfig>,
    /// Workspace root directory (where DBs and raw files live).
    pub workspace_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = default_data_dir();
        Self {
            memory: MemoryConfig::default(),
            storage: StorageConfig::default(),
            embedding_routes: Vec::new(),
            workspace_dir: data_dir,
        }
    }
}

impl Config {
    /// Returns the workspace directory.
    pub fn workspace_dir(&self) -> &std::path::Path {
        &self.workspace_dir
    }

    /// Returns the default root openhuman directory.
    pub fn default_root_openhuman_dir() -> PathBuf {
        default_data_dir()
    }

    /// Returns the content root for the memory tree vault.
    pub fn memory_tree_content_root(&self) -> PathBuf {
        self.workspace_dir.join("memory_tree")
    }

    /// Returns the local model override for a given workload (e.g. "embeddings").
    pub fn workload_local_model(&self, _workload: &str) -> Option<String> {
        // In standalone mode, no local model override.
        // Full app wires this to local_ai config.
        None
    }
}

/// Storage provider configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub provider: StorageProviderConfig,
}

/// Storage provider details.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageProviderConfig {
    pub provider: String,
}

/// RPC config helpers (placeholder for config_rpc references in tools).
pub mod rpc {
    use super::Config;
    use once_cell::sync::OnceCell;
    use std::sync::Arc;

    static GLOBAL_CONFIG: OnceCell<Arc<Config>> = OnceCell::new();

    /// Set the global config (called once at startup).
    pub fn set_config(config: Config) {
        let _ = GLOBAL_CONFIG.set(Arc::new(config));
    }

    /// Get the global config.
    pub fn get_config() -> Arc<Config> {
        GLOBAL_CONFIG
            .get()
            .cloned()
            .unwrap_or_else(|| Arc::new(Config::default()))
    }
}

/// Returns the default root directory for OpenHuman data.
pub fn default_root_openhuman_dir() -> PathBuf {
    default_data_dir()
}
