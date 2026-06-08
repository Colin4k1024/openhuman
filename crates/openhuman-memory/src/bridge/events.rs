//! Event publishing abstraction.
//!
//! Replaces `crate::core::event_bus::publish_global` with an injectable hook.

use async_trait::async_trait;

/// Fired when the memory system produces observable events.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum MemoryEvent {
    IngestionStarted { source: String },
    IngestionCompleted { source: String, chunks: usize },
    EmbeddingModelUnhealthy { provider: String, error: String },
    HealthChanged { component: String, healthy: bool, message: Option<String> },
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
