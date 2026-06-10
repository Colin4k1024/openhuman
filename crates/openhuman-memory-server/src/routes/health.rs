//! `memory.health` — liveness probe.

use serde_json::{json, Value};

use crate::AppState;
use crate::error::RpcError;

/// Returns `{"status":"ok","workspace":"<path>"}`.
pub async fn handle_health(state: &AppState, _params: Value) -> Result<Value, RpcError> {
    let workspace = state
        .memory
        .workspace_dir()
        .to_string_lossy()
        .to_string();
    Ok(json!({ "status": "ok", "workspace": workspace }))
}
