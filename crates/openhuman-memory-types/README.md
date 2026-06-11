# openhuman-memory-types

Shared type definitions for the OpenHuman memory system.

Pure data structures used throughout the memory pipeline: chunks, trees, summaries, metadata. No logic dependencies—only `serde`, `chrono`, and `sha2`.

## Modules

### chunk

Core types for Phase 1 memory ingestion.

**Chunk** — the atomic persistence unit.

- `id: String` — deterministic SHA256 hash derived from source_kind, source_id, and sequence number
- `content: String` — canonical Markdown content
- `metadata: Metadata` — provenance and origin information
- `token_count: u32` — rough token count (1 token ≈ 4 chars)
- `seq_in_source: u32` — stable sequence number starting at 0
- `created_at: DateTime<Utc>` — persistence timestamp
- `partial_message: bool` — true if this chunk is a sub-split of a larger logical unit (e.g., message exceeding `max_tokens`)

**Metadata** — provenance captured at ingest time.

- `source_kind: SourceKind` — discriminator: Chat, Email, or Document
- `source_id: String` — stable logical ID for the ingestion group (channel ID, thread ID, doc ID)
- `owner: String` — account or user the content belongs to
- `timestamp: DateTime<Utc>` — point-in-time for ordering within source
- `time_range: (DateTime<Utc>, DateTime<Utc>)` — covering time range (usually point-in-time for leaves, widens for summaries)
- `tags: Vec<String>` — arbitrary labels from source (Gmail labels, Slack reactions, Notion tags)
- `source_ref: Option<SourceRef>` — opaque pointer back to raw source record for drill-down
- `path_scope: Option<String>` — overrides `source_id` for file path grouping when set

**SourceKind** — which kind of upstream source produced a chunk.

- `Chat` — transcript scoped by channel or group (Slack, Discord, Telegram, WhatsApp)
- `Email` — email thread (Gmail, generic IMAP)
- `Document` — standalone document (Notion, Drive, meeting note, uploaded file)

**DataSource** — concrete upstream provider.

- `Discord`, `Telegram`, `Whatsapp` → `SourceKind::Chat`
- `Gmail`, `OtherEmail` → `SourceKind::Email`
- `Notion`, `MeetingNotes`, `DriveDocs` → `SourceKind::Document`

**SourceRef** — opaque provider-specific back-pointer.

- `value: String` — provider-specific identifier for citation and drill-down

### tree

Core types for Phase 3a — summary trees, per-source bucket-seal.

**Tree** — one summary-tree instance, grouping leaves under one scope.

- `id: String` — unique tree identifier
- `kind: TreeKind` — Source, Topic, or Global
- `scope: String` — logical identifier for what the tree covers (e.g., `chat:slack:#eng`)
- `root_id: Option<String>` — None until first seal emits an L1 node
- `max_level: u32` — highest level that has ever sealed
- `status: TreeStatus` — Active or Archived
- `created_at: DateTime<Utc>` — tree creation timestamp
- `last_sealed_at: Option<DateTime<Utc>>` — most recent seal time

**TreeKind** — what kind of tree this is.

- `Source` — one tree per ingest source (e.g., `chat:slack:#eng`, `email:gmail:user`)
- `Topic` — per-entity or topic tree (Phase 3c)
- `Global` — cross-source daily digest tree (Phase 3b)

**TreeStatus** — activity state of a tree.

- `Active` — accepts new leaves
- `Archived` — stays queryable but rejects new leaves

**SummaryNode** — sealed summary node, one level above raw leaves.

- `id: String` — unique node identifier
- `tree_id: String` — parent tree identifier
- `level: u32` — 1 for summaries over leaves, 2 over L1 summaries, etc.
- `parent_id: Option<String>` — parent node (None for root)
- `child_ids: Vec<String>` — concrete children fixed at seal time
- `content: String` — summariser output (target: 800–1500 tokens)
- `token_count: u32` — content token count
- `entities: Vec<String>` — curated subset of children's entity IDs
- `topics: Vec<String>` — curated topic labels (hashtag-like phrases)
- `time_range_start` / `time_range_end: DateTime<Utc>` — covers all children's time ranges
- `score: f32` — max of children's scores at seal time (for Phase 4 reranking)
- `sealed_at: DateTime<Utc>` — when this node was sealed
- `deleted: bool` — tombstone flag (false in Phase 3a; reserved for cleanup)
- `embedding: Option<Vec<f32>>` — Phase 4: summary content embedding for semantic rerank

**Buffer** — unsealed frontier at a given `(tree_id, level)`.

- `tree_id: String` — parent tree identifier
- `level: u32` — buffer level
- `item_ids: Vec<String>` — pending child IDs (chunks or lower-level summaries)
- `token_sum: i64` — accumulated token count
- `oldest_at: Option<DateTime<Utc>>` — timestamp of oldest item (None when empty; used for time-based flush)

**HotnessCounters** — activity metrics per entity.

- Tracks recency, frequency, and derived hotness score for Phase 3c entity trees.

## Usage Example

Constructing a chunk for Phase 1 ingestion:

```rust
use openhuman_memory_types::{Chunk, Metadata, SourceKind, SourceRef};
use chrono::Utc;

let metadata = Metadata::point_in_time(
    SourceKind::Chat,
    "slack:channel:C123456",
    "user@example.com",
    Utc::now(),
);

let chunk = Chunk {
    id: chunk_id(&"chat", &"slack:channel:C123456", 0),
    content: "Meeting notes: discussed Q3 roadmap...".to_string(),
    metadata,
    token_count: 42,
    seq_in_source: 0,
    created_at: Utc::now(),
    partial_message: false,
};
```

## License

MIT
