//! Entity RPC handlers.
//!
//! Covers: `memory.list_entities`, `memory.get_entity`, `memory.put_entity`.

use serde_json::Value;

use openhuman_memory::config::Config;
use openhuman_memory::entities::types::{Entity, EntityKind};
use openhuman_memory::entities::{get_entity, list_entities, put_entity};

use crate::AppState;
use crate::error::RpcError;

fn parse_kind(params: &Value) -> Result<EntityKind, RpcError> {
    let kind_str = params
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: kind"))?;
    EntityKind::parse(kind_str)
        .map_err(|e| RpcError::invalid_params(format!("invalid kind: {e}")))
}

fn make_config(state: &AppState) -> Config {
    let mut cfg = Config::default();
    cfg.workspace_dir = state.workspace_dir.clone();
    cfg
}

pub async fn handle_list_entities(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let kind = parse_kind(&params)?;
    let cfg = make_config(state);
    let entities = list_entities(&cfg, kind)
        .map_err(|e| RpcError::internal(format!("list_entities: {e}")))?;
    tracing::debug!("[rpc] list_entities kind={} count={}", kind.as_str(), entities.len());
    serde_json::to_value(entities).map_err(|e| RpcError::internal(format!("serialize: {e}")))
}

pub async fn handle_get_entity(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let kind = parse_kind(&params)?;
    let canonical_id = params
        .get("canonical_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: canonical_id"))?;
    let cfg = make_config(state);
    let entity = get_entity(&cfg, kind, canonical_id)
        .map_err(|e| RpcError::internal(format!("get_entity: {e}")))?;
    tracing::debug!("[rpc] get_entity kind={} id={canonical_id} found={}", kind.as_str(), entity.is_some());
    serde_json::to_value(entity).map_err(|e| RpcError::internal(format!("serialize: {e}")))
}

pub async fn handle_put_entity(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let entity: Entity = serde_json::from_value(params)
        .map_err(|e| RpcError::invalid_params(format!("invalid entity: {e}")))?;
    let cfg = make_config(state);
    let stored = put_entity(&cfg, entity)
        .map_err(|e| RpcError::internal(format!("put_entity: {e}")))?;
    tracing::debug!("[rpc] put_entity kind={} id={}", stored.kind.as_str(), stored.id);
    serde_json::to_value(stored).map_err(|e| RpcError::internal(format!("serialize: {e}")))
}
