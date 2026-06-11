# openhuman-memory-python

Python bindings for the OpenHuman memory system via PyO3 and maturin. Provides synchronous access to document storage, key-value persistence, and text scoring without requiring the HTTP server.

## Installation

### Build from source

```bash
cd crates/openhuman-memory-python
maturin develop
```

This builds the Rust extension module and installs it in your Python environment.

### Requirements

- Python 3.8+
- Rust 1.70+
- `maturin` (install via `pip install maturin`)

## Usage

### Basic Setup

```python
from openhuman_memory import Config, MemoryStore

# Create configuration (uses ~/.openhuman-memory by default)
config = Config()
# Or specify a custom workspace
config = Config(workspace="/tmp/my-memory")

# Initialize memory store
store = MemoryStore(config)
```

### Documents

Insert and retrieve documents in namespaces:

```python
# Upsert a document
doc_id = store.upsert_document(
    namespace="github",
    key="issue-456",
    title="Performance regression in auth",
    content="User reports 10s login latency in production..."
)
print(f"Document ID: {doc_id}")

# List documents in a namespace
docs_json = store.list_documents(namespace="github")
print(docs_json)  # Returns JSON string

# List all namespaces
namespaces = store.list_namespaces()
print(namespaces)  # ["github", "slack", "notes"]

# Clear all documents in a namespace
store.clear_namespace(namespace="github")
```

### Key-Value Store

Store and retrieve global key-value pairs:

```python
# Set a value (must be JSON-serializable)
store.kv_set("user_theme", '"dark"')  # Note: JSON string
store.kv_set("user_count", "42")

# Get a value
theme = store.kv_get("user_theme")
print(theme)  # "dark" (JSON decoded)

# Delete a key
store.kv_delete("user_theme")
```

### Text Scoring

Score text chunks for admission to long-term memory:

```python
from openhuman_memory import score_chunk
import json

# Score a text chunk
result_json = score_chunk(
    text="Meeting notes: discussed Q3 roadmap with engineering team",
    source_kind="document"  # or "email", "chat"
)

# Parse result
result = json.loads(result_json)
print(result)
# {
#   "chunk_id": "py-1717081200...",
#   "total": 0.72,
#   "kept": True,
#   "drop_reason": None,
#   "entities": [
#     {"text": "Q3", "kind": "Date"},
#     {"text": "engineering team", "kind": "Organization"}
#   ]
# }

# Check if chunk is worth storing
if result["kept"]:
    store.upsert_document(
        namespace="meetings",
        key=f"meeting-{result['chunk_id']}",
        title="Meeting notes",
        content="..."
    )
```

## API Reference

### `Config`

Configuration for the memory system.

#### Constructor

```python
Config(workspace: str = None) -> Config
```

- `workspace` (optional): Path to memory workspace directory. Defaults to `~/.openhuman-memory`. Created if it does not exist.

**Raises:**
- `RuntimeError` if workspace directory cannot be created

#### Properties

```python
config.workspace: str  # Get workspace path
```

### `MemoryStore`

Main interface to the memory system.

#### Constructor

```python
MemoryStore(config: Config) -> MemoryStore
```

**Raises:**
- `RuntimeError` if store initialization fails

#### Methods

##### `upsert_document(namespace, key, title, content) -> str`

Insert or update a document.

**Parameters:**
- `namespace` (str): Namespace grouping (e.g., "github", "slack")
- `key` (str): Unique key within namespace
- `title` (str): Document title
- `content` (str): Document content

**Returns:** Document ID (str)

**Raises:** `RuntimeError` on storage error

##### `list_documents(namespace=None) -> str`

List documents in a namespace or all namespaces.

**Parameters:**
- `namespace` (str, optional): If provided, list only documents in this namespace. If None, list all.

**Returns:** JSON string array of documents with metadata

**Raises:** `RuntimeError` on storage error

##### `list_namespaces() -> list[str]`

List all namespaces with at least one document.

**Returns:** List of namespace names

**Raises:** `RuntimeError` on storage error

##### `clear_namespace(namespace) -> None`

Delete all documents in a namespace.

**Parameters:**
- `namespace` (str): Namespace to clear

**Returns:** None

**Raises:** `RuntimeError` on storage error

##### `kv_set(key, value_json, namespace=None) -> None`

Store a global key-value pair.

**Parameters:**
- `key` (str): Key name
- `value_json` (str): JSON-encoded value (e.g., `'"hello"'`, `'42'`, `'{"a": 1}'`)
- `namespace` (str, optional): Namespace scope (default: global)

**Returns:** None

**Raises:** `RuntimeError` if JSON is invalid or storage fails

**Example:**
```python
store.kv_set("config", '{"theme": "dark", "lang": "en"}')
```

##### `kv_get(key, namespace=None) -> str | None`

Retrieve a global key-value pair.

**Parameters:**
- `key` (str): Key name
- `namespace` (str, optional): Namespace scope (default: global)

**Returns:** JSON string value, or None if key not found

**Raises:** `RuntimeError` on storage error

**Example:**
```python
value = store.kv_get("config")
if value:
    config = json.loads(value)
```

##### `kv_delete(key, namespace=None) -> None`

Delete a global key-value pair.

**Parameters:**
- `key` (str): Key name
- `namespace` (str, optional): Namespace scope (default: global)

**Returns:** None

**Raises:** `RuntimeError` on storage error

### `score_chunk(text, source_kind=None) -> str`

Score a text chunk for admission to memory. Standalone function.

**Parameters:**
- `text` (str): Text to score
- `source_kind` (str, optional): Source type — `"chat"` (default), `"email"`, or `"document"`

**Returns:** JSON string with score details:
```json
{
  "chunk_id": "py-...",
  "total": 0.75,
  "kept": true,
  "drop_reason": null,
  "entities": [
    {"text": "2024-03-15", "kind": "Date"},
    {"text": "alice@example.com", "kind": "Email"}
  ]
}
```

**Raises:** `RuntimeError` on scoring error

## Limitations

The Python bindings provide synchronous access to core memory operations. The following are **not** available:

- **Async methods:** All operations block; no async/await support
- **Embedding search:** No semantic similarity search without the HTTP server
- **Graph operations:** Knowledge graph methods (topology, temporal decay, export) are Rust/server-only
- **Sync & federation:** Multi-instance synchronization requires the HTTP server and coordination layer

For these features, use the HTTP server (`openhuman-memory-server`) or the Rust API directly.

## Examples

### Store and retrieve chat history

```python
from openhuman_memory import Config, MemoryStore
import json

config = Config(workspace="/tmp/chat-memory")
store = MemoryStore(config)

# Store a conversation turn
store.upsert_document(
    namespace="chat",
    key="turn-1",
    title="User query about Rust",
    content="How do I handle errors in Rust?"
)

# Retrieve all chat documents
docs = store.list_documents(namespace="chat")
print(docs)
```

### Score and filter messages

```python
from openhuman_memory import score_chunk
import json

messages = [
    "ok",  # Trivial
    "Here's a detailed explanation of async/await in Rust...",  # Worth keeping
    "Great!",  # Trivial
]

for msg in messages:
    result = json.loads(score_chunk(msg))
    if result["kept"]:
        print(f"Keep: {msg[:50]}")
    else:
        print(f"Drop ({result['drop_reason']}): {msg[:50]}")
```

### Multi-namespace organization

```python
from openhuman_memory import Config, MemoryStore

store = MemoryStore(Config())

# Organize by source
store.upsert_document("github", "pr-123", "PR Title", "PR content...")
store.upsert_document("slack", "conv-456", "Slack thread", "Thread content...")
store.upsert_document("email", "msg-789", "Email subject", "Email body...")

# List all namespaces
namespaces = store.list_namespaces()
# ["github", "slack", "email"]

# Clear one source
store.clear_namespace("slack")
```

## Troubleshooting

### `ImportError: cannot import name 'openhuman_memory'`

The extension module was not built. Rebuild with:
```bash
cd crates/openhuman-memory-python
maturin develop
```

### `RuntimeError: tokio runtime: ...`

The internal async runtime failed to initialize. Check disk space and system resources.

### `RuntimeError: memory init: ...`

The memory store could not open or create the workspace. Verify:
- Workspace path is writable
- Disk space is available
- No permission errors in the parent directory

## Development

Run tests:

```bash
cd crates/openhuman-memory-python
cargo test --lib
```

Or via pytest (if available):

```bash
maturin develop
pytest tests/
```
