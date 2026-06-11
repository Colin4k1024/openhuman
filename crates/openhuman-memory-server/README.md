# openhuman-memory-server

Standalone HTTP/JSON-RPC server for the OpenHuman memory system. Provides document storage, key-value persistence, semantic scoring, entity extraction, and knowledge graph operations.

## Quick Start

Build and run the server:

```bash
cargo build --release -p openhuman-memory-server
./target/release/openhuman-memory-server \
  --port 8420 \
  --token mysecrettoken \
  --workspace ~/.openhuman-memory-server
```

Or use environment variables:

```bash
export MEMORY_SERVER_TOKEN=mysecrettoken
export RUST_LOG=openhuman_memory_server=debug
./target/release/openhuman-memory-server --port 8420 --workspace ~/.openhuman-memory-server
```

## Authentication

All requests require Bearer token authentication in the `Authorization` header:

```
Authorization: Bearer <your-token>
```

Without this header, the server returns HTTP 401 Unauthorized.

## Configuration

| Flag | Env Var | Default | Description |
|------|---------|---------|-------------|
| `--port` | — | `8420` | TCP port to listen on |
| `--token` | `MEMORY_SERVER_TOKEN` | (required) | Bearer token for API authentication |
| `--workspace` | — | `~/.openhuman-memory-server` | Workspace directory for storage |

## API Reference

All endpoints use JSON-RPC 2.0 POST to `/rpc` with the request format:

```json
{
  "jsonrpc": "2.0",
  "method": "memory.method_name",
  "params": { /* method params */ },
  "id": 1
}
```

Success response:

```json
{
  "jsonrpc": "2.0",
  "result": { /* result data */ },
  "id": 1
}
```

Error response:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32600,
    "message": "Invalid Request"
  },
  "id": 1
}
```

### Health

#### `memory.health`

Check server status and workspace info.

**Params:** (none)

**Response:**
```json
{
  "status": "ok",
  "workspace": "/home/user/.openhuman-memory-server",
  "timestamp": "2024-06-11T12:00:00Z"
}
```

### Documents

#### `memory.upsert_document`

Insert or update a document in a namespace.

**Params:**
```json
{
  "namespace": "github",
  "key": "issue-123",
  "title": "Bug in auth flow",
  "content": "User reports login timeout...",
  "source_type": "github",
  "priority": "high",
  "tags": ["bug", "auth"],
  "metadata": { "issue_id": "123" }
}
```

**Response:** Document ID (string)

#### `memory.list_documents`

List documents in a namespace or all namespaces.

**Params:**
```json
{
  "namespace": "github"  /* optional; if omitted, lists all */
}
```

**Response:** JSON array of documents with metadata

#### `memory.list_namespaces`

List all namespaces with at least one document.

**Params:** (none)

**Response:** JSON array of namespace names (strings)

#### `memory.clear_namespace`

Delete all documents in a namespace.

**Params:**
```json
{
  "namespace": "github"
}
```

**Response:** Confirmation (null)

#### `memory.delete_document`

Delete a specific document.

**Params:**
```json
{
  "namespace": "github",
  "document_id": "doc-uuid-123"
}
```

**Response:** Confirmation (null)

### Key-Value Store

#### `memory.kv_set`

Store or update a global key-value pair. Value must be JSON-serializable.

**Params:**
```json
{
  "key": "user_preference_theme",
  "value": "dark",
  "namespace": "settings"  /* optional */
}
```

**Response:** Confirmation (null)

#### `memory.kv_get`

Retrieve a global key-value pair.

**Params:**
```json
{
  "key": "user_preference_theme",
  "namespace": "settings"  /* optional */
}
```

**Response:** JSON value or null if not found

#### `memory.kv_delete`

Delete a global key-value pair.

**Params:**
```json
{
  "key": "user_preference_theme",
  "namespace": "settings"  /* optional */
}
```

**Response:** Confirmation (null)

### Scoring & Extraction

#### `memory.score_chunk`

Score a text chunk for admission to long-term memory. Returns signal scores (0-1) and extracted entities.

**Params:**
```json
{
  "text": "Meeting notes from Q2 planning...",
  "source_kind": "document"  /* optional: "chat", "email", "document" */
}
```

**Response:**
```json
{
  "chunk_id": "chunk-uuid",
  "total": 0.75,
  "kept": true,
  "drop_reason": null,
  "entities": [
    { "text": "2024-06-15", "kind": "Date" },
    { "text": "alice@example.com", "kind": "Email" }
  ]
}
```

### Entities

#### `memory.list_entities`

List all entities of a given kind.

**Params:**
```json
{
  "kind": "Person"
}
```

**Response:** JSON array of entities

#### `memory.get_entity`

Retrieve a specific entity by kind and canonical ID.

**Params:**
```json
{
  "kind": "Person",
  "canonical_id": "alice@example.com"
}
```

**Response:** Entity object with metadata

#### `memory.put_entity`

Insert or update an entity.

**Params:**
```json
{
  "kind": "Person",
  "canonical_id": "alice@example.com",
  "display_name": "Alice Smith",
  "attributes": { "role": "engineer" }
}
```

**Response:** Confirmation (null)

### Knowledge Graph

#### `memory.graph_node`

Get a single node by ID.

**Params:**
```json
{
  "node_id": "entity-123"
}
```

**Response:** Node object with edges

#### `memory.graph_nodes`

List graph nodes, optionally filtered by entity type.

**Params:**
```json
{
  "entity_type": "Person",
  "limit": 100
}
```

**Response:** JSON array of nodes

#### `memory.graph_neighbors`

Find neighbors of a node via specified relation types.

**Params:**
```json
{
  "node_id": "entity-123",
  "relation_type": "mentions",
  "limit": 50
}
```

**Response:** JSON array of neighbor nodes with edge weights

#### `memory.graph_co_occurring`

Find entities that co-occur frequently with a given node.

**Params:**
```json
{
  "node_id": "entity-123",
  "limit": 20
}
```

**Response:** JSON array of co-occurring entities

#### `memory.graph_discover`

Discover related entities from a text chunk and optional seed entities.

**Params:**
```json
{
  "chunk_id": "chunk-123",
  "content": "Alice met Bob at the conference...",
  "entities": [
    { "text": "Alice", "kind": "Person" },
    { "text": "Bob", "kind": "Person" }
  ]
}
```

**Response:** JSON array of discovered entities and relations

#### `memory.graph_train_embeddings`

Train node embeddings using a graph-based method (e.g., DeepWalk, Node2Vec).

**Params:**
```json
{
  "dimension": 128,
  "epochs": 10
}
```

**Response:**
```json
{
  "status": "training_complete",
  "nodes_embedded": 1234,
  "dimension": 128
}
```

#### `memory.graph_nearest`

Find nearest nodes to a target node in embedding space.

**Params:**
```json
{
  "node_id": "entity-123",
  "limit": 10
}
```

**Response:** JSON array of nearest neighbors with similarity scores

#### `memory.graph_active_edges`

List edges that were active (touched) within the last N days.

**Params:**
```json
{
  "days": 7,
  "limit": 100
}
```

**Response:** JSON array of edges with activity timestamps

#### `memory.graph_decay`

Apply temporal decay to edge weights. Edges below `min_weight` are pruned.

**Params:**
```json
{
  "half_life_days": 30,
  "min_weight": 0.1
}
```

**Response:**
```json
{
  "status": "decay_applied",
  "edges_pruned": 45,
  "edges_remaining": 1200
}
```

#### `memory.graph_export`

Export the full graph in a specific format.

**Params:**
```json
{
  "format": "json"  /* or "dot" for Graphviz */
}
```

**Response:** Graph data as JSON or DOT notation

## Error Codes

JSON-RPC 2.0 error codes:

| Code | Message | Meaning |
|------|---------|---------|
| `-32700` | Parse error | Invalid JSON in request |
| `-32600` | Invalid Request | Missing required fields |
| `-32601` | Method not found | Unknown method name |
| `-32602` | Invalid params | Parameter validation failed |
| `-32603` | Internal error | Server error (check logs) |
| `-32000` to `-32099` | Server error | Custom server errors |

## Docker

Build and run in Docker:

```dockerfile
FROM rust:1.80 as builder
WORKDIR /build
COPY . .
RUN cargo build --release -p openhuman-memory-server

FROM debian:bookworm-slim
COPY --from=builder /build/target/release/openhuman-memory-server /usr/local/bin/
EXPOSE 8420
CMD ["openhuman-memory-server", "--port", "8420", "--workspace", "/data/memory"]
```

```bash
docker build -t openhuman-memory-server .
docker run -e MEMORY_SERVER_TOKEN=secret -p 8420:8420 -v memory-data:/data/memory openhuman-memory-server
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `MEMORY_SERVER_TOKEN` | Bearer token (required if not passed via `--token`) |
| `RUST_LOG` | Logging level (`debug`, `info`, `warn`, `error`). Default: `openhuman_memory_server=debug,info` |
| `HOME` | Used to expand `~` in workspace path |

Example:

```bash
export RUST_LOG=openhuman_memory_server=trace
export MEMORY_SERVER_TOKEN=production-secret
./target/release/openhuman-memory-server --port 8420
```

## Logging

Structured logging via `tracing`. Configure with `RUST_LOG`:

```bash
# Debug level for the server
RUST_LOG=openhuman_memory_server=debug

# Debug for memory store, info for everything else
RUST_LOG=openhuman_memory=debug,info

# Trace-level detail
RUST_LOG=trace
```

Logs include method name, request ID, and operation details for easy debugging and monitoring.

## Performance Notes

- **Concurrent requests:** server uses tokio async runtime; handles many concurrent connections
- **Storage:** SQLite backend with indices on namespace, document key, and entity canonical IDs
- **Graph operations:** in-memory adjacency lists with lazy persistence
- **Embeddings:** optional; only trained on explicit `graph_train_embeddings` call

## Development

Run tests:

```bash
cargo test -p openhuman-memory-server
```

Integration tests use an in-process server with temporary workspace:

```bash
cargo test -p openhuman-memory-server --test '*' -- --nocapture
```
