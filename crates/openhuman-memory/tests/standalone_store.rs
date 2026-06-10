//! Integration test: standalone UnifiedMemory store — create, KV ops, document upsert.

use openhuman_memory::embedding_ext::default_embedding_provider;
use openhuman_memory::store::unified::UnifiedMemory;
use openhuman_memory::store::types::NamespaceDocumentInput;
use tempfile::TempDir;

#[tokio::test]
async fn unified_memory_kv_roundtrip() {
    let dir = TempDir::new().unwrap();
    let embedder = default_embedding_provider();

    let mem = UnifiedMemory::new(dir.path(), embedder, None)
        .expect("should create unified memory store");

    // KV set
    let val = serde_json::json!("world");
    mem.kv_set_global("hello", &val).await.unwrap();

    // KV get
    let result = mem.kv_get_global("hello").await.unwrap();
    assert_eq!(result, Some(val));

    // KV delete
    mem.kv_delete_global("hello").await.unwrap();
    let result = mem.kv_get_global("hello").await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn unified_memory_document_upsert() {
    let dir = TempDir::new().unwrap();
    let embedder = default_embedding_provider();

    let mem = UnifiedMemory::new(dir.path(), embedder, None)
        .expect("should create unified memory store");

    let input = NamespaceDocumentInput {
        namespace: "test-ns".into(),
        key: "doc-1".into(),
        title: "Rust Notes".into(),
        content: "Rust is a systems programming language focused on safety.".into(),
        source_type: "note".into(),
        priority: "normal".into(),
        tags: vec!["rust".into(), "programming".into()],
        metadata: serde_json::json!({}),
        category: "knowledge".into(),
        session_id: None,
        document_id: None,
        taint: Default::default(),
    };

    let id = mem.upsert_document(input).await.unwrap();
    assert!(!id.is_empty());

    // List namespaces should include our namespace
    let namespaces = mem.list_namespaces().await.unwrap();
    assert!(namespaces.contains(&"test-ns".to_string()));
}

#[tokio::test]
async fn unified_memory_namespace_kv() {
    let dir = TempDir::new().unwrap();
    let embedder = default_embedding_provider();

    let mem = UnifiedMemory::new(dir.path(), embedder, None)
        .expect("should create unified memory store");

    let val = serde_json::json!(42);
    mem.kv_set_namespace("my-ns", "counter", &val).await.unwrap();

    let result = mem.kv_get_namespace("my-ns", "counter").await.unwrap();
    assert_eq!(result, Some(val));
}
