//! Graph RPC handlers — neighbors, edges, nodes.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::RpcError;
use crate::AppState;

use openhuman_memory::graph::{
    self, edges_from, edges_involving, get_node, list_nodes, upsert_node, GraphNode,
};

#[derive(Deserialize)]
struct NeighborsParams {
    node_id: String,
    #[serde(default = "default_limit")]
    limit: usize,
    relation_type: Option<String>,
}

#[derive(Deserialize)]
struct NodeParams {
    node_id: String,
}

#[derive(Deserialize)]
struct ListNodesParams {
    entity_type: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Deserialize)]
struct DiscoverParams {
    chunk_id: String,
    content: String,
    entities: Vec<EntityInput>,
}

#[derive(Deserialize)]
struct EntityInput {
    id: String,
    kind: String,
    surface: String,
}

fn default_limit() -> usize {
    50
}

/// memory.graph_neighbors — get edges from/involving a node.
pub async fn handle_graph_neighbors(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let p: NeighborsParams =
        serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

    let edges = if let Some(ref rt) = p.relation_type {
        edges_from(&state.config, &p.node_id, Some(rt), p.limit)
    } else {
        edges_involving(&state.config, &p.node_id, p.limit)
    }
    .map_err(|e| RpcError::internal(&e.to_string()))?;

    Ok(json!(edges))
}

/// memory.graph_node — get a single node.
pub async fn handle_graph_node(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let p: NodeParams =
        serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

    let node = get_node(&state.config, &p.node_id)
        .map_err(|e| RpcError::internal(&e.to_string()))?;

    match node {
        Some(n) => Ok(json!(n)),
        None => Err(RpcError::internal(&format!("node {} not found", p.node_id))),
    }
}

/// memory.graph_nodes — list nodes with optional type filter.
pub async fn handle_graph_nodes(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let p: ListNodesParams =
        serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

    let nodes = list_nodes(&state.config, p.entity_type.as_deref(), p.limit)
        .map_err(|e| RpcError::internal(&e.to_string()))?;

    Ok(json!(nodes))
}

/// memory.graph_co_occurring — derived co-occurrence from entity index.
pub async fn handle_graph_co_occurring(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let p: NeighborsParams =
        serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

    let edges = graph::co_occurring_entities(&state.config, &p.node_id, Some(p.limit))
        .map_err(|e| RpcError::internal(&e.to_string()))?;

    Ok(json!(edges))
}

/// memory.graph_discover — run discovery pipeline on a chunk and persist results.
pub async fn handle_graph_discover(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let p: DiscoverParams =
        serde_json::from_value(params).map_err(|e| RpcError::invalid_params(&e.to_string()))?;

    use openhuman_memory::tree::score::extract::EntityKind;
    use openhuman_memory::tree::score::resolver::CanonicalEntity;

    let entities: Vec<CanonicalEntity> = p
        .entities
        .iter()
        .map(|e| CanonicalEntity {
            canonical_id: e.id.clone(),
            kind: match e.kind.as_str() {
                "Person" => EntityKind::Person,
                "Organization" => EntityKind::Organization,
                "Location" => EntityKind::Location,
                "Product" => EntityKind::Product,
                _ => EntityKind::Technology, // fallback to a generic kind
            },
            surface: e.surface.clone(),
            span_start: 0,
            span_end: e.surface.len() as u32,
            score: 1.0,
        })
        .collect();

    let count = graph::discover_and_persist(&state.config, &p.chunk_id, &p.content, &entities, None)
        .map_err(|e| RpcError::internal(&e.to_string()))?;

    Ok(json!({ "relations_discovered": count }))
}
