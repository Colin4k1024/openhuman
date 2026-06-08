//! Integration client abstraction.
//!
//! Replaces `crate::openhuman::integrations::build_client` / `IntegrationClient`.

use async_trait::async_trait;

/// Generic HTTP integration client (used for Apify, external APIs, etc.).
#[async_trait]
pub trait IntegrationClient: Send + Sync {
    /// Make a GET request to the integration endpoint.
    async fn get(&self, path: &str) -> anyhow::Result<serde_json::Value>;

    /// Make a POST request with a JSON body.
    async fn post(&self, path: &str, body: serde_json::Value) -> anyhow::Result<serde_json::Value>;
}

/// Builder for integration clients.
pub trait IntegrationClientFactory: Send + Sync {
    fn build_client(&self, base_url: &str, api_key: Option<&str>) -> Box<dyn IntegrationClient>;
}

/// Build an integration client (stub).
pub fn build_client(
    _base_url: &str,
    _api_key: Option<&str>,
) -> anyhow::Result<Box<dyn IntegrationClient>> {
    anyhow::bail!("integration client not available in standalone mode")
}
