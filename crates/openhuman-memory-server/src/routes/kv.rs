//! Key-value RPC handlers.
//!
//! Covers: `memory.kv_set`, `memory.kv_get`, `memory.kv_delete`.
//!
//! Each method operates on the global KV store when no `namespace` field is
//! provided, and on the namespace-scoped KV store when `namespace` is present.

use serde_json::Value;

use crate::AppState;
use crate::error::RpcError;

pub async fn handle_kv_set(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let key = params
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: key"))?;
    let value = params
        .get("value")
        .ok_or_else(|| RpcError::invalid_params("missing field: value"))?;
    let namespace = params.get("namespace").and_then(|v| v.as_str());

    match namespace {
        Some(ns) => {
            state
                .memory
                .kv_set_namespace(ns, key, value)
                .await
                .map_err(RpcError::internal)?;
            tracing::debug!("[rpc] kv_set namespace={ns} key={key}");
        }
        None => {
            state
                .memory
                .kv_set_global(key, value)
                .await
                .map_err(RpcError::internal)?;
            tracing::debug!("[rpc] kv_set global key={key}");
        }
    }

    Ok(serde_json::json!({ "set": true }))
}

pub async fn handle_kv_get(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let key = params
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: key"))?;
    let namespace = params.get("namespace").and_then(|v| v.as_str());

    let result = match namespace {
        Some(ns) => {
            tracing::debug!("[rpc] kv_get namespace={ns} key={key}");
            state
                .memory
                .kv_get_namespace(ns, key)
                .await
                .map_err(RpcError::internal)?
        }
        None => {
            tracing::debug!("[rpc] kv_get global key={key}");
            state
                .memory
                .kv_get_global(key)
                .await
                .map_err(RpcError::internal)?
        }
    };

    Ok(serde_json::json!({ "value": result }))
}

pub async fn handle_kv_delete(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let key = params
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: key"))?;
    let namespace = params.get("namespace").and_then(|v| v.as_str());

    let deleted = match namespace {
        Some(ns) => {
            tracing::debug!("[rpc] kv_delete namespace={ns} key={key}");
            state
                .memory
                .kv_delete_namespace(ns, key)
                .await
                .map_err(RpcError::internal)?
        }
        None => {
            tracing::debug!("[rpc] kv_delete global key={key}");
            state
                .memory
                .kv_delete_global(key)
                .await
                .map_err(RpcError::internal)?
        }
    };

    Ok(serde_json::json!({ "deleted": deleted }))
}
