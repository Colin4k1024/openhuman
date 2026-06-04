//! Scoring module — will be populated in Phase 1.2 (memory_tree extraction).
//!
//! The submodules (embed, store, extract) are only compiled with `__full_app`.

#[cfg(feature = "__full_app")]
pub mod embed;
#[cfg(feature = "__full_app")]
pub mod store;
