//! Document namespace RPC handlers.
//!
//! Covers: `memory.upsert_document`, `memory.list_documents`,
//! `memory.list_namespaces`, `memory.clear_namespace`, `memory.delete_document`.

use serde_json::Value;

use openhuman_memory::store::types::NamespaceDocumentInput;

use crate::AppState;
use crate::error::RpcError;

pub async fn handle_upsert_document(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let input: NamespaceDocumentInput = serde_json::from_value(params)
        .map_err(|e| RpcError::invalid_params(format!("invalid upsert_document params: {e}")))?;
    let doc_id = state
        .memory
        .upsert_document(input)
        .await
        .map_err(RpcError::internal)?;
    tracing::debug!("[rpc] upsert_document doc_id={doc_id}");
    Ok(serde_json::json!({ "document_id": doc_id }))
}

pub async fn handle_list_documents(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let namespace: Option<String> = params
        .get("namespace")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let docs = state
        .memory
        .list_documents(namespace.as_deref())
        .await
        .map_err(RpcError::internal)?;
    tracing::debug!("[rpc] list_documents namespace={namespace:?}");
    Ok(docs)
}

pub async fn handle_list_namespaces(state: &AppState, _params: Value) -> Result<Value, RpcError> {
    let ns = state
        .memory
        .list_namespaces()
        .await
        .map_err(RpcError::internal)?;
    tracing::debug!("[rpc] list_namespaces count={}", ns.len());
    Ok(serde_json::json!(ns))
}

pub async fn handle_clear_namespace(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let namespace = params
        .get("namespace")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: namespace"))?;
    state
        .memory
        .clear_namespace(namespace)
        .await
        .map_err(RpcError::internal)?;
    tracing::debug!("[rpc] clear_namespace namespace={namespace}");
    Ok(serde_json::json!({ "cleared": true }))
}

pub async fn handle_delete_document(state: &AppState, params: Value) -> Result<Value, RpcError> {
    let namespace = params
        .get("namespace")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: namespace"))?;
    let document_id = params
        .get("document_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_params("missing field: document_id"))?;
    state
        .memory
        .delete_document(namespace, document_id)
        .await
        .map_err(RpcError::internal)?;
    tracing::debug!("[rpc] delete_document namespace={namespace} document_id={document_id}");
    Ok(serde_json::json!({ "deleted": true }))
}
