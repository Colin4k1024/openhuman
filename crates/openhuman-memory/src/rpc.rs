//! RPC outcome type (mirrors src/rpc/mod.rs).

use serde::Serialize;
use serde_json::json;

/// Outcome of an RPC call — success with data + optional log messages.
#[derive(Clone, Debug, Serialize)]
pub struct RpcOutcome<T> {
    pub value: T,
    pub logs: Vec<String>,
}

impl<T> RpcOutcome<T> {
    pub fn new(value: T, logs: Vec<String>) -> Self {
        Self { value, logs }
    }
}

impl<T: Serialize> RpcOutcome<T> {
    pub fn single_log(value: T, log: impl Into<String>) -> Self {
        Self {
            value,
            logs: vec![log.into()],
        }
    }

    pub fn ok(value: T) -> Result<Self, String> {
        Ok(Self {
            value,
            logs: Vec::new(),
        })
    }

    pub fn with_logs(value: T, logs: Vec<String>) -> Self {
        Self { value, logs }
    }

    pub fn into_cli_compatible_json(self) -> Result<serde_json::Value, String> {
        let RpcOutcome { value, logs } = self;
        let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
        if logs.is_empty() {
            Ok(value)
        } else {
            Ok(json!({ "result": value, "logs": logs }))
        }
    }
}
