//! Integration test: scoring pipeline on sample chunks.

use chrono::Utc;
use openhuman_memory::store::chunks::types::{Chunk, Metadata, SourceKind};
use openhuman_memory::tree::score::{score_chunk, ScoringConfig};

fn sample_chunk(content: &str) -> Chunk {
    let now = Utc::now();
    Chunk {
        id: format!("test-{}", content.len()),
        content: content.to_string(),
        metadata: Metadata {
            source_kind: SourceKind::Chat,
            source_id: "test-source".into(),
            owner: String::new(),
            timestamp: now,
            time_range: (now, now),
            tags: vec![],
            source_ref: None,
            path_scope: None,
        },
        token_count: (content.len() / 4) as u32,
        seq_in_source: 0,
        created_at: now,
        partial_message: false,
    }
}

#[tokio::test]
async fn score_chunk_basic() {
    let cfg = ScoringConfig::default_regex_only();

    // A chunk with some entity-rich content
    let chunk = sample_chunk(
        "I met John Smith at Google headquarters in Mountain View. \
         We discussed the Q4 roadmap and the upcoming product launch on 2024-03-15.",
    );

    let result = score_chunk(&chunk, &cfg).await.unwrap();

    // Should produce a non-negative score
    assert!(result.total >= 0.0, "score should be non-negative, got {}", result.total);
    // Rich content should score higher than trivial content
    println!("Score: {:.3}, entities: {:?}", result.total, result.extracted.entities);
}

#[tokio::test]
async fn score_chunk_low_quality() {
    let cfg = ScoringConfig::default_regex_only();

    // A trivial chunk that should score low
    let chunk = sample_chunk("ok");

    let result = score_chunk(&chunk, &cfg).await.unwrap();
    assert!(result.total < 0.5, "trivial chunk should score low, got {}", result.total);
}
