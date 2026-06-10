//! Integration tests for openhuman-memory-server.
//!
//! Each test starts a real TCP server on an OS-assigned port, exercises the
//! JSON-RPC surface via reqwest, and asserts the response shape.

use std::sync::Arc;

use openhuman_memory::{embedding_ext::default_embedding_provider, store::UnifiedMemory};
use openhuman_memory_server::{build_app, AppState};
use reqwest::Client;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::net::TcpListener;

const TOKEN: &str = "test-secret-token";

// ── helpers ──────────────────────────────────────────────────────────────────

/// Spin up a server on a random OS port. Returns `(base_url, TempDir)`.
/// The `TempDir` must be kept alive for the duration of the test.
async fn start_server() -> (String, TempDir) {
    let tmp = TempDir::new().expect("tempdir");

    let embedder = default_embedding_provider();
    let memory = UnifiedMemory::new(tmp.path(), embedder, None).expect("UnifiedMemory::new");

    let config = openhuman_memory::config::Config {
        workspace_dir: tmp.path().to_path_buf(),
        ..Default::default()
    };
    let state = Arc::new(AppState {
        memory,
        config,
        workspace_dir: tmp.path().to_path_buf(),
    });

    let app = build_app(state, TOKEN.to_string());

    // Port 0 → OS assigns a free ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("local_addr").port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    // Give the task a moment to start accepting connections.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    (format!("http://127.0.0.1:{port}"), tmp)
}

/// POST a JSON-RPC request with the test bearer token.
async fn rpc(client: &Client, base: &str, method: &str, params: Value) -> Value {
    client
        .post(format!("{base}/rpc"))
        .bearer_auth(TOKEN)
        .json(&json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": 1 }))
        .send()
        .await
        .expect("send")
        .json::<Value>()
        .await
        .expect("json")
}

// ── auth ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn auth_rejects_missing_token() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    let status = client
        .post(format!("{base}/rpc"))
        // No Authorization header.
        .json(&json!({ "jsonrpc": "2.0", "method": "memory.health", "params": {}, "id": 1 }))
        .send()
        .await
        .expect("send")
        .status();

    assert_eq!(status, 401, "missing token must return 401");
}

#[tokio::test]
async fn auth_rejects_wrong_token() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    let status = client
        .post(format!("{base}/rpc"))
        .bearer_auth("wrong-token")
        .json(&json!({ "jsonrpc": "2.0", "method": "memory.health", "params": {}, "id": 1 }))
        .send()
        .await
        .expect("send")
        .status();

    assert_eq!(status, 401, "wrong token must return 401");
}

// ── health ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_returns_ok() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    let resp = rpc(&client, &base, "memory.health", json!({})).await;

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["result"]["status"], "ok");
    assert!(
        resp["result"]["workspace"].is_string(),
        "workspace field must be a string"
    );
}

// ── documents ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn upsert_and_list_document() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    // Upsert a document.
    let resp = rpc(
        &client,
        &base,
        "memory.upsert_document",
        json!({
            "namespace": "test-ns",
            "key": "doc-1",
            "title": "Test Document",
            "content": "This is a test document with sufficient content for indexing.",
            "source_type": "note",
            "priority": "normal",
            "tags": ["test"],
            "metadata": {},
            "category": "core",
        }),
    )
    .await;

    assert!(resp.get("error").is_none(), "upsert should not error: {resp}");
    let doc_id = resp["result"]["document_id"].as_str().expect("document_id");
    assert!(!doc_id.is_empty(), "document_id must be non-empty");

    // List namespaces — test-ns must appear.
    let ns_resp = rpc(&client, &base, "memory.list_namespaces", json!({})).await;
    assert!(ns_resp.get("error").is_none(), "list_namespaces error: {ns_resp}");
    let namespaces: Vec<String> = serde_json::from_value(ns_resp["result"].clone())
        .expect("parse namespaces");
    assert!(
        namespaces.contains(&"test-ns".to_string()),
        "test-ns must appear in namespaces: {namespaces:?}"
    );

    // List documents in the namespace.
    let docs_resp = rpc(
        &client,
        &base,
        "memory.list_documents",
        json!({ "namespace": "test-ns" }),
    )
    .await;
    assert!(docs_resp.get("error").is_none(), "list_documents error: {docs_resp}");
    let docs = &docs_resp["result"];
    assert!(docs.is_array() || docs.is_object(), "result should be array or object");
}

// ── kv ───────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn kv_global_set_get_delete() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    // Set a global KV pair.
    let set_resp = rpc(
        &client,
        &base,
        "memory.kv_set",
        json!({ "key": "my-key", "value": { "answer": 42 } }),
    )
    .await;
    assert!(set_resp.get("error").is_none(), "kv_set error: {set_resp}");
    assert_eq!(set_resp["result"]["set"], true);

    // Get it back.
    let get_resp = rpc(
        &client,
        &base,
        "memory.kv_get",
        json!({ "key": "my-key" }),
    )
    .await;
    assert!(get_resp.get("error").is_none(), "kv_get error: {get_resp}");
    assert_eq!(get_resp["result"]["value"]["answer"], 42);

    // Delete it.
    let del_resp = rpc(
        &client,
        &base,
        "memory.kv_delete",
        json!({ "key": "my-key" }),
    )
    .await;
    assert!(del_resp.get("error").is_none(), "kv_delete error: {del_resp}");
    assert_eq!(del_resp["result"]["deleted"], true);

    // Get after delete — value should be null.
    let get2 = rpc(
        &client,
        &base,
        "memory.kv_get",
        json!({ "key": "my-key" }),
    )
    .await;
    assert!(get2.get("error").is_none(), "kv_get after delete error: {get2}");
    assert!(get2["result"]["value"].is_null(), "value should be null after delete");
}

#[tokio::test]
async fn kv_namespace_scoped_set_get() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    let set_resp = rpc(
        &client,
        &base,
        "memory.kv_set",
        json!({ "namespace": "prefs", "key": "theme", "value": "dark" }),
    )
    .await;
    assert!(set_resp.get("error").is_none(), "kv_set ns error: {set_resp}");

    let get_resp = rpc(
        &client,
        &base,
        "memory.kv_get",
        json!({ "namespace": "prefs", "key": "theme" }),
    )
    .await;
    assert!(get_resp.get("error").is_none(), "kv_get ns error: {get_resp}");
    assert_eq!(get_resp["result"]["value"], "dark");
}

// ── score_chunk ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn score_chunk_returns_result() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    let resp = rpc(
        &client,
        &base,
        "memory.score_chunk",
        json!({
            "text": "We decided to ship the Phoenix feature on Friday after reviewing the migration plan with alice@example.com. @bob will coordinate the launch.",
            "source_kind": "document"
        }),
    )
    .await;

    assert!(resp.get("error").is_none(), "score_chunk error: {resp}");
    let result = &resp["result"];
    assert!(result["chunk_id"].is_string(), "chunk_id must be string");
    assert!(result["total"].is_number(), "total must be a number");
    assert!(result["kept"].is_boolean(), "kept must be boolean");
}

// ── method not found ─────────────────────────────────────────────────────────

#[tokio::test]
async fn unknown_method_returns_error() {
    let (base, _tmp) = start_server().await;
    let client = Client::new();

    let resp = rpc(&client, &base, "memory.does_not_exist", json!({})).await;

    assert!(resp.get("error").is_some(), "unknown method must return error");
    assert_eq!(resp["error"]["code"], -32601);
}
