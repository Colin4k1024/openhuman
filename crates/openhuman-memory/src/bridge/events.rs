//! Event publishing abstraction.
//!
//! Replaces `crate::core::event_bus::publish_global` with an injectable hook.

use async_trait::async_trait;

/// Fired when the memory system produces observable events.
#[derive(Clone, Debug)]
pub enum MemoryEvent {
    IngestionStarted { source: String },
    IngestionCompleted { source: String, chunks: usize },
    EmbeddingModelUnhealthy { provider: String, error: String },
    HealthChanged { component: String, healthy: bool },
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
