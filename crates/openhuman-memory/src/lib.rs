//! OpenHuman Memory — standalone memory system.
//!
//! Provides storage, semantic retrieval, ingestion pipeline, and
//! summarization tree capabilities. Can be used as a library or
//! deployed as an independent service via `openhuman-memory-server`.

pub mod bridge;
pub mod config;
// Store module — imports rewritten, 176 type errors remain (bridge types need fleshing out).
// Enable with: pub mod store;
#[cfg(feature = "__compile_store")]
pub mod store;

// Re-export foundational crates
pub use openhuman_embeddings as embeddings;
pub use openhuman_memory_types as types;
