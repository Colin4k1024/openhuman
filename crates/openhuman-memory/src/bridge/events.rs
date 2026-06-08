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
    MemoryIngestionStarted { source: String },
    MemoryIngestionCompleted { source: String, count: u32 },
    MemorySyncStageChanged { stage: String, detail: String },
    DocumentCanonicalized { source: String, doc_id: String },
    EmbeddingModelUnhealthy { provider: String, error: String },
    HealthChanged { component: String, healthy: bool, message: Option<String> },
    CacheRebuilt { scope: String },

    // Tree
    MemoryTreeBuildProgress {
        tree_scope: String,
        phase: String,
        step: String,
        detail: String,
        item_count: Option<u32>,
        level: Option<u32>,
    },
    TreeSummarizerHourCompleted { namespace: String, node_id: String, token_count: u32 },
    TreeSummarizerPropagated { namespace: String, level: u32, node_id: String, token_count: u32 },
    TreeSummarizerRebuildCompleted { namespace: String, total_nodes: u32 },

    // Channel
    ChannelMessageReceived { channel: String, message_id: String, sender: String, content: String },
    ChannelMessageProcessed { channel: String, message_id: String },

    // Composio
    ComposioConfigChanged {},
    ComposioConnectionCreated { provider: String },
    ComposioIntegrationsChanged {},
    ComposioTriggerReceived { trigger_type: String, payload: String },

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
pub fn subscribe_global(_handler: Box<dyn EventHandler>) -> SubscriptionHandle {
    SubscriptionHandle
}
