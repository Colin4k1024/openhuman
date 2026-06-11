//! Federation — cross-user pattern discovery with privacy preservation.
//!
//! Submodules:
//! - `local_patterns`: extract behavioral patterns from local memory.
//! - `privacy`: ε-differential privacy, PII filtering, generalization.

pub mod aggregation;
pub mod audit;
pub mod community;
pub mod local_patterns;
pub mod privacy;
