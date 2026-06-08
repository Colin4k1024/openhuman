//! Memory tree — generic summary-tree engine.
//!
//! This module provides the core tree mechanics: bucket-seal cascades,
//! scoring, embedding, entity extraction, retrieval, and summarisation.
//!
//! In standalone mode (no `__full_app` feature), only the scoring framework,
//! I/O types, health diagnostics, and utility code are available. The full
//! tree runtime, summarisation, retrieval, and ingestion require the
//! `__full_app` feature which is activated when linked from the main crate.

pub mod score;
pub mod util;

#[cfg(feature = "__full_app")]
pub mod health;
#[cfg(feature = "__full_app")]
pub mod io;

// Heavy modules that depend on the full inference/orchestration stack.
#[cfg(feature = "__full_app")]
pub mod ingest;
#[cfg(feature = "__full_app")]
pub mod retrieval;
#[cfg(feature = "__full_app")]
pub mod summarise;
#[cfg(feature = "__full_app")]
pub mod tools;
#[cfg(feature = "__full_app")]
pub mod tree;
#[cfg(feature = "__full_app")]
pub mod tree_runtime;

#[cfg(feature = "__full_app")]
pub use io::{
    TreeLabelStrategy, TreeLeafPayload, TreeReadHit, TreeReadRequest, TreeReadResult,
    TreeWriteOutcome, TreeWriteRequest,
};
