//! Composio abstraction — external integration client.
//!
//! Replaces `crate::openhuman::composio` usage for Gmail fetch, profile blocks, etc.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Integration connection status.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntegrationStatus {
    Connected,
    Disconnected,
    Error(String),
}

/// Entry in the authoritative integrations list.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IntegrationEntry {
    pub toolkit: String,
    pub connected: bool,
    pub connection_id: Option<String>,
}

/// Result from fetching connected integrations — distinguishes authoritative
/// responses (backend replied) from unavailable (backend down/unauthed).
#[derive(Clone, Debug)]
pub enum FetchConnectedIntegrationsStatus {
    /// Backend responded with a (possibly empty) list.
    Authoritative(Vec<IntegrationEntry>),
    /// Backend was unreachable or returned an error.
    Unavailable,
}

impl Default for FetchConnectedIntegrationsStatus {
    fn default() -> Self {
        Self::Unavailable
    }
}

/// A connected integration entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectedIntegration {
    pub id: String,
    pub provider: String,
    pub status: IntegrationStatus,
}

/// Composio client trait for external service operations.
#[async_trait]
pub trait ComposioClient: Send + Sync {
    async fn fetch_connected_integrations(&self) -> anyhow::Result<FetchConnectedIntegrationsStatus>;
    async fn execute_action(&self, action: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value>;
    /// Execute a named tool with optional JSON params.
    async fn execute_tool(&self, action: &str, params: Option<serde_json::Value>) -> anyhow::Result<ComposioExecuteResponse>;
    /// List all connections for the current entity.
    async fn list_connections(&self) -> anyhow::Result<ComposioConnectionsResponse>;
}

#[async_trait]
impl ComposioClient for Box<dyn ComposioClient> {
    async fn fetch_connected_integrations(&self) -> anyhow::Result<FetchConnectedIntegrationsStatus> {
        (**self).fetch_connected_integrations().await
    }
    async fn execute_action(&self, action: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        (**self).execute_action(action, params).await
    }
    async fn execute_tool(&self, action: &str, params: Option<serde_json::Value>) -> anyhow::Result<ComposioExecuteResponse> {
        (**self).execute_tool(action, params).await
    }
    async fn list_connections(&self) -> anyhow::Result<ComposioConnectionsResponse> {
        (**self).list_connections().await
    }
}

/// Profile/identity helpers.
pub mod providers {
    /// Re-export profile_md at providers level.
    pub mod profile_md {
        pub use super::super::profile_md::*;
    }

    pub mod profile {
        /// Identity kind for self-recognition.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum IdentityKind {
            Email,
            Phone,
            Username,
            FullName,
            Handle,
            Other(String),
        }

        /// Check if a text matches any known self-identity of the given kind.
        pub fn is_self_identity_any_toolkit(_kind: IdentityKind, _text: &str) -> bool {
            // Stub — full app provides real implementation.
            false
        }
    }
}

/// Composio client kind — mirrors the real `ComposioClientKind` in the main app.
pub enum ComposioClientKind {
    /// Backend-proxied client.
    Backend(Box<dyn ComposioClient>),
    /// Direct-mode client (holds the API key / endpoint string).
    Direct(String),
}

/// Create a composio client from config, returning the client kind
/// (Backend or Direct) so callers can dispatch appropriately.
/// Returns Err in standalone mode.
pub fn create_composio_client(
    _config: &crate::config::Config,
) -> anyhow::Result<ComposioClientKind> {
    anyhow::bail!("composio client not available in standalone mode")
}

/// Direct list connections (stub).
pub async fn direct_list_connections(
    _direct_key: &str,
) -> anyhow::Result<ComposioConnectionsResponse> {
    Ok(ComposioConnectionsResponse::default())
}

/// Direct execute via a Composio direct-mode client (stub).
/// Signature: (direct_key, action, params, entity_id)
pub async fn direct_execute(
    _direct_key: &str,
    _action: &str,
    _params: Option<serde_json::Value>,
    _entity_id: &str,
) -> anyhow::Result<ComposioExecuteResponse> {
    anyhow::bail!("composio execute not available in standalone mode")
}

/// Response from a composio execute call.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioExecuteResponse {
    pub success: bool,
    pub successful: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
    /// Markdown-formatted response content.
    pub markdown_formatted: Option<String>,
    /// Cost in USD for this execute call.
    pub cost_usd: f64,
}

/// A composio connection.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioConnection {
    pub id: String,
    pub app_name: String,
    pub status: String,
    /// Normalized toolkit name (e.g. "gmail", "slack").
    pub toolkit: String,
    /// Raw toolkit identifier from the Composio API.
    pub app_unique_id: String,
}

impl ComposioConnection {
    /// Returns true if the connection is in an active/connected state.
    pub fn is_active(&self) -> bool {
        matches!(self.status.to_lowercase().as_str(), "active" | "connected")
    }

    /// Returns the normalized toolkit name (lowercase app_name).
    pub fn normalized_toolkit(&self) -> String {
        if !self.toolkit.is_empty() {
            self.toolkit.to_lowercase()
        } else {
            self.app_name.to_lowercase()
        }
    }
}

/// Connections response.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioConnectionsResponse {
    pub connections: Vec<ComposioConnection>,
}

/// Capability descriptor — mirrors the real `ComposioCapability` in the main app.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioCapability {
    pub name: String,
    pub description: String,
    /// Toolkit/app identifier.
    pub toolkit: String,
    /// Whether there is a native provider for this toolkit.
    pub native_provider: bool,
    /// Number of curated tools for this capability.
    pub curated_tool_count: usize,
    /// Whether this toolkit has a curated tool catalog.
    pub curated_tools: bool,
    /// Whether tool execution is enabled.
    pub tool_execution: bool,
    /// Whether trigger webhooks are enabled.
    pub trigger_webhooks: bool,
    /// Whether memory ingestion is enabled.
    pub memory_ingest: bool,
    /// Whether periodic sync is enabled.
    pub periodic_sync: bool,
    /// Whether initial sync is enabled.
    pub initial_sync: bool,
    /// Sync interval in seconds.
    pub sync_interval_secs: Option<u64>,
    /// Whether user profile extraction is enabled.
    pub user_profile: bool,
}

/// Trigger history stub.
pub mod trigger_history {
    use std::sync::Arc;

    pub trait TriggerHistoryStore: Send + Sync {
        fn record_trigger(
            &self,
            toolkit: &str,
            trigger: &str,
            metadata_id: &str,
            metadata_uuid: &str,
            payload: &serde_json::Value,
        ) -> anyhow::Result<()>;
    }

    pub async fn list(
        _config: &crate::config::Config,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        Ok(Vec::new())
    }

    pub fn global() -> Option<Arc<dyn TriggerHistoryStore>> {
        None
    }
}

/// Composio ops stubs.
pub mod ops {
    pub async fn fetch_connected_integrations(
        _config: &crate::config::Config,
    ) -> anyhow::Result<Vec<super::ConnectedIntegration>> {
        Ok(Vec::new())
    }

    pub fn invalidate_connected_integrations_cache() {}

    pub async fn fetch_connected_integrations_status(
        _config: &crate::config::Config,
    ) -> super::FetchConnectedIntegrationsStatus {
        super::FetchConnectedIntegrationsStatus::Unavailable
    }

    pub fn report_composio_op_error(_kind: &str, _msg: &str) {}
}

/// Profile.md managed-block helpers.
pub mod profile_md {
    /// Start marker for a managed block.
    pub fn block_start(label: &str) -> String {
        format!("<!-- BEGIN {} -->", label)
    }

    /// End marker for a managed block.
    pub fn block_end(label: &str) -> String {
        format!("<!-- END {} -->", label)
    }

    /// Replace a managed block in the profile.md file inside `workspace_dir`.
    /// `label` is the block name, `heading` is inserted before the content.
    /// No-op stub in standalone mode — returns Ok(()) immediately.
    pub fn replace_managed_block(
        _workspace_dir: &std::path::Path,
        _label: &str,
        _heading: &str,
        _content: String,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace a managed block in a markdown document string (in-memory version).
    pub fn replace_managed_block_str(doc: &str, label: &str, content: &str) -> String {
        let start = block_start(label);
        let end = block_end(label);
        if let (Some(s), Some(e)) = (doc.find(&start), doc.find(&end)) {
            let before = &doc[..s];
            let after = &doc[e + end.len()..];
            format!("{}{}\n{}\n{}{}", before, start, content, end, after)
        } else {
            // Append if block doesn't exist
            format!("{}\n{}\n{}\n{}\n", doc, start, content, end)
        }
    }
}
