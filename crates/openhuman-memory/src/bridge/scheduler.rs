//! Scheduler gate abstraction.
//!
//! Replaces `crate::openhuman::scheduler_gate` usage in memory_queue workers.

use async_trait::async_trait;

/// Pause reason (mirrors scheduler_gate::policy::PauseReason).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PauseReason {
    UserPaused,
    LowBattery,
    LowBandwidth,
    RateLimited,
    Other(String),
}

/// Scheduler policy (mirrors scheduler_gate::policy).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerPolicy {
    Running,
    Throttled,
    Paused(PauseReason),
}

/// RAII permit — dropped when the work completes.
pub struct GatePermit(());

impl GatePermit {
    pub fn new() -> Self {
        Self(())
    }
}

/// Trait for gating background work on scheduler capacity.
#[async_trait]
pub trait SchedulerGate: Send + Sync {
    /// Block until the scheduler allows work to proceed.
    async fn wait_for_capacity(&self) -> GatePermit;

    /// Get current policy without blocking.
    fn current_policy(&self) -> SchedulerPolicy;

    /// Update configuration.
    fn update_config(&self, config: Option<serde_json::Value>);
}

/// Gate module (mirrors scheduler_gate::gate).
pub mod gate {
    use super::SchedulerPolicy;

    pub fn current_policy() -> SchedulerPolicy {
        SchedulerPolicy::Running
    }

    pub fn update_config(_config: Option<serde_json::Value>) {}
}

/// Policy module (mirrors scheduler_gate::policy).
pub mod policy {
    pub use super::{PauseReason, SchedulerPolicy};
}

/// Wait for scheduler capacity (standalone: immediate).
pub async fn wait_for_capacity() -> GatePermit {
    GatePermit::new()
}

/// No-op gate that always permits (standalone mode).
#[derive(Clone, Default)]
pub struct AlwaysPermitGate;

#[async_trait]
impl SchedulerGate for AlwaysPermitGate {
    async fn wait_for_capacity(&self) -> GatePermit {
        GatePermit::new()
    }

    fn current_policy(&self) -> SchedulerPolicy {
        SchedulerPolicy::Running
    }

    fn update_config(&self, _config: Option<serde_json::Value>) {}
}
