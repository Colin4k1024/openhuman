//! JSON-RPC dispatcher — single POST /rpc handler that routes by `method`.

pub mod documents;
pub mod entities;
pub mod graph;
pub mod health;
pub mod kv;
pub mod score;

use std::sync::Arc;

use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    error::{RpcError, RpcErrorResponse},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: Option<String>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub id: Value,
}

pub async fn rpc_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RpcRequest>,
) -> impl IntoResponse {
    tracing::debug!("[rpc] method={} id={:?}", req.method, req.id);

    let result = dispatch(&state, &req.method, req.params).await;

    match result {
        Ok(value) => Json(json!({
            "jsonrpc": "2.0",
            "result": value,
            "id": req.id
        }))
        .into_response(),
        Err(err) => RpcErrorResponse { error: err, id: req.id }.into_response(),
    }
}

async fn dispatch(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        // Health
        "memory.health" => health::handle_health(state, params).await,

        // Documents
        "memory.upsert_document" => documents::handle_upsert_document(state, params).await,
        "memory.list_documents" => documents::handle_list_documents(state, params).await,
        "memory.list_namespaces" => documents::handle_list_namespaces(state, params).await,
        "memory.clear_namespace" => documents::handle_clear_namespace(state, params).await,
        "memory.delete_document" => documents::handle_delete_document(state, params).await,

        // KV
        "memory.kv_set" => kv::handle_kv_set(state, params).await,
        "memory.kv_get" => kv::handle_kv_get(state, params).await,
        "memory.kv_delete" => kv::handle_kv_delete(state, params).await,

        // Score
        "memory.score_chunk" => score::handle_score_chunk(state, params).await,

        // Entities
        "memory.list_entities" => entities::handle_list_entities(state, params).await,
        "memory.get_entity" => entities::handle_get_entity(state, params).await,
        "memory.put_entity" => entities::handle_put_entity(state, params).await,

        // Graph
        "memory.graph_neighbors" => graph::handle_graph_neighbors(state, params).await,
        "memory.graph_node" => graph::handle_graph_node(state, params).await,
        "memory.graph_nodes" => graph::handle_graph_nodes(state, params).await,
        "memory.graph_co_occurring" => graph::handle_graph_co_occurring(state, params).await,
        "memory.graph_discover" => graph::handle_graph_discover(state, params).await,

        other => Err(RpcError::method_not_found(other)),
    }
}
