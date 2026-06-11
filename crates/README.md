# OpenHuman Memory System

Standalone, modular memory infrastructure extracted from OpenHuman.

Implements multi-phase persistent knowledge graph with document ingestion, semantic scoring, summary trees, and federation primitives. Ready to deploy as a library, HTTP microservice, Python package, or browser/edge module.

## Architecture

```
┌────────────────────────────────────────────────────────────────────┐
│ Integration Layers                                                 │
├────────────────────────────────────────────────────────────────────┤
│  openhuman-memory-server         HTTP JSON-RPC microservice        │
│  openhuman-memory-python         Python bindings + notebooks       │
│  openhuman-memory-wasm           Browser/edge preprocessing        │
└──────────┬─────────────────────────┬──────────────────────────────┘
           │ (JSON-RPC bridge)      │
           ▼                        ▼
┌────────────────────────────────────────────────────────────────────┐
│ openhuman-memory                  Full-feature knowledge graph    │
│ ├─ Document ingestion + scoring                                  │
│ ├─ Persistent summary trees                                      │
│ ├─ Graph operations (nodes, edges, traversal)                    │
│ ├─ Semantic reranking + federation                               │
│ └─ Event sourcing + changelog                                    │
├────────────────────────────────────────────────────────────────────┤
│ openhuman-embeddings              Multi-backend vector provider    │
│ ├─ OpenAI, Ollama, Cohere, Voyage                                │
│ └─ Custom OpenAI-compatible endpoints                            │
└──────────┬──────────────────────────┬──────────────────────────────┘
           │ (type bridge)           │
           ▼                         ▼
┌────────────────────────────────────────────────────────────────────┐
│ openhuman-memory-types            Shared types (serde + chrono)    │
│ ├─ Chunk, Metadata, SourceRef, DataSource, SourceKind             │
│ ├─ Tree, TreeKind, TreeStatus, SummaryNode, Buffer                │
│ └─ EntityIndexStats, HotnessCounters                              │
└────────────────────────────────────────────────────────────────────┘
```

## Crate Overview

| Crate | Purpose | Use Case |
|-------|---------|----------|
| **openhuman-memory-types** | Shared types | Type-safe contracts across crates |
| **openhuman-embeddings** | Vector embeddings | Semantic search for any text-to-vector provider |
| **openhuman-memory** | Full knowledge graph | Core library (Phase 1–4: ingest, score, summarize, retrieve) |
| **openhuman-memory-server** | HTTP microservice | Deploy as standalone JSON-RPC backend |
| **openhuman-memory-python** | Python bindings | Jupyter notebooks, data science pipelines |
| **openhuman-memory-wasm** | Browser module | Client-side preprocessing, edge computing |

## Choose Your Integration Path

### Rust Library (Full Feature Set)

Use `openhuman-memory` directly in your Rust project for complete control.

```bash
cargo add openhuman-memory
```

**Best for:** Rust applications, embedded systems, performance-critical deployments.

### HTTP Microservice (Any Language)

Deploy `openhuman-memory-server` as a standalone service—call it from any language via JSON-RPC.

```bash
cargo build -p openhuman-memory-server
./target/release/openhuman-memory-server --port 8420 --token secret
```

**Best for:** Polyglot teams, container orchestration (Kubernetes), cloud functions.

### Python (Jupyter, Pipelines)

Use `openhuman-memory-python` for data science and notebook workflows.

```bash
pip install openhuman-memory
```

**Best for:** Notebooks, ML pipelines, rapid prototyping.

### Browser / Edge (Preprocessing)

Use `openhuman-memory-wasm` for client-side chunking and preprocessing before sending to the server.

```bash
npm install openhuman-memory-wasm
```

**Best for:** Web apps, edge computing, offline-first applications.

## Quick Start (3 Minutes)

### 1. Build the Server

```bash
cd crates
cargo build -p openhuman-memory-server --release
```

### 2. Start the Service

```bash
./target/release/openhuman-memory-server \
  --port 8420 \
  --token secret \
  --db sqlite:///tmp/memory.db
```

### 3. Upsert a Document

```bash
curl -X POST http://localhost:8420/rpc \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer secret" \
  -d '{
    "jsonrpc": "2.0",
    "method": "memory.upsert_document",
    "params": {
      "source_kind": "document",
      "source_id": "doc:123",
      "owner": "user@example.com",
      "content": "Meeting notes: discussed Q3 roadmap and budget allocation.",
      "tags": ["meeting", "planning"]
    },
    "id": 1
  }'
```

### 4. Search

```bash
curl -X POST http://localhost:8420/rpc \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer secret" \
  -d '{
    "jsonrpc": "2.0",
    "method": "memory.search",
    "params": {
      "query": "Q3 budget",
      "limit": 5
    },
    "id": 2
  }'
```

## Feature Matrix

| Feature | Types | Embeddings | Memory | Server | Python | WASM |
|---------|:-----:|:----------:|:------:|:------:|:------:|:----:|
| Shared types | ✓ | — | ✓ | ✓ | ✓ | ✓ |
| Multi-backend embeddings | — | ✓ | ✓ | ✓ | ✓ | ✓ |
| Document ingest | — | — | ✓ | ✓ | ✓ | — |
| Semantic scoring | — | — | ✓ | ✓ | ✓ | — |
| Summary trees | — | — | ✓ | ✓ | ✓ | — |
| Graph queries | — | — | ✓ | ✓ | ✓ | — |
| Semantic retrieval | — | — | ✓ | ✓ | ✓ | — |
| Federation primitives | — | — | ✓ | ✓ | ✓ | — |
| HTTP JSON-RPC | — | — | — | ✓ | — | — |
| Python API | — | — | — | — | ✓ | — |
| WebAssembly | — | — | — | — | — | ✓ |

## Building

### Rust Crates

```bash
# Build all
cargo build --release

# Build individual crate
cargo build -p openhuman-memory-server --release

# Run tests
cargo test

# Check formatting
cargo fmt --check
cargo clippy -- -D warnings
```

### Python Bindings

```bash
# Build Python extension
cd crates/openhuman-memory-python
maturin develop

# Test
pytest
```

### WebAssembly

```bash
# Build WASM module
cd crates/openhuman-memory-wasm
wasm-pack build --target web
```

## License

MIT
