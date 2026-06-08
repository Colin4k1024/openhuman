//! Bridge module — defines traits and types that abstract over external
//! dependencies the memory system needs from the broader application.
//!
//! When running inside the full OpenHuman app, these are wired to the real
//! implementations. When running standalone, they can be stubbed or configured
//! with alternative implementations.

pub mod agent;
pub mod channels;
pub mod composio;
pub mod events;
pub mod inference;
pub mod integrations;
pub mod memory_traits;
pub mod people;
pub mod redact;
pub mod scheduler;
pub mod tools;
