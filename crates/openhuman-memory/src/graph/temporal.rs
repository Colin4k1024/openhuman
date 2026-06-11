//! Temporal graph — time-windowed queries, relation decay, and export.
//!
//! Edges in the persistent store carry `first_seen_ms`, `last_seen_ms`, and
//! `weight`. This module adds:
//! - Time-windowed queries: "most active edges in the last N days"
//! - Relation decay: reduce weight of edges not seen recently
//! - Interaction count tracking via weight increment
//! - DOT/JSON export for visualization

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;
use super::persistent_store::{ensure_schema_inner, GraphEdgePersistent};

// ─── Types ───────────────────────────────────────────────────────────────────

/// An edge with temporal metadata surfaced for caller convenience.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporalEdge {
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub weight: f64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    /// Approximate interaction count (weight at creation = 1.0 per interaction).
    pub interaction_count: u64,
}

/// Parameters for the decay function.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecayConfig {
    /// Half-life in days: after this many days without interaction, weight halves.
    pub half_life_days: f64,
    /// Minimum weight below which edges are pruned (0 = never prune).
    pub min_weight: f64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            half_life_days: 30.0,
            min_weight: 0.1,
        }
    }
}

/// Export format.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphExport {
    pub format: String,
    pub content: String,
    pub node_count: usize,
    pub edge_count: usize,
}

// ─── Time-windowed queries ───────────────────────────────────────────────────

/// Get the most active edges within a time window (by last_seen recency + weight).
pub fn most_active_edges(
    config: &Config,
    since: DateTime<Utc>,
    limit: usize,
) -> Result<Vec<TemporalEdge>> {
    with_connection(config, |conn| {
        ensure_schema_inner(conn)?;
        let since_ms = since.timestamp_millis();
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, relation_type, weight, first_seen_ms, last_seen_ms
             FROM mem_graph_edges
             WHERE last_seen_ms >= ?1
             ORDER BY weight DESC, last_seen_ms DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![since_ms, limit as i64], |row| {
                Ok(TemporalEdge {
                    source_id: row.get(0)?,
                    target_id: row.get(1)?,
                    relation_type: row.get(2)?,
                    weight: row.get(3)?,
                    first_seen: ms_to_dt(row.get(4)?),
                    last_seen: ms_to_dt(row.get(5)?),
                    interaction_count: row.get::<_, f64>(3).map(|w| w.round() as u64).unwrap_or(0),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("most_active_edges")?;
        Ok(rows)
    })
}

/// Get edges involving a specific node within a time window.
pub fn node_activity(
    config: &Config,
    node_id: &str,
    since: DateTime<Utc>,
    limit: usize,
) -> Result<Vec<TemporalEdge>> {
    with_connection(config, |conn| {
        ensure_schema_inner(conn)?;
        let since_ms = since.timestamp_millis();
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, relation_type, weight, first_seen_ms, last_seen_ms
             FROM mem_graph_edges
             WHERE (source_id = ?1 OR target_id = ?1) AND last_seen_ms >= ?2
             ORDER BY last_seen_ms DESC
             LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![node_id, since_ms, limit as i64], |row| {
                Ok(TemporalEdge {
                    source_id: row.get(0)?,
                    target_id: row.get(1)?,
                    relation_type: row.get(2)?,
                    weight: row.get(3)?,
                    first_seen: ms_to_dt(row.get(4)?),
                    last_seen: ms_to_dt(row.get(5)?),
                    interaction_count: row.get::<_, f64>(3).map(|w| w.round() as u64).unwrap_or(0),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("node_activity")?;
        Ok(rows)
    })
}

// ─── Decay ───────────────────────────────────────────────────────────────────

/// Apply exponential decay to all edges based on time since last_seen.
///
/// Returns the number of edges updated and the number pruned (below min_weight).
pub fn apply_decay(config: &Config, decay_config: &DecayConfig) -> Result<DecayResult> {
    with_connection(config, |conn| {
        ensure_schema_inner(conn)?;
        let now_ms = Utc::now().timestamp_millis();
        let half_life_ms = (decay_config.half_life_days * 86_400_000.0) as i64;

        // Load all edges
        let mut stmt = conn.prepare(
            "SELECT edge_id, weight, last_seen_ms FROM mem_graph_edges",
        )?;
        let edges: Vec<(String, f64, i64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut updated = 0usize;
        let mut pruned = 0usize;

        for (edge_id, weight, last_seen_ms) in &edges {
            let elapsed_ms = now_ms - last_seen_ms;
            if elapsed_ms <= 0 {
                continue;
            }

            // Exponential decay: new_weight = weight * 2^(-elapsed / half_life)
            let decay_factor = 2.0_f64.powf(-(elapsed_ms as f64) / (half_life_ms as f64));
            let new_weight = weight * decay_factor;

            if new_weight < decay_config.min_weight && decay_config.min_weight > 0.0 {
                conn.execute("DELETE FROM mem_graph_edges WHERE edge_id = ?1", params![edge_id])?;
                pruned += 1;
            } else if (new_weight - weight).abs() > 0.001 {
                conn.execute(
                    "UPDATE mem_graph_edges SET weight = ?1 WHERE edge_id = ?2",
                    params![new_weight, edge_id],
                )?;
                updated += 1;
            }
        }

        Ok(DecayResult { updated, pruned })
    })
}

/// Result of a decay pass.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecayResult {
    pub updated: usize,
    pub pruned: usize,
}

// ─── Export ──────────────────────────────────────────────────────────────────

/// Export the graph as DOT (Graphviz) format.
pub fn export_dot(config: &Config) -> Result<GraphExport> {
    with_connection(config, |conn| {
        ensure_schema_inner(conn)?;

        let mut dot = String::from("digraph memory_graph {\n  rankdir=LR;\n  node [shape=box];\n\n");

        // Nodes
        let mut stmt = conn.prepare("SELECT node_id, label, entity_type FROM mem_graph_nodes")?;
        let nodes: Vec<(String, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (id, label, etype) in &nodes {
            let escaped_label = label.replace('"', "\\\"");
            dot.push_str(&format!(
                "  \"{}\" [label=\"{}\\n({})\"];\n",
                id, escaped_label, etype
            ));
        }
        dot.push('\n');

        // Edges
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, relation_type, weight FROM mem_graph_edges",
        )?;
        let edges: Vec<(String, String, String, f64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (src, tgt, rel, w) in &edges {
            dot.push_str(&format!(
                "  \"{}\" -> \"{}\" [label=\"{} ({:.1})\"];\n",
                src, tgt, rel, w
            ));
        }
        dot.push_str("}\n");

        Ok(GraphExport {
            format: "dot".into(),
            content: dot,
            node_count: nodes.len(),
            edge_count: edges.len(),
        })
    })
}

/// Export the graph as JSON (nodes + edges arrays).
pub fn export_json(config: &Config) -> Result<GraphExport> {
    with_connection(config, |conn| {
        ensure_schema_inner(conn)?;

        let mut stmt = conn.prepare(
            "SELECT node_id, label, entity_type FROM mem_graph_nodes",
        )?;
        let nodes: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let label: String = row.get(1)?;
                let etype: String = row.get(2)?;
                Ok(serde_json::json!({ "id": id, "label": label, "type": etype }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, relation_type, weight, first_seen_ms, last_seen_ms FROM mem_graph_edges",
        )?;
        let edges: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                let src: String = row.get(0)?;
                let tgt: String = row.get(1)?;
                let rel: String = row.get(2)?;
                let w: f64 = row.get(3)?;
                let fs: i64 = row.get(4)?;
                let ls: i64 = row.get(5)?;
                Ok(serde_json::json!({
                    "source": src, "target": tgt, "relation": rel,
                    "weight": w, "first_seen_ms": fs, "last_seen_ms": ls
                }))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let nc = nodes.len();
        let ec = edges.len();
        let export = serde_json::json!({ "nodes": nodes, "edges": edges });

        Ok(GraphExport {
            format: "json".into(),
            content: serde_json::to_string_pretty(&export).unwrap_or_default(),
            node_count: nc,
            edge_count: ec,
        })
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or_default()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::persistent_store::{upsert_edge, upsert_node, GraphEdgePersistent, GraphNode};
    use tempfile::TempDir;

    fn test_config() -> (TempDir, Config) {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.workspace_dir = tmp.path().to_path_buf();
        (tmp, cfg)
    }

    fn setup_graph(cfg: &Config) {
        let now = Utc::now();
        let old = now - Duration::days(60);
        for (id, label) in [("a", "Alice"), ("b", "Bob"), ("c", "Carol")] {
            upsert_node(cfg, &GraphNode {
                node_id: id.into(),
                entity_type: "Person".into(),
                label: label.into(),
                properties: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            }).unwrap();
        }
        // Recent edge
        upsert_edge(cfg, &GraphEdgePersistent {
            edge_id: String::new(),
            source_id: "a".into(),
            target_id: "b".into(),
            relation_type: "knows".into(),
            weight: 5.0,
            evidence: vec![],
            first_seen: old,
            last_seen: now,
        }).unwrap();
        // Old edge
        upsert_edge(cfg, &GraphEdgePersistent {
            edge_id: String::new(),
            source_id: "b".into(),
            target_id: "c".into(),
            relation_type: "knows".into(),
            weight: 2.0,
            evidence: vec![],
            first_seen: old,
            last_seen: old,
        }).unwrap();
    }

    #[test]
    fn most_active_filters_by_time() {
        let (_tmp, cfg) = test_config();
        setup_graph(&cfg);

        let since = Utc::now() - Duration::days(7);
        let active = most_active_edges(&cfg, since, 10).unwrap();
        // Only the a->b edge was seen recently
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].source_id, "a");
    }

    #[test]
    fn decay_reduces_old_edges() {
        let (_tmp, cfg) = test_config();
        setup_graph(&cfg);

        let result = apply_decay(&cfg, &DecayConfig {
            half_life_days: 30.0,
            min_weight: 0.1,
        }).unwrap();

        // The old edge (60 days, half_life 30) should be decayed: 2.0 * 0.25 = 0.5
        assert!(result.updated > 0 || result.pruned > 0);
    }

    #[test]
    fn export_dot_produces_valid_output() {
        let (_tmp, cfg) = test_config();
        setup_graph(&cfg);

        let export = export_dot(&cfg).unwrap();
        assert!(export.content.starts_with("digraph"));
        assert!(export.content.contains("Alice"));
        assert_eq!(export.node_count, 3);
        assert_eq!(export.edge_count, 2);
    }

    #[test]
    fn export_json_produces_valid_output() {
        let (_tmp, cfg) = test_config();
        setup_graph(&cfg);

        let export = export_json(&cfg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&export.content).unwrap();
        assert_eq!(parsed["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(parsed["edges"].as_array().unwrap().len(), 2);
    }
}
