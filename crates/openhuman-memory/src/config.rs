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

/// Memory tree configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryTreeConfig {
    /// Max concurrent summarization tasks.
    pub max_concurrent_summarize: usize,
    /// Enable/disable tree engine.
    pub enabled: bool,
    /// Cloud summarization opt-in.
    pub cloud_summarization_opt_in: bool,
    /// Custom embedding endpoint.
    pub embedding_endpoint: Option<String>,
    /// Embedding model override.
    pub embedding_model: Option<String>,
    /// Cloud LLM model override.
    pub cloud_llm_model: Option<String>,
    /// Model used for smart-walk query expansion.
    pub smart_walk_model: Option<String>,
    /// Timeout for embedding requests (ms).
    pub embedding_timeout_ms: Option<u64>,
}

/// Secrets configuration (API keys, tokens).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SecretsConfig {
    pub openai_api_key: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub cohere_api_key: Option<String>,
    pub voyage_api_key: Option<String>,
    /// Whether secrets are stored encrypted.
    pub encrypt: bool,
}

/// Local AI configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalAiConfig {
    pub ollama_base_url: Option<String>,
    pub default_model: Option<String>,
    pub chat_model_id: String,
    pub runtime_enabled: bool,
}

/// Scheduler gate mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulerGateMode {
    /// Decide based on power + CPU + deployment-mode signals.
    #[default]
    Auto,
    /// Always run background AI flat-out.
    AlwaysOn,
    /// Never run background AI.
    Off,
    /// Legacy aliases kept for serde compat.
    On,
    Throttled,
}

impl SchedulerGateMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::AlwaysOn => "always_on",
            Self::Off => "off",
            Self::On => "on",
            Self::Throttled => "throttled",
        }
    }
}

/// Scheduler gate configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SchedulerGateConfig {
    pub enabled: bool,
    pub mode: SchedulerGateMode,
}

/// Reliability tuning.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReliabilityConfig {
    pub max_retries: u32,
    pub timeout_secs: u64,
    pub provider_retries: u32,
    pub provider_backoff_ms: u64,
}

/// Learning configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LearningConfig {
    pub enabled: bool,
    pub reflection_interval_secs: u64,
    pub max_candidates: usize,
    /// Enable post-turn reflection (observation extraction). Default: true.
    #[serde(default = "learning_default_true")]
    pub reflection_enabled: bool,
    /// Enable automatic user profile extraction. Default: true.
    #[serde(default = "learning_default_true")]
    pub user_profile_enabled: bool,
    /// Enable tool effectiveness tracking. Default: true.
    #[serde(default = "learning_default_true")]
    pub tool_tracking_enabled: bool,
    /// Which LLM to use for reflection.
    #[serde(default)]
    pub reflection_source: ReflectionSource,
    /// Maximum reflections per session before throttling. Default: 20.
    #[serde(default = "learning_default_max_reflections")]
    pub max_reflections_per_session: usize,
    /// Minimum tool calls in a turn to trigger reflection. Default: 1.
    #[serde(default = "learning_default_min_turn_complexity")]
    pub min_turn_complexity: usize,
}

fn learning_default_true() -> bool { true }
fn learning_default_max_reflections() -> usize { 20 }
fn learning_default_min_turn_complexity() -> usize { 1 }

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reflection_interval_secs: 0,
            max_candidates: 0,
            reflection_enabled: true,
            user_profile_enabled: true,
            tool_tracking_enabled: true,
            reflection_source: ReflectionSource::default(),
            max_reflections_per_session: 20,
            min_turn_complexity: 1,
        }
    }
}

/// Top-level config struct that the memory store expects.
///
/// In the full OpenHuman app, this is constructed from the main `Config`.
/// When running standalone, it's loaded from a TOML file or env vars.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub memory: MemoryConfig,
    pub memory_tree: MemoryTreeConfig,
    pub storage: StorageConfig,
    pub embedding_routes: Vec<EmbeddingRouteConfig>,
    pub secrets: SecretsConfig,
    pub local_ai: LocalAiConfig,
    pub scheduler_gate: SchedulerGateConfig,
    pub reliability: ReliabilityConfig,
    /// Workspace root directory (where DBs and raw files live).
    pub workspace_dir: PathBuf,
    /// Default LLM model for summarization etc.
    pub default_model: Option<String>,
    /// Output language for LLM-generated content.
    pub output_language: Option<String>,
    /// Embedding provider name.
    pub embeddings_provider: Option<String>,
    /// Memory sources configuration.
    #[cfg(feature = "__full_app")]
    pub memory_sources: Vec<crate::sources::types::MemorySourceEntry>,
    #[cfg(not(feature = "__full_app"))]
    pub memory_sources: serde_json::Value,
    /// Learning configuration.
    pub learning: LearningConfig,
    /// Path to the config.json file (used by `save()` and test helpers).
    #[serde(skip)]
    pub config_path: std::path::PathBuf,
    /// Composio integration configuration (only used by full-app modules).
    #[cfg(feature = "__full_app")]
    #[serde(default)]
    pub composio: ComposioConfig,
    /// Backend API URL (only used by full-app modules).
    #[cfg(feature = "__full_app")]
    #[serde(default)]
    pub api_url: Option<String>,
    /// Backend API key (only used by full-app modules).
    #[cfg(feature = "__full_app")]
    #[serde(default)]
    pub api_key: Option<String>,
    /// Custom LLM inference endpoint (only used by full-app modules).
    #[cfg(feature = "__full_app")]
    #[serde(default)]
    pub inference_url: Option<String>,
    /// Runtime configuration.
    #[serde(default)]
    pub runtime: RuntimeConfig,
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = default_data_dir();
        Self {
            memory: MemoryConfig::default(),
            memory_tree: MemoryTreeConfig::default(),
            storage: StorageConfig::default(),
            embedding_routes: Vec::new(),
            secrets: SecretsConfig::default(),
            local_ai: LocalAiConfig::default(),
            scheduler_gate: SchedulerGateConfig::default(),
            reliability: ReliabilityConfig::default(),
            workspace_dir: data_dir.clone(),
            default_model: None,
            output_language: None,
            embeddings_provider: None,
            learning: LearningConfig::default(),
            #[cfg(feature = "__full_app")]
            memory_sources: Vec::new(),
            #[cfg(not(feature = "__full_app"))]
            memory_sources: serde_json::Value::Null,
            config_path: data_dir.join("config.json"),
            #[cfg(feature = "__full_app")]
            composio: ComposioConfig::default(),
            #[cfg(feature = "__full_app")]
            api_url: None,
            #[cfg(feature = "__full_app")]
            api_key: None,
            #[cfg(feature = "__full_app")]
            inference_url: None,
            runtime: RuntimeConfig::default(),
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

    /// Returns true if the given workload should use local (Ollama) inference.
    /// In the full app this is driven by `LocalAiConfig`; here it is always false.
    pub fn workload_uses_local(&self, _workload: &str) -> bool {
        false
    }

    /// Build an output-language directive string for LLM prompts.
    pub fn output_language_directive(&self) -> Option<String> {
        output_language_directive(self.output_language.as_deref())
    }

    /// System prompt override (stub — full app wires to agent config).
    pub fn prompt(&self) -> Option<&str> {
        None
    }

    /// Apply environment-variable overrides to this config.
    /// Stub — full app reads `OPENHUMAN_*` env vars here.
    pub fn apply_env_overrides(&mut self) {}

    /// Persist the config to `self.config_path`.
    ///
    /// In standalone mode this serialises as JSON (no `toml` dependency).
    /// The full app uses TOML — call sites that use this via `__full_app`
    /// depend on the main `Config::save()` which does use TOML.
    pub async fn save(&self) -> anyhow::Result<()> {
        use tokio::io::AsyncWriteExt as _;
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("failed to serialize config: {e}"))?;
        if let Some(parent) = self.config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut f = tokio::fs::File::create(&self.config_path).await?;
        f.write_all(json.as_bytes()).await?;
        Ok(())
    }

    /// Load config from the default location or initialise defaults.
    /// Mirrors `Config::load_or_init()` from the main app.
    pub async fn load_or_init() -> anyhow::Result<Self> {
        let config_path = default_data_dir().join("config.json");
        if config_path.exists() {
            let contents = tokio::fs::read_to_string(&config_path).await?;
            // Try JSON first (standalone), then fall back to defaults.
            let mut cfg: Config = serde_json::from_str(&contents)
                .unwrap_or_default();
            cfg.config_path = config_path;
            Ok(cfg)
        } else {
            let mut cfg = Config::default();
            cfg.config_path = config_path;
            Ok(cfg)
        }
    }
}

/// Runtime configuration stub.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    /// Whether reasoning/thinking mode is enabled.
    pub reasoning_enabled: bool,
}

/// Composio integration configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ComposioConfig {
    /// Composio mode: "direct" or "backend".
    pub mode: Option<String>,
    /// Composio API key for direct mode.
    pub api_key: Option<String>,
    /// Backend endpoint for backend mode.
    pub backend_url: Option<String>,
    /// Entity/account identifier for this Composio connection.
    pub entity_id: Option<String>,
    /// Toolkits disabled from triage/sync.
    #[serde(default)]
    pub triage_disabled_toolkits: Vec<String>,
    /// Whether triage is disabled globally.
    #[serde(default)]
    pub triage_disabled: bool,
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
    pub(crate) fn set_config(config: Config) {
        let _ = GLOBAL_CONFIG.set(Arc::new(config));
    }

    /// Get the global config.
    pub(crate) fn get_config() -> Arc<Config> {
        GLOBAL_CONFIG
            .get()
            .cloned()
            .unwrap_or_else(|| Arc::new(Config::default()))
    }

    /// Load config with timeout (async, returns a cloned owned Config).
    /// In the full app this waits for config to be ready; here it returns immediately.
    /// Returns an owned `Config` so call sites can mutate and save.
    pub(crate) async fn load_config_with_timeout() -> Result<Config, String> {
        Ok((*get_config()).clone())
    }

    /// Alias for synchronous access.
    pub(crate) fn config() -> Arc<Config> {
        get_config()
    }

    /// Reload config snapshot (async stub).
    /// Accepts a reference to the current config to mirror the real-app signature.
    pub(crate) async fn reload_config_snapshot_with_timeout(
        _current: &std::sync::Arc<Config>,
    ) -> Result<Config, String> {
        Ok((*get_config()).clone())
    }
}

/// Returns the default root directory for OpenHuman data.
pub fn default_root_openhuman_dir() -> PathBuf {
    default_data_dir()
}

/// Default cloud LLM model.
pub const DEFAULT_CLOUD_LLM_MODEL: &str = "claude-3-haiku-20240307";

/// Reflection source configuration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum ReflectionSource {
    #[default]
    Cloud,
    Local,
}

/// Config schema stubs.
pub mod schema {
    pub use super::*;

    /// Composio mode constant.
    pub const COMPOSIO_MODE_DIRECT: &str = "direct";
}

/// Config ops stubs.
pub mod ops {
    pub(crate) use super::rpc::{config, get_config, load_config_with_timeout};
}

/// Stub for the LocalAiService that handles local LLM inference.
pub struct LocalAiServiceStub;

impl LocalAiServiceStub {
    /// Run an LLM prompt via local inference (stub — returns error in standalone mode).
    pub async fn prompt(
        &self,
        _config: &Config,
        _prompt: &str,
        _max_tokens: Option<u32>,
        _flag: bool,
    ) -> anyhow::Result<String> {
        anyhow::bail!("local AI not available in standalone mode")
    }
}

/// Global config accessor / local AI service factory.
///
/// When called with a config reference, returns a `LocalAiServiceStub`
/// that can run local LLM prompts. Mirrors the real app's `local_ai::global(&config)`.
pub fn global(_config: &Config) -> LocalAiServiceStub {
    LocalAiServiceStub
}

/// Default Ollama base URL.
pub const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";

/// Get the Ollama base URL from global config.
/// Call sites that have a `Config` reference should prefer `ollama_base_url_from(config)`.
pub fn ollama_base_url() -> String {
    rpc::get_config()
        .local_ai
        .ollama_base_url
        .clone()
        .unwrap_or_else(|| OLLAMA_BASE_URL.to_string())
}

/// Get the configured Ollama base URL from a specific config.
pub fn ollama_base_url_from(config: &Config) -> String {
    config
        .local_ai
        .ollama_base_url
        .clone()
        .unwrap_or_else(|| OLLAMA_BASE_URL.to_string())
}

/// Build a language directive string for LLM prompts.
pub fn output_language_directive(output_language: Option<&str>) -> Option<String> {
    match output_language {
        Some(lang) if !lang.is_empty() => Some(format!("Respond in {}.", lang)),
        _ => None,
    }
}
