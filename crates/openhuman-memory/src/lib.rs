//! OpenHuman Memory — standalone memory system.
//!
//! Provides storage, semantic retrieval, ingestion pipeline, and
//! summarization tree capabilities. Can be used as a library or
//! deployed as an independent service via `openhuman-memory-server`.

pub mod bridge;
pub mod config;
// pub mod store;  // TODO: enable once imports are fully resolved

// Re-export foundational crates
pub use openhuman_embeddings as embeddings;
pub use openhuman_memory_types as types;
