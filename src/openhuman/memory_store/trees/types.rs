//! Re-exports core tree types from the standalone `openhuman-memory-types` crate.
//!
//! The canonical implementation lives in `openhuman_memory_types::tree`.

// Topic-tree hotness (Phase 3c) — formerly memory_store::trees_topic::types
// ============================================================================
//
// Folded in here because topic and global trees are not structurally distinct
// from source trees — they all live in the same `mem_tree_trees` table keyed
// by `TreeKind`. The only topic-specific extra state is the entity hotness
// counters in `mem_tree_entity_hotness`, which gate materialisation of a
// topic tree but are themselves not trees.

/// Hotness threshold above which a topic tree is materialised for an entity.
pub const TOPIC_CREATION_THRESHOLD: f32 = 10.0;

/// Hotness threshold below which a topic tree becomes an archive candidate.
pub const TOPIC_ARCHIVE_THRESHOLD: f32 = 2.0;

/// How often (in ingests touching the entity) to recompute hotness from the
/// full [`EntityIndexStats`]. Between recomputes only the cheap counters bump.
pub const TOPIC_RECHECK_EVERY: u32 = 100;

/// Input record fed to the hotness math (see `memory::tree_topic::hotness`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityIndexStats {
    pub mention_count_30d: u32,
    pub distinct_sources: u32,
    pub last_seen_ms: Option<i64>,
    pub query_hits_30d: u32,
    pub graph_centrality: Option<f32>,
}

/// Row persisted in `mem_tree_entity_hotness`. Persistence helpers live in
/// [`super::hotness`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HotnessCounters {
    pub entity_id: String,
    pub mention_count_30d: u32,
    pub distinct_sources: u32,
    pub last_seen_ms: Option<i64>,
    pub query_hits_30d: u32,
    pub graph_centrality: Option<f32>,
    pub ingests_since_check: u32,
    pub last_hotness: Option<f32>,
    pub last_updated_ms: i64,
}

impl HotnessCounters {
    pub fn fresh(entity_id: &str, now_ms: i64) -> Self {
        Self {
            entity_id: entity_id.to_string(),
            mention_count_30d: 0,
            distinct_sources: 0,
            last_seen_ms: None,
            query_hits_30d: 0,
            graph_centrality: None,
            ingests_since_check: 0,
            last_hotness: None,
            last_updated_ms: now_ms,
        }
    }

    pub fn stats(&self) -> EntityIndexStats {
        EntityIndexStats {
            mention_count_30d: self.mention_count_30d,
            distinct_sources: self.distinct_sources,
            last_seen_ms: self.last_seen_ms,
            query_hits_30d: self.query_hits_30d,
            graph_centrality: self.graph_centrality,
        }
    }
}

#[cfg(test)]
mod hotness_type_tests {
    use super::*;

    #[test]
    fn fresh_counters_are_zero() {
        let c = HotnessCounters::fresh("email:alice@example.com", 1_700_000_000_000);
        assert_eq!(c.entity_id, "email:alice@example.com");
        assert_eq!(c.mention_count_30d, 0);
        assert_eq!(c.distinct_sources, 0);
        assert_eq!(c.ingests_since_check, 0);
        assert!(c.last_hotness.is_none());
        assert!(c.last_seen_ms.is_none());
        assert_eq!(c.last_updated_ms, 1_700_000_000_000);
    }

    #[test]
    fn stats_projection_mirrors_row() {
        let c = HotnessCounters {
            entity_id: "e".into(),
            mention_count_30d: 5,
            distinct_sources: 2,
            last_seen_ms: Some(42),
            query_hits_30d: 1,
            graph_centrality: Some(0.3),
            ingests_since_check: 4,
            last_hotness: Some(9.9),
            last_updated_ms: 100,
        };
        let s = c.stats();
        assert_eq!(s.mention_count_30d, 5);
        assert_eq!(s.distinct_sources, 2);
        assert_eq!(s.last_seen_ms, Some(42));
        assert_eq!(s.query_hits_30d, 1);
        assert_eq!(s.graph_centrality, Some(0.3));
    }

    #[test]
    fn thresholds_make_creation_strictly_above_archive() {
        assert!(TOPIC_CREATION_THRESHOLD > TOPIC_ARCHIVE_THRESHOLD);
        assert!(TOPIC_RECHECK_EVERY > 0);
    }
}
=======
pub use openhuman_memory_types::tree::*;
