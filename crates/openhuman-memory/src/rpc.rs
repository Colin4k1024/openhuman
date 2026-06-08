//! Lightweight RPC outcome type for standalone use.
//!
//! In the full app, the real `RpcOutcome` from `src/core/types.rs` is used.
//! This stub provides the same interface needed by memory domain handlers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Outcome of an RPC call — success with data + optional log messages.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RpcOutcome<T: Serialize> {
    pub data: T,
    pub logs: Vec<String>,
}

impl<T: Serialize> RpcOutcome<T> {
    pub fn ok(data: T) -> Result<Self, String> {
        Ok(Self {
            data,
            logs: Vec::new(),
        })
    }

    pub fn single_log(data: T, log: impl Into<String>) -> Self {
        Self {
            data,
            logs: vec![log.into()],
        }
    }

    pub fn with_logs(data: T, logs: Vec<String>) -> Self {
        Self { data, logs }
    }
}

impl RpcOutcome<Value> {
    pub fn empty_ok() -> Result<Self, String> {
        Ok(Self {
            data: Value::Null,
            logs: Vec::new(),
        })
    }
}
