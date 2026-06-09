//! Event publishing abstraction.
//!
//! Replaces `crate::bridge::events::publish_global` with an injectable hook.

use async_trait::async_trait;

/// Fired when the memory system produces observable events.
/// Mirrors `DomainEvent` variants relevant to memory.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum MemoryEvent {
    // Memory-specific
    IngestionStarted { source: String },
    IngestionCompleted { source: String, chunks: usize },
    MemoryIngestionStarted {
        document_id: String,
        title: String,
        namespace: String,
        queue_depth: usize,
    },
    MemoryIngestionCompleted {
        document_id: String,
        namespace: String,
        success: bool,
        elapsed_ms: u64,
        queue_depth: usize,
    },
    MemorySyncStageChanged {
        trigger: String,
        stage: String,
        provider: Option<String>,
        connection_id: Option<String>,
        detail: Option<String>,
    },
    DocumentCanonicalized {
        source_id: String,
        source_kind: String,
        chunks_written: u32,
        chunk_ids: Vec<String>,
        canonicalized_at: String,
        body_preview: Option<String>,
    },
    EmbeddingModelUnhealthy {
        model: String,
        provider: Option<String>,
        fallback_provider: Option<String>,
        message: String,
    },
    HealthChanged { component: String, healthy: bool, message: Option<String> },
    CacheRebuilt {
        added: u32,
        evicted: u32,
        kept: u32,
        rebuilt_at: String,
        total_size: usize,
    },

    // Tree
    MemoryTreeBuildProgress {
        tree_scope: Option<String>,
        phase: String,
        step: String,
        detail: Option<String>,
        item_count: Option<u32>,
        level: Option<u32>,
    },
    TreeSummarizerHourCompleted { namespace: String, node_id: String, token_count: u32 },
    TreeSummarizerPropagated { namespace: String, level: String, node_id: String, token_count: u32 },
    TreeSummarizerRebuildCompleted { namespace: String, total_nodes: u64 },

    // Channel
    ChannelMessageReceived {
        channel: String,
        message_id: String,
        sender: String,
        content: String,
        reply_target: Option<String>,
        thread_ts: Option<String>,
        workspace_dir: std::path::PathBuf,
    },
    ChannelMessageProcessed {
        channel: String,
        message_id: String,
        sender: String,
        reply_target: Option<String>,
        thread_ts: Option<String>,
        response: Option<String>,
        elapsed_ms: Option<u64>,
        success: bool,
        workspace_dir: std::path::PathBuf,
    },

    // Composio
    ComposioConfigChanged { mode: Option<String>, api_key_set: bool },
    ComposioConnectionCreated { toolkit: String, connection_id: String, connect_url: Option<String> },
    ComposioIntegrationsChanged { toolkits: Vec<String> },
    ComposioTriggerReceived {
        toolkit: String,
        trigger: String,
        metadata_id: String,
        metadata_uuid: String,
        payload: serde_json::Value,
    },

    // Cron
    CronJobTriggered { job_id: String },

    // Sync
    MemorySyncRequested { channel_id: Option<String> },

    // Catch-all for events not modeled here
    Other { kind: String, payload: serde_json::Value },
}

/// Trait for publishing memory events to the broader application.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, event: MemoryEvent);
}

/// No-op publisher for standalone usage.
#[derive(Clone, Default)]
pub struct NoopEventPublisher;

#[async_trait]
impl EventPublisher for NoopEventPublisher {
    async fn publish(&self, _event: MemoryEvent) {}
}

/// Legacy alias used by store code.
pub type DomainEvent = MemoryEvent;

/// Publish a memory event globally (no-op in standalone mode).
/// In the full app, this bridges to the core event bus.
pub fn publish_global(_event: MemoryEvent) {
    // No-op in standalone mode. The full app wires this to event_bus::publish_global.
}

/// Event handler trait (mirrors core event_bus::EventHandler).
#[async_trait]
pub trait EventHandler: Send + Sync {
    fn name(&self) -> &str;
    /// Filter which event domains this handler cares about.
    fn domains(&self) -> Option<&[&str]> {
        None
    }
    async fn handle(&self, event: &MemoryEvent);
}

/// Subscription handle (RAII — drop cancels).
pub struct SubscriptionHandle;

/// Subscribe to memory events globally (no-op in standalone mode).
pub fn subscribe_global(_handler: std::sync::Arc<dyn EventHandler>) -> Option<SubscriptionHandle> {
    Some(SubscriptionHandle)
}
