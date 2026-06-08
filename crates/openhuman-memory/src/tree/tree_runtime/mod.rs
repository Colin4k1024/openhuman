//! Hierarchical time-based summary tree.
//!
//! Organizes summaries as a tree: root → year → month → day → hour (leaf).
//! Each hour, a background job drains buffered raw content, summarizes it into
//! the hour leaf, and propagates updated summaries upward through the tree.

#[cfg(feature = "__full_app")]
pub mod bus;
#[cfg(feature = "__full_app")]
pub(crate) mod cli;
pub mod engine;
pub mod ops;
pub mod store;
pub mod types;

#[cfg(feature = "__full_app")]
mod schemas;

pub use types::*;

#[cfg(feature = "__full_app")]
pub use ops as rpc;
#[cfg(feature = "__full_app")]
pub use schemas::{
    all_controller_schemas as all_tree_summarizer_controller_schemas,
    all_registered_controllers as all_tree_summarizer_registered_controllers,
};
