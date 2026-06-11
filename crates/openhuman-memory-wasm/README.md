# openhuman-memory-wasm

Browser-safe WebAssembly functions for client-side memory processing. Provides lightweight text scoring, entity extraction, and vector similarity without requiring network calls or native runtimes.

## Build

Build the WASM module:

```bash
cd crates/openhuman-memory-wasm
wasm-pack build --target web
```

Output:
- `pkg/openhuman_memory_wasm.wasm` — binary module
- `pkg/openhuman_memory_wasm.d.ts` — TypeScript definitions
- `pkg/openhuman_memory_wasm.js` — JavaScript bindings

### Build options

```bash
# Production (optimized, minimized)
wasm-pack build --release --target web

# Development (debug symbols, larger size)
wasm-pack build --dev --target web

# For Node.js (instead of web)
wasm-pack build --target nodejs
```

## Usage

### JavaScript / TypeScript

Import and use in your web application:

```typescript
import init, {
  approx_token_count,
  cosine_similarity,
  extract_entities,
  score_text,
} from "@openhuman/memory-wasm";

// Initialize the WASM module (async, one-time)
await init();

// Now use the functions
const tokens = approx_token_count("Hello, world!");
console.log(`Approximate tokens: ${tokens}`);
```

### React Example

```typescript
import { useEffect, useState } from "react";
import init, { score_text } from "@openhuman/memory-wasm";

export function TextScorerComponent() {
  const [wasmReady, setWasmReady] = useState(false);

  useEffect(() => {
    init().then(() => setWasmReady(true));
  }, []);

  const handleScoreText = (text: string) => {
    if (!wasmReady) {
      console.warn("WASM not ready yet");
      return;
    }
    const resultJson = score_text(text);
    const result = JSON.parse(resultJson);
    console.log(`Score: ${result.total}, Kept: ${result.kept}`);
  };

  return (
    <button onClick={() => handleScoreText("Sample text for scoring...")}>
      Score Text
    </button>
  );
}
```

## API Reference

All functions are pure computations with no side effects. Imports are available in `openhuman_memory_wasm`:

### `approx_token_count(text: string): number`

Estimate token count using the 4-characters-per-token heuristic.

**Parameters:**
- `text` — Text to count

**Returns:** Approximate token count (number, minimum 1)

**Example:**
```typescript
const count = approx_token_count("The quick brown fox");
// count = 5
```

### `cosine_similarity(a: Float32Array, b: Float32Array): number`

Compute cosine similarity between two float32 vectors.

**Parameters:**
- `a` — First vector (Float32Array)
- `b` — Second vector (Float32Array)

**Returns:** Similarity score (number, 0-1 range for normalized vectors)

**Notes:**
- Both vectors must have the same length
- Returns 0 if vectors have zero length or are orthogonal
- Handles zero-norm vectors gracefully

**Example:**
```typescript
const a = new Float32Array([1.0, 0.0, 0.0]);
const b = new Float32Array([1.0, 0.0, 0.0]);
const sim = cosine_similarity(a, b);
// sim = 1.0 (identical vectors)

const c = new Float32Array([0.0, 1.0, 0.0]);
const orth = cosine_similarity(a, c);
// orth ≈ 0.0 (orthogonal vectors)
```

### `extract_entities(text: string): string`

Extract named entities (dates, emails, URLs) from text using regex patterns.

**Parameters:**
- `text` — Text to extract entities from

**Returns:** JSON string array of entity objects

**Entity object:**
```typescript
{
  text: string;     // The matched text
  kind: string;     // Entity type: "Date", "Email", "Url"
}
```

**Supported patterns:**
- **Date:** YYYY-MM-DD, MM/DD/YYYY
- **Email:** RFC-style email addresses
- **Url:** HTTP and HTTPS URLs

**Example:**
```typescript
const result = extract_entities("Contact alice@example.com on 2024-06-15");
const entities = JSON.parse(result);
// entities = [
//   { text: "2024-06-15", kind: "Date" },
//   { text: "alice@example.com", kind: "Email" }
// ]
```

### `score_text(text: string): string`

Score text for memory admission. Combines multiple signals: token density, unique word ratio, and entity presence.

**Parameters:**
- `text` — Text to score

**Returns:** JSON string with scoring details

**Result object:**
```typescript
{
  total: number;              // Overall score (0-1)
  kept: boolean;              // true if score > 0.25 threshold
  token_count: number;        // Approximate token count
  unique_words_ratio: number; // Ratio of unique to total words (0-1)
  token_signal: number;       // Token density signal (0-1)
  entity_count: number;       // Number of entities detected
  entities: Array<{           // Extracted entities
    text: string;
    kind: string;
  }>;
}
```

**Scoring logic:**
- **unique_words_ratio:** 40% weight — higher uniqueness scores better
- **token_signal:** 40% weight — peaks for text between 50-200 tokens
- **entity_bonus:** 20% weight — capped at 0.3, based on entity count
- **Admission threshold:** 0.25 → kept = true if score exceeds threshold

**Example:**
```typescript
const result = score_text("Quick note: meeting at 3pm with alice@example.com");
const score = JSON.parse(result);
console.log(score);
// {
//   total: 0.62,
//   kept: true,
//   token_count: 10,
//   unique_words_ratio: 0.9,
//   token_signal: 0.35,
//   entity_count: 2,
//   entities: [
//     { text: "3pm", kind: "Date" },
//     { text: "alice@example.com", kind: "Email" }
//   ]
// }
```

## Use Cases

### Client-side text preprocessing

Before sending text to the server, score it locally to filter trivial content:

```typescript
async function submitMessage(text: string) {
  const scoreResult = JSON.parse(score_text(text));
  if (!scoreResult.kept) {
    console.log("Skipping trivial message");
    return;
  }
  // Send to server
  await fetch("/api/memory/upsert", {
    method: "POST",
    body: JSON.stringify({ content: text, ...scoreResult }),
  });
}
```

### Offline similarity matching

Find similar embeddings without server round-trips:

```typescript
// User-provided embedding (from model or server)
const userEmbedding = new Float32Array([0.1, 0.2, 0.3, ...]);

// Local embeddings from cache
const cachedEmbeddings = [
  new Float32Array([0.11, 0.21, 0.31, ...]),
  new Float32Array([0.9, 0.8, 0.7, ...]),
];

// Find best match
const similarities = cachedEmbeddings.map(
  (emb) => cosine_similarity(userEmbedding, emb)
);
const bestMatch = Math.max(...similarities);
console.log(`Best match similarity: ${bestMatch}`);
```

### Entity highlighting in UI

Extract and highlight entities in user-generated text:

```typescript
function TextDisplay({ text }: { text: string }) {
  const entities = JSON.parse(extract_entities(text));
  const entitySet = new Set(entities.map((e) => e.text));

  return (
    <p>
      {text.split(/\s+/).map((word, idx) =>
        entitySet.has(word) ? (
          <span key={idx} className="highlight-entity">
            {word}
          </span>
        ) : (
          <span key={idx}>{word} </span>
        )
      )}
    </p>
  );
}
```

### Token budget calculation

Estimate text size before sending to a token-limited API:

```typescript
const userInput = document.getElementById("text-input").value;
const tokenCount = approx_token_count(userInput);
const tokenLimit = 2000;
const usage = ((tokenCount / tokenLimit) * 100).toFixed(1);

console.log(`${tokenCount} / ${tokenLimit} tokens (${usage}%)`);
if (tokenCount > tokenLimit) {
  alert("Text is too long");
}
```

## Limitations

This WASM module focuses on **client-side preprocessing only**. The following are **not** available:

- **Persistent storage:** No SQLite or file access in the browser
- **Semantic search:** No embedding models or vector database queries
- **Knowledge graph:** No graph operations or temporal decay
- **Async operations:** All functions are synchronous
- **Custom entity extraction:** Regex patterns are fixed; cannot be extended from JavaScript

For full memory capabilities, use:
- **Python bindings** (`openhuman-memory-python`) for server-side Python applications
- **HTTP server** (`openhuman-memory-server`) for distributed systems

## Performance

All functions run in WebAssembly with near-native performance:

- `approx_token_count`: ~1µs per call
- `cosine_similarity`: ~1µs per 1000-dim vector
- `extract_entities`: ~10-100µs depending on text length
- `score_text`: ~50-200µs depending on text length and entity count

Typical web page load adds ~200KB WASM binary size.

## Testing

Run tests in the Rust test suite:

```bash
cd crates/openhuman-memory-wasm
cargo test --target wasm32-unknown-unknown
```

Or with `wasm-pack test`:

```bash
wasm-pack test --headless --firefox
```

## Building for Production

Optimize the WASM binary:

```bash
# Build with LTO and release optimizations
wasm-pack build --release --target web

# Further compress with wasm-opt (if available)
npm install -g wasm-opt
wasm-opt -O4 -o pkg/openhuman_memory_wasm_opt.wasm pkg/openhuman_memory_wasm.wasm

# Result: ~50KB optimized binary
```

## Compatibility

- **Browser support:** All modern browsers with WebAssembly support (Chrome 74+, Firefox 79+, Safari 14.1+, Edge 74+)
- **Module systems:** ES modules (native), CommonJS (with bundler support), Node.js
- **Frameworks:** Works with React, Vue, Svelte, Angular, Vanilla JS

## License

MIT
