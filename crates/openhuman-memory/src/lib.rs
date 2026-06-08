//! OpenHuman Memory — standalone memory system.
//!
//! Provides storage, semantic retrieval, ingestion pipeline, and
//! summarization tree capabilities. Can be used as a library or
//! deployed as an independent service via `openhuman-memory-server`.
//!
//! # Feature flags
//!
//! - `__full_app` — enables modules that depend on the full OpenHuman
//!   application context (inference providers, orchestration, RPC schemas).
//!   Without this flag, only the core storage, scoring, and queue primitives
//!   are available.
//! - `__compile_store` — legacy alias, enables store submodules.

pub mod bridge;
pub mod config;
pub mod core_types;
pub mod queue;
pub mod rpc;
pub mod store;
pub mod tree;

// Modules gated behind __full_app — require orchestration layer + external deps.
#[cfg(feature = "__full_app")]
pub mod archivist;
#[cfg(feature = "__full_app")]
pub mod conversations;
#[cfg(feature = "__full_app")]
pub mod entities;
#[cfg(feature = "__full_app")]
pub mod graph;
#[cfg(feature = "__full_app")]
pub mod learning_full;
#[cfg(feature = "__full_app")]
pub mod sources;
#[cfg(feature = "__full_app")]
pub mod sync;
#[cfg(feature = "__full_app")]
pub mod tools_impl;
#[cfg(feature = "__full_app")]
pub mod orchestration;

// Stub learning module (standalone, lightweight).
#[cfg(not(feature = "__full_app"))]
pub mod learning;

// Re-export foundational crates
pub use openhuman_embeddings as embeddings;
pub use openhuman_memory_types as types;
