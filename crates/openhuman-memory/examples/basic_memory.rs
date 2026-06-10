//! Basic example: create a memory store, insert documents, and query.
//!
//! Run with: `cargo run --example basic_memory`

use openhuman_memory::config::Config;
use openhuman_memory::embedding_ext::{
    create_embedding_provider_from_config, default_embedding_provider,
};
use openhuman_memory::store::types::NamespaceDocumentInput;
use openhuman_memory::store::unified::UnifiedMemory;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a config (uses defaults — Ollama localhost)
    let workspace = PathBuf::from("/tmp/openhuman-memory-example");
    std::fs::create_dir_all(&workspace)?;

    let config = Config {
        workspace_dir: workspace.clone(),
        ..Config::default()
    };

    // Create embedding provider from config, or fall back to default (noop)
    let embedder = create_embedding_provider_from_config(&config)
        .map(std::sync::Arc::from)
        .unwrap_or_else(|_| default_embedding_provider());

    // Open unified memory store
    let mem = UnifiedMemory::new(&workspace, embedder, None)?;

    println!("Memory store created at: {}", workspace.display());

    // Insert some documents
    let docs = vec![
        ("rust-intro", "Rust Programming", "Rust is a systems programming language focused on safety, speed, and concurrency."),
        ("python-intro", "Python Programming", "Python is a high-level programming language known for its readability and versatility."),
        ("go-intro", "Go Programming", "Go is a statically typed, compiled language designed at Google for simplicity and efficiency."),
    ];

    for (key, title, content) in docs {
        let input = NamespaceDocumentInput {
            namespace: "programming".into(),
            key: key.into(),
            title: title.into(),
            content: content.into(),
            source_type: "note".into(),
            priority: "normal".into(),
            tags: vec!["programming".into()],
            metadata: serde_json::json!({}),
            category: "knowledge".into(),
            session_id: None,
            document_id: None,
            taint: Default::default(),
        };
        let id = mem.upsert_document(input).await.map_err(|e| anyhow::anyhow!(e))?;
        println!("  Inserted: {} -> {}", key, id);
    }

    // KV operations
    mem.kv_set_global("last_run", &serde_json::json!(chrono::Utc::now().to_rfc3339()))
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let last_run = mem
        .kv_get_global("last_run")
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    println!("\n  KV last_run = {:?}", last_run);

    // List namespaces
    let namespaces = mem
        .list_namespaces()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    println!("\n  Namespaces: {:?}", namespaces);

    println!("\nDone! Memory store is ready for use.");
    Ok(())
}
