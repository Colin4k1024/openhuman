//! Sync protocol — distributed memory synchronization primitives.
//!
//! Submodules:
//! - `changelog`: Lamport-stamped change entries for every memory mutation.
//! - `device`: Per-device identity, key generation, and discovery.

pub mod changelog;
pub mod crypto;
pub mod device;
pub mod merge;
pub mod transport;
