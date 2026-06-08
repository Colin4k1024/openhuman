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

/// Result from fetching connected integrations.
#[derive(Clone, Debug, Default)]
pub struct FetchConnectedIntegrationsStatus {
    pub integrations: Vec<ConnectedIntegration>,
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

/// Composio client kind.
#[derive(Clone, Debug)]
pub enum ComposioClientKind {
    Default,
    Direct,
    Custom(String),
}

/// Create a composio client (stub).
pub fn create_composio_client(
    _config: &crate::config::Config,
    _kind: ComposioClientKind,
) -> anyhow::Result<Box<dyn ComposioClient>> {
    anyhow::bail!("composio client not available in standalone mode")
}

/// Direct list connections (stub).
pub async fn direct_list_connections(
    _config: &crate::config::Config,
) -> anyhow::Result<Vec<ConnectedIntegration>> {
    Ok(Vec::new())
}

/// Direct execute (stub).
pub async fn direct_execute(
    _config: &crate::config::Config,
    _action: &str,
    _params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    anyhow::bail!("composio execute not available in standalone mode")
}

/// Response from a composio execute call.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioExecuteResponse {
    pub success: bool,
    pub successful: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

/// A composio connection.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioConnection {
    pub id: String,
    pub app_name: String,
    pub status: String,
}

/// Connections response.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioConnectionsResponse {
    pub connections: Vec<ComposioConnection>,
}

/// Capability descriptor.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComposioCapability {
    pub name: String,
    pub description: String,
}

/// Trigger history stub.
pub mod trigger_history {
    pub async fn list(
        _config: &crate::config::Config,
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        Ok(Vec::new())
    }

    pub fn global() -> Vec<serde_json::Value> {
        Vec::new()
    }
}

/// Composio ops stubs.
pub mod ops {
    pub async fn fetch_connected_integrations(
        _config: &crate::config::Config,
    ) -> anyhow::Result<super::FetchConnectedIntegrationsStatus> {
        Ok(super::FetchConnectedIntegrationsStatus::default())
    }

    pub fn invalidate_connected_integrations_cache() {}

    pub fn fetch_connected_integrations_status() -> super::FetchConnectedIntegrationsStatus {
        super::FetchConnectedIntegrationsStatus::default()
    }

    pub fn report_composio_op_error(_msg: &str) {}
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

    /// Replace a managed block in a markdown document.
    pub fn replace_managed_block(doc: &str, label: &str, content: &str) -> String {
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
