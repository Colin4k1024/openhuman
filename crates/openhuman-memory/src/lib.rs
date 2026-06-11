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

// Standalone modules — no external app dependencies.
pub mod entities;
pub mod federation;
pub mod graph;
pub mod sync_protocol;

// Modules requiring orchestration, sync, or deep app integration.
#[cfg(feature = "__full_app")]
pub mod archivist;
#[cfg(feature = "__full_app")]
pub mod conversations;
#[cfg(feature = "__full_app")]
pub mod learning_full;
#[cfg(feature = "__full_app")]
pub mod orchestration;
#[cfg(feature = "__full_app")]
pub mod sources;
#[cfg(feature = "__full_app")]
pub mod sync;
#[cfg(feature = "__full_app")]
pub mod tools_impl;

// Stub learning module (standalone, lightweight).
#[cfg(not(feature = "__full_app"))]
pub mod learning;

// Re-export foundational crates
pub use openhuman_embeddings as embeddings;
pub use openhuman_memory_types as types;

/// Embedding extensions — factory functions for creating embedding providers.
///
/// In standalone mode, this wires directly to `openhuman_embeddings::create_provider`.
/// Supports Ollama (local), OpenAI, Cohere, Voyage, and any OpenAI-compatible endpoint.
pub mod embedding_ext;
