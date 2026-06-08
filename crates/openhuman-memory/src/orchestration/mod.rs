//! Orchestration layer — high-level memory routing, policy, ingestion,
//! query, sync, and RPC surface.
//!
//! This module is only compiled with `__full_app` since it depends on
//! inference providers, external integrations, and the agent harness.

pub mod chat;
pub mod global;
pub mod ops;
pub mod rpc_models;

pub use rpc_models::*;
pub use crate::bridge::memory_traits::ingestion::{
    MemoryIngestionConfig, MemoryIngestionRequest, MemoryIngestionResult,
};
pub mod ingest_pipeline;
pub mod ingestion;
pub mod preferences;
pub mod query;
pub mod remember;
pub mod sync;
pub mod traits;
pub mod tree_policy;
pub mod tree_source;
pub mod util;

// tools_rpc needs its own mod.rs
// pub mod tools_rpc;

pub use traits::{Memory, MemoryCategory, MemoryEntry, MemoryTaint, NamespaceSummary, RecallOpts};
