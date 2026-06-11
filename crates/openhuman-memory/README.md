# openhuman-memory

Standalone memory system extracted from the OpenHuman core. Provides storage, semantic retrieval, scoring, entity extraction, and job pipeline capabilities that can be used independently or embedded in other applications.

## Architecture

```
openhuman-memory/
├── src/
│   ├── bridge/          # Trait abstractions for external dependencies
│   ├── config.rs        # Standalone configuration
│   ├── core_types.rs    # RPC controller type stubs
│   ├── rpc.rs           # RpcOutcome type
│   ├── store/           # SQLite storage layer
│   │   ├── chunks/      # Text chunk storage + embeddings
│   │   ├── content/     # Raw content, obsidian, atomic summaries
│   │   ├── entities.rs  # Entity store
│   │   ├── kinds.rs     # Memory kind catalog
│   │   ├── kv.rs        # Key-value store
│   │   ├── trees/       # Summary tree persistence
│   │   ├── types.rs     # Shared types (NamespaceDocumentInput, etc.)
│   │   ├── unified/     # UnifiedMemory (FTS5, profile, segments, events, graph)
│   │   └── vectors/     # Vector embedding storage
│   ├── tree/
│   │   ├── score/       # Scoring & admission pipeline
│   │   │   ├── embed/   # Embedding providers (Ollama, Inert)
│   │   │   ├── extract/ # Entity extraction (regex, LLM)
│   │   │   ├── signals/ # Score signal computation
│   │   │   ├── resolver.rs  # Entity canonicalization
│   │   │   └── store.rs     # Score persistence
│   │   └── util.rs      # String utilities
│   ├── queue/           # Async job pipeline
│   │   ├── store.rs     # SQLite job queue
│   │   ├── types.rs     # Job types (Seal, Extract, Append, etc.)
│   │   ├── scheduler.rs # Daily/periodic job scheduler
│   │   └── health_types.rs  # Failure classification
│   └── embedding_ext/   # Embedding factory stubs
```

## Quick Start

### Add to your Cargo.toml

```toml
[dependencies]
openhuman-memory = { path = "crates/openhuman-memory" }
```

### Initialize Storage

```rust
use openhuman_memory::config::Config;
use openhuman_memory::store::chunks::store::with_connection;

fn main() -> anyhow::Result<()> {
    // Use default config (stores in ~/.openhuman/memory/)
    let config = Config::default();

    // Or configure a custom data directory
    let config = Config {
        workspace_dir: "/path/to/data".into(),
        ..Default::default()
    };

    // Access the SQLite connection
    with_connection(&config, |conn| {
        // Tables are auto-created on first access
        println!("Memory DB ready");
        Ok(())
    })?;

    Ok(())
}
```

### Store and Retrieve Chunks

```rust
use chrono::Utc;
use openhuman_memory::store::chunks::types::{Chunk, Metadata, SourceKind};

fn make_chunk(text: &str, source_id: &str) -> Chunk {
    let now = Utc::now();
    Chunk {
        id: format!("chunk-{}", uuid::Uuid::new_v4()),
        content: text.to_string(),
        metadata: Metadata {
            source_kind: SourceKind::Chat,
            source_id: source_id.to_string(),
            owner: String::new(),
            timestamp: now,
            time_range: (now, now),
            tags: vec![],
            source_ref: None,
            path_scope: None,
        },
        token_count: (text.len() / 4) as u32,
        seq_in_source: 0,
        created_at: now,
        partial_message: false,
    }
}
```

### Score a Chunk (Admission Pipeline)

```rust
use openhuman_memory::tree::score::{score_chunk, ScoringConfig};
use openhuman_memory::store::chunks::types::Chunk;

async fn score_example(chunk: &Chunk) -> anyhow::Result<()> {
    let scoring_cfg = ScoringConfig::default_regex_only();
    let result = score_chunk(chunk, &scoring_cfg).await?;

    println!("Score: {:.2} (kept={})", result.total, result.kept);
    println!("Entities: {:?}", result.extracted.entities);
    println!("Signals: {:?}", result.signals);

    if !result.kept {
        println!("Dropped: {:?}", result.drop_reason);
    }

    Ok(())
}
```

### Use the Job Queue

```rust
use openhuman_memory::config::Config;
use openhuman_memory::queue::store::{enqueue, count_by_status};
use openhuman_memory::queue::types::{NewJob, JobStatus, SealPayload};

fn enqueue_seal_job(config: &Config, tree_id: &str, level: u32) -> anyhow::Result<()> {
    let payload = SealPayload {
        tree_id: tree_id.to_string(),
        level,
        source_id: None,
        force_now_ms: None,
    };

    let job = NewJob::seal(&payload)?;
    let job_id = enqueue(config, &job)?;
    println!("Enqueued seal job: {:?}", job_id);

    // Check queue status
    let ready = count_by_status(config, &JobStatus::Ready)?;
    println!("Jobs ready: {}", ready);

    Ok(())
}
```

### Embedding & Similarity Search

```rust
use openhuman_memory::tree::score::embed::{
    Embedder, OllamaEmbedder, InertEmbedder,
    cosine_similarity, pack_embedding, unpack_embedding,
    EMBEDDING_DIM,
};

async fn embed_example() -> anyhow::Result<()> {
    // Use Ollama for real embeddings
    let embedder = OllamaEmbedder::new("http://127.0.0.1:11434", "bge-m3");

    let vec_a = embedder.embed("Hello world").await?;
    let vec_b = embedder.embed("Hi there").await?;

    let similarity = cosine_similarity(&vec_a, &vec_b);
    println!("Similarity: {:.4}", similarity);

    // Pack for SQLite storage
    let blob = pack_embedding(&vec_a);
    let restored = unpack_embedding(&blob)?;
    assert_eq!(vec_a, restored);

    Ok(())
}

// For tests, use InertEmbedder (deterministic, no network)
fn test_embedder() -> Box<dyn Embedder> {
    Box::new(InertEmbedder)
}
```

### Entity Extraction

```rust
use openhuman_memory::tree::score::extract::{
    CompositeExtractor, EntityExtractor, RegexEntityExtractor,
};

async fn extract_entities(text: &str) -> anyhow::Result<()> {
    let extractor = CompositeExtractor::regex_only();
    let entities = extractor.extract(text).await?;

    for entity in &entities.entities {
        println!("{}: {} (kind={:?})", entity.surface, entity.canonical, entity.kind);
    }

    for topic in &entities.topics {
        println!("Topic: {}", topic.label);
    }

    Ok(())
}
```

### UnifiedMemory (Full-Text Search + Namespaces)

```rust
use openhuman_memory::config::Config;
use openhuman_memory::store::unified::UnifiedMemory;

fn fts_example(config: &Config) -> anyhow::Result<()> {
    let memory = UnifiedMemory::open(config)?;

    // Store a document
    memory.upsert_document(
        "notes",           // namespace
        "doc-001",         // document ID
        "Meeting notes from the architecture review...",
        None,              // metadata
    )?;

    // Full-text search
    let results = memory.search_fts("architecture review", Some("notes"), 10)?;
    for hit in results {
        println!("[{:.2}] {}: {}", hit.score, hit.doc_id, &hit.text[..80]);
    }

    Ok(())
}
```

### Key-Value Store

```rust
use openhuman_memory::config::Config;
use openhuman_memory::store::kv;

fn kv_example(config: &Config) -> anyhow::Result<()> {
    // Set
    kv::set(config, "user:preference:theme", "dark")?;

    // Get
    if let Some(value) = kv::get(config, "user:preference:theme")? {
        println!("Theme: {}", value);
    }

    // Delete
    kv::delete(config, "user:preference:theme")?;

    Ok(())
}
```

## Configuration

```rust
use openhuman_memory::config::{Config, MemoryConfig, MemoryTreeConfig, LocalAiConfig};
use std::path::PathBuf;

let config = Config {
    memory: MemoryConfig {
        backend: "sqlite".into(),
        embedding_provider: "ollama".into(),
        embedding_model: "bge-m3".into(),
        embedding_dimensions: 1024,
        data_dir: PathBuf::from("/custom/data/path"),
        ..Default::default()
    },
    memory_tree: MemoryTreeConfig {
        enabled: true,
        ..Default::default()
    },
    local_ai: LocalAiConfig {
        ollama_base_url: Some("http://127.0.0.1:11434".into()),
        chat_model_id: "qwen2.5:0.5b".into(),
        runtime_enabled: true,
        ..Default::default()
    },
    workspace_dir: PathBuf::from("/custom/workspace"),
    ..Default::default()
};
```

### Environment Variables (when using `Config::default()`)

| Variable | Default | Description |
|----------|---------|-------------|
| — | `~/.openhuman/memory/` | Data directory for SQLite DBs |

## Bridge Traits (Dependency Injection)

The crate uses trait abstractions for external dependencies. When running standalone, no-op/stub implementations are used. When embedded in the full OpenHuman app, real implementations are injected.

| Bridge | Purpose | Standalone Behavior |
|--------|---------|-------------------|
| `bridge::inference::ChatProvider` | LLM completions | Returns error |
| `bridge::inference::InferenceProvider` | Simple completions | Returns error |
| `bridge::scheduler::SchedulerGate` | Rate limiting | Always permits |
| `bridge::events::EventPublisher` | Domain events | No-op |
| `bridge::composio::ComposioClient` | External integrations | Returns error |
| `bridge::integrations::IntegrationClient` | HTTP clients | Returns error |
| `bridge::tools::Tool` | Agent tool trait | — |
| `bridge::agent::PostTurnHook` | Post-turn callbacks | — |

### Implementing a Custom ChatProvider

```rust
use async_trait::async_trait;
use openhuman_memory::bridge::inference::{ChatProvider, ChatPrompt, ChatResponse};

struct MyProvider { /* ... */ }

#[async_trait]
impl ChatProvider for MyProvider {
    fn name(&self) -> &str { "my-provider" }

    async fn complete_chat(&self, prompt: &ChatPrompt) -> anyhow::Result<ChatResponse> {
        // Call your LLM here
        let text = call_my_llm(&prompt.system, &prompt.user).await?;
        Ok(ChatResponse { content: text, usage: None })
    }
}
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| (none) | Core storage, scoring, queue — compiles standalone |
| `__compile_store` | Legacy alias |
| `__full_app` | Enables orchestration, sync, learning, sources (requires full app context) |

## Module Reference

### Always Available (no features)

| Module | Description |
|--------|-------------|
| `store::chunks` | Text chunk CRUD, embedding blobs, connection pool |
| `store::unified` | UnifiedMemory with FTS5, profile, segments, graph |
| `store::content` | Raw content paths, obsidian format, atomic summaries |
| `store::trees` | Summary tree node persistence |
| `store::vectors` | Vector embedding store |
| `store::kv` | Key-value store |
| `store::kinds` | `MemoryKind` enum catalog |
| `store::entities` | Entity store |
| `store::types` | Shared types |
| `tree::score` | Full scoring pipeline (signals, extraction, embeddings) |
| `tree::score::embed` | Embedder trait + Ollama/Inert impls |
| `tree::score::extract` | Entity extraction (regex + optional LLM) |
| `tree::score::signals` | Score signal computation |
| `tree::score::resolver` | Entity canonicalization |
| `tree::util` | String utilities (char boundary helpers) |
| `queue` | Job pipeline (store, types, scheduler, testing) |
| `config` | Configuration types + global accessor |
| `rpc` | RpcOutcome type |
| `bridge` | All bridge traits |

### Requires `__full_app`

| Module | Description |
|--------|-------------|
| `tree::{summarise, tree, tree_runtime, health, io, ingest, retrieval, tools}` | Full tree engine |
| `orchestration` | Memory routing, ingest pipeline, query, sync |
| `sources` | Source readers (GitHub, RSS, web, folder) |
| `sync` | Composio provider sync, canonicalization |
| `conversations` | Conversation memory + inverted index |
| `learning_full` | Reflection, learning hooks, profile building |
| `archivist` | Archival operations |
| `tools_impl` | Agent tool implementations |
| `entities` | Entity domain |
| `graph` | Graph queries |

## Testing

```bash
# Run standalone tests
cd crates/openhuman-memory
cargo test

# Check compilation
cargo check
cargo check --features __full_app  # (requires full app deps)
```

## Knowledge Graph (`graph`)

The graph module provides a persistent knowledge graph with derived and explicit relations.

### Persistent Store

```rust
use openhuman_memory::graph::{upsert_node, upsert_edge, edges_from, GraphNode, GraphEdgePersistent};

let node = GraphNode {
    node_id: "person:alice".into(),
    entity_type: "Person".into(),
    label: "Alice".into(),
    properties: serde_json::json!({"role": "engineer"}),
    created_at: Utc::now(),
    updated_at: Utc::now(),
};
upsert_node(&config, &node)?;
```

### Auto-Discovery

```rust
use openhuman_memory::graph::discover_and_persist;
// Discovers co-occurrence, temporal, and pattern-based relations from chunks
let count = discover_and_persist(&config, "chunk-1", "Alice works at Google", &entities, None)?;
```

### Graph Embeddings (TransE)

```rust
use openhuman_memory::graph::{train_embeddings, nearest_nodes, TransEConfig};

let result = train_embeddings(&config, &TransEConfig { dimension: 64, epochs: 100, ..Default::default() })?;
let similar = nearest_nodes(&config, "person:alice", 10)?;
```

### Temporal Graph

```rust
use openhuman_memory::graph::{most_active_edges, apply_decay, export_dot, DecayConfig};

let active = most_active_edges(&config, Utc::now() - Duration::days(30), 20)?;
let decay_result = apply_decay(&config, &DecayConfig::default())?;
let dot_export = export_dot(&config)?;
```

## Sync Protocol (`sync_protocol`)

Distributed memory synchronization primitives for multi-device scenarios.

| Module | Purpose |
|--------|---------|
| `changelog` | Lamport-stamped change log for every mutation |
| `device` | Per-device X25519 identity + mDNS discovery |
| `transport` | `SyncTransport` trait (LAN/Relay/Direct) |
| `merge` | CRDT conflict resolution (LWW, union, idempotent) |
| `crypto` | XChaCha20-Poly1305 E2E encryption (via `chacha20poly1305` + `x25519-dalek`) |

```rust
use openhuman_memory::sync_protocol::changelog::{record_change, get_changes_since, ChangePayload, OpType};
use openhuman_memory::sync_protocol::device::get_or_create_identity;
use openhuman_memory::sync_protocol::crypto::{encrypt, decrypt, derive_session_key};

// Record a change
let entry = record_change(&config, "device-a", OpType::Insert, &ChangePayload {
    store: "kv".into(), key: "greeting".into(), namespace: None, value: Some(json!("hello")),
})?;

// Pull changes for sync
let changes = get_changes_since(&config, 0)?;

// Encrypt for transit
let key = derive_session_key(&shared_secret, b"session-1");
let sealed = encrypt(&key, &serde_json::to_vec(&changes)?);
```

## Federation (`federation`)

Cross-user pattern discovery with privacy preservation.

| Module | Purpose |
|--------|---------|
| `local_patterns` | Extract behavioral patterns (temporal, preference, knowledge) |
| `privacy` | ε-differential privacy, PII generalization, budget tracking |
| `aggregation` | Additive secret sharing for secure aggregation |
| `community` | Group patterns, cold-start profiles, user segmentation |
| `audit` | Contribution tracking, opt-out, GDPR erasure |

```rust
use openhuman_memory::federation::local_patterns::{observe_pattern, PatternObservation, PatternType};
use openhuman_memory::federation::privacy::{sanitize_patterns, PrivacyConfig};
use openhuman_memory::federation::audit::{compliance_summary, gdpr_erase};

// Observe a pattern
observe_pattern(&config, &PatternObservation {
    pattern_type: PatternType::Temporal,
    trigger_context: "after standup".into(),
    action_taken: "checks email".into(),
    metadata: None,
})?;

// Sanitize for sharing (adds noise, strips PII)
let sanitized = sanitize_patterns(&config, &patterns, &PrivacyConfig::default())?;

// GDPR erasure
let result = gdpr_erase(&config)?;
```

## License

MIT
