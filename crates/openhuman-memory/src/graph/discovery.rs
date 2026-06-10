//! Automatic relation discovery from chunks.
//!
//! Scans chunks and their extracted entities to build graph edges:
//! - Co-occurrence: two entities in the same chunk → `co_occurs_with`
//! - Temporal: entities appearing sequentially in the same session → `follows`
//! - Pattern-based: simple regex rules like "X works at Y" → typed relation

use anyhow::Result;
use chrono::Utc;
use regex::Regex;

use crate::config::Config;
use crate::graph::persistent_store::{
    self, EvidenceRef, GraphEdgePersistent, GraphNode,
};
use crate::tree::score::extract::EntityKind;
use crate::tree::score::resolver::CanonicalEntity;

/// Discovered relation between two entities.
#[derive(Debug, Clone)]
pub struct DiscoveredRelation {
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub evidence_chunk_id: String,
    pub evidence_snippet: String,
    pub weight: f64,
}

/// Discover co-occurrence relations from a set of entities found in the same chunk.
/// Every pair of distinct entities gets a `co_occurs_with` edge.
pub fn discover_co_occurrences(
    entities: &[CanonicalEntity],
    chunk_id: &str,
    chunk_content: &str,
) -> Vec<DiscoveredRelation> {
    let mut relations = Vec::new();
    let snippet = &chunk_content[..chunk_content.len().min(100)];

    for i in 0..entities.len() {
        for j in (i + 1)..entities.len() {
            let a = &entities[i];
            let b = &entities[j];
            // Skip if same entity
            if a.canonical_id == b.canonical_id {
                continue;
            }
            relations.push(DiscoveredRelation {
                source_id: a.canonical_id.clone(),
                target_id: b.canonical_id.clone(),
                relation_type: "co_occurs_with".into(),
                evidence_chunk_id: chunk_id.into(),
                evidence_snippet: snippet.into(),
                weight: 1.0,
            });
        }
    }
    relations
}

/// Discover temporal (sequential) relations from entities in consecutive chunks
/// within the same session/source.
pub fn discover_temporal_relations(
    prev_entities: &[CanonicalEntity],
    curr_entities: &[CanonicalEntity],
    chunk_id: &str,
    chunk_content: &str,
) -> Vec<DiscoveredRelation> {
    let mut relations = Vec::new();
    let snippet = &chunk_content[..chunk_content.len().min(80)];

    for prev in prev_entities {
        for curr in curr_entities {
            if prev.canonical_id == curr.canonical_id {
                continue;
            }
            relations.push(DiscoveredRelation {
                source_id: prev.canonical_id.clone(),
                target_id: curr.canonical_id.clone(),
                relation_type: "follows".into(),
                evidence_chunk_id: chunk_id.into(),
                evidence_snippet: snippet.into(),
                weight: 0.5, // weaker than co-occurrence
            });
        }
    }
    relations
}

/// Pattern-based relation extraction using simple regex rules.
/// Returns typed relations like "works_at", "located_in", "created_by".
pub fn discover_pattern_relations(
    text: &str,
    entities: &[CanonicalEntity],
    chunk_id: &str,
) -> Vec<DiscoveredRelation> {
    let mut relations = Vec::new();

    // Build entity lookup by surface form
    let persons: Vec<&CanonicalEntity> = entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Person))
        .collect();
    let orgs: Vec<&CanonicalEntity> = entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Organization))
        .collect();
    let locations: Vec<&CanonicalEntity> = entities
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Location))
        .collect();

    // Pattern: "<Person> works at/for <Org>"
    let works_at_re = Regex::new(r"(?i)works?\s+(?:at|for)\s+").unwrap();
    if works_at_re.is_match(text) {
        for person in &persons {
            for org in &orgs {
                if text.contains(&person.surface) && text.contains(&org.surface) {
                    relations.push(DiscoveredRelation {
                        source_id: person.canonical_id.clone(),
                        target_id: org.canonical_id.clone(),
                        relation_type: "works_at".into(),
                        evidence_chunk_id: chunk_id.into(),
                        evidence_snippet: text[..text.len().min(100)].into(),
                        weight: 2.0,
                    });
                }
            }
        }
    }

    // Pattern: "<Person> is from/lives in <Location>"
    let lives_in_re = Regex::new(r"(?i)(?:is from|lives? in|based in|located in)\s+").unwrap();
    if lives_in_re.is_match(text) {
        for person in &persons {
            for loc in &locations {
                if text.contains(&person.surface) && text.contains(&loc.surface) {
                    relations.push(DiscoveredRelation {
                        source_id: person.canonical_id.clone(),
                        target_id: loc.canonical_id.clone(),
                        relation_type: "located_in".into(),
                        evidence_chunk_id: chunk_id.into(),
                        evidence_snippet: text[..text.len().min(100)].into(),
                        weight: 1.5,
                    });
                }
            }
        }
    }

    // Pattern: "<Org> in/at <Location>"
    if !orgs.is_empty() && !locations.is_empty() {
        let org_loc_re = Regex::new(r"(?i)(?:headquartered|offices?|based)\s+(?:in|at)\s+").unwrap();
        if org_loc_re.is_match(text) {
            for org in &orgs {
                for loc in &locations {
                    if text.contains(&org.surface) && text.contains(&loc.surface) {
                        relations.push(DiscoveredRelation {
                            source_id: org.canonical_id.clone(),
                            target_id: loc.canonical_id.clone(),
                            relation_type: "located_in".into(),
                            evidence_chunk_id: chunk_id.into(),
                            evidence_snippet: text[..text.len().min(100)].into(),
                            weight: 1.5,
                        });
                    }
                }
            }
        }
    }

    relations
}

/// Persist discovered relations into the graph store.
/// Auto-creates nodes if they don't exist.
pub fn persist_discoveries(
    config: &Config,
    relations: &[DiscoveredRelation],
    entities: &[CanonicalEntity],
) -> Result<()> {
    // Ensure all referenced entities exist as nodes
    for entity in entities {
        let node = GraphNode {
            node_id: entity.canonical_id.clone(),
            entity_type: format!("{:?}", entity.kind),
            label: entity.surface.clone(),
            properties: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        persistent_store::upsert_node(config, &node)?;
    }

    // Persist edges (increment weight for existing edges)
    for rel in relations {
        persistent_store::increment_edge_weight(
            config,
            &rel.source_id,
            &rel.target_id,
            &rel.relation_type,
            rel.weight,
        )?;
    }

    Ok(())
}

/// Full discovery pipeline for a single chunk: extract all relation types
/// and persist them.
pub fn discover_and_persist(
    config: &Config,
    chunk_id: &str,
    chunk_content: &str,
    entities: &[CanonicalEntity],
    prev_entities: Option<&[CanonicalEntity]>,
) -> Result<usize> {
    let mut all_relations = Vec::new();

    // Co-occurrence
    all_relations.extend(discover_co_occurrences(entities, chunk_id, chunk_content));

    // Temporal (if previous chunk entities available)
    if let Some(prev) = prev_entities {
        all_relations.extend(discover_temporal_relations(prev, entities, chunk_id, chunk_content));
    }

    // Pattern-based
    all_relations.extend(discover_pattern_relations(chunk_content, entities, chunk_id));

    let count = all_relations.len();
    persist_discoveries(config, &all_relations, entities)?;
    Ok(count)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config() -> (TempDir, Config) {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.workspace_dir = tmp.path().to_path_buf();
        (tmp, cfg)
    }

    fn entity(id: &str, kind: EntityKind, surface: &str) -> CanonicalEntity {
        CanonicalEntity {
            canonical_id: id.into(),
            kind,
            surface: surface.into(),
            span_start: 0,
            span_end: surface.len() as u32,
            score: 1.0,
        }
    }

    #[test]
    fn co_occurrence_generates_pairs() {
        let entities = vec![
            entity("person:alice", EntityKind::Person, "Alice"),
            entity("org:google", EntityKind::Organization, "Google"),
            entity("loc:mv", EntityKind::Location, "Mountain View"),
        ];
        let rels = discover_co_occurrences(&entities, "c1", "Alice at Google in Mountain View");
        // 3 entities → 3 pairs: (alice,google), (alice,mv), (google,mv)
        assert_eq!(rels.len(), 3);
        assert!(rels.iter().all(|r| r.relation_type == "co_occurs_with"));
    }

    #[test]
    fn pattern_works_at() {
        let entities = vec![
            entity("person:alice", EntityKind::Person, "Alice"),
            entity("org:google", EntityKind::Organization, "Google"),
        ];
        let rels = discover_pattern_relations(
            "Alice works at Google on AI projects",
            &entities,
            "c1",
        );
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].relation_type, "works_at");
        assert_eq!(rels[0].source_id, "person:alice");
        assert_eq!(rels[0].target_id, "org:google");
    }

    #[test]
    fn full_pipeline_persists() {
        let (_tmp, cfg) = test_config();
        let entities = vec![
            entity("person:bob", EntityKind::Person, "Bob"),
            entity("org:acme", EntityKind::Organization, "Acme"),
        ];
        let count = discover_and_persist(
            &cfg,
            "chunk-1",
            "Bob works for Acme in the engineering team",
            &entities,
            None,
        )
        .unwrap();
        assert!(count >= 2); // co-occurrence + works_at pattern

        // Verify node was created
        let node = persistent_store::get_node(&cfg, "person:bob").unwrap();
        assert!(node.is_some());

        // Verify edges exist
        let edges = persistent_store::edges_from(&cfg, "person:bob", None, 10).unwrap();
        assert!(!edges.is_empty());
    }
}
