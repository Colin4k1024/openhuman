# openhuman-embeddings

Multi-backend embedding vector provider for semantic search.

Trait-based abstraction over OpenAI, Ollama, Cohere, Voyage, and a no-op testing backend. Converts text into numerical vectors for similarity search and semantic reranking.

## Supported Backends

- **OpenAI** — `text-embedding-3-small`, `text-embedding-3-large`, etc. via `https://api.openai.com`
- **Ollama** — local embedding server (e.g., `http://localhost:11434`)
- **Cohere** — Cohere API-compatible embedder
- **Voyage** — Voyage API-compatible embedder
- **Custom** — any OpenAI-compatible endpoint via `custom:<url>` provider slug
- **Noop** — mock provider for testing (returns zero vectors)

## EmbeddingProvider Trait

Core abstraction for all backends:

```rust
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}
```

Each provider converts a batch of text strings into a batch of float vectors. Dimension size is specified at construction time and validated on each call.

## Factory

Use `create_provider()` to instantiate any backend:

```rust
use openhuman_embeddings::create_provider;

let provider = create_provider(
    "openai",                      // provider slug
    "text-embedding-3-small",      // model ID
    1536,                           // embedding dimensions
    "sk-...",                       // API key (empty string if not needed)
    "",                             // ollama_base_url (ignored for openai)
    None,                           // http_client (optional pre-configured reqwest::Client)
)?;

let vectors = provider.embed(&["hello world", "foo bar"]).await?;
assert_eq!(vectors.len(), 2);
assert_eq!(vectors[0].len(), 1536);
```

### Provider Slugs

| Slug | Backend | API Key | Base URL |
|------|---------|---------|----------|
| `openai` | OpenAI | required | (fixed at `https://api.openai.com`) |
| `ollama` | Ollama | not needed | required in `ollama_base_url` |
| `voyage` | Voyage | required | (fixed) |
| `cohere` | Cohere | required | (fixed) |
| `custom:<url>` | OpenAI-compatible | optional | embedded in slug |
| `none` | Noop (testing) | not needed | not needed |

## Configuration

### Environment Variables

Each backend respects standard env vars:

| Backend | Env Var | Example |
|---------|---------|---------|
| OpenAI | `OPENAI_API_KEY` | `sk-proj-...` |
| Ollama | `OLLAMA_BASE_URL` | `http://localhost:11434` |
| Voyage | `VOYAGE_API_KEY` | `pa-...` |
| Cohere | `COHERE_API_KEY` | `co_...` |

Most applications load these at startup and pass via `create_provider()` parameters.

## Usage Example

Minimal embedding pipeline:

```rust
use openhuman_embeddings::create_provider;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create an Ollama provider (local, no auth)
    let provider = create_provider(
        "ollama",
        "nomic-embed-text",
        768,
        "",
        "http://localhost:11434",
        None,
    )?;

    // Embed a batch of texts
    let texts = vec!["document one", "document two"];
    let vectors = provider.embed(&texts).await?;

    // Use vectors for similarity search
    println!("Embedded {} texts into {} dimensions", texts.len(), vectors[0].len());

    Ok(())
}
```

## Error Handling

All backends return `anyhow::Result<Vec<Vec<f32>>>`. Common failures:

- `create_provider()` with unknown slug → `"unknown embedding provider"`
- Network failure (HTTP request) → wrapped `reqwest` error
- API rate limit → propagated from backend
- Dimension mismatch → caught and reported as error

## Testing

Use the `noop` backend for deterministic testing:

```rust
let provider = create_provider("none", "", 0, "", "", None)?;
let vectors = provider.embed(&["test"]).await?;
// Returns empty vectors for testing
```

## License

MIT
