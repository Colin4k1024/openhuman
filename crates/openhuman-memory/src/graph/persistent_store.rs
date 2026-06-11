//! Persistent graph store — explicit nodes and edges beyond co-occurrence.
//!
//! This complements the derived co-occurrence graph (query.rs) with a writable
//! graph store for explicitly discovered/asserted relationships:
//! - Employment: "Alice works_at Google"
//! - Collaboration: "Alice collaborates_with Bob"
//! - Topic affinity: "Alice interested_in Rust"
//!
//! Storage: two SQLite tables in the chunks DB (same `with_connection` path).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;

// ─── Schema ──────────────────────────────────────────────────────────────────

const GRAPH_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mem_graph_nodes (
    node_id         TEXT PRIMARY KEY,
    entity_type     TEXT NOT NULL,
    label           TEXT NOT NULL,
    properties      TEXT NOT NULL DEFAULT '{}',
    created_at_ms   INTEGER NOT NULL,
    updated_at_ms   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_graph_nodes_type ON mem_graph_nodes(entity_type);

CREATE TABLE IF NOT EXISTS mem_graph_edges (
    edge_id         TEXT PRIMARY KEY,
    source_id       TEXT NOT NULL,
    target_id       TEXT NOT NULL,
    relation_type   TEXT NOT NULL,
    weight          REAL NOT NULL DEFAULT 1.0,
    evidence        TEXT NOT NULL DEFAULT '[]',
    first_seen_ms   INTEGER NOT NULL,
    last_seen_ms    INTEGER NOT NULL,
    FOREIGN KEY (source_id) REFERENCES mem_graph_nodes(node_id),
    FOREIGN KEY (target_id) REFERENCES mem_graph_nodes(node_id)
);

CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON mem_graph_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON mem_graph_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_relation ON mem_graph_edges(relation_type);
CREATE INDEX IF NOT EXISTS idx_graph_edges_weight ON mem_graph_edges(weight DESC);
";

// ─── Types ───────────────────────────────────────────────────────────────────

/// A node in the persistent knowledge graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub node_id: String,
    pub entity_type: String,
    pub label: String,
    pub properties: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An edge in the persistent knowledge graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEdgePersistent {
    pub edge_id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub weight: f64,
    pub evidence: Vec<EvidenceRef>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
}

/// A reference to the evidence chunk/source that supports an edge.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub chunk_id: String,
    pub snippet: String,
}

// ─── Init ────────────────────────────────────────────────────────────────────

/// Ensure graph tables exist. Called lazily on first graph operation.
pub(crate) fn ensure_schema_inner(conn: &Connection) -> Result<()> {
    ensure_schema(conn)
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(GRAPH_SCHEMA)
        .context("create graph schema")?;
    Ok(())
}

// ─── Node CRUD ───────────────────────────────────────────────────────────────

/// Add or update a node. If node_id exists, updates label/properties/updated_at.
pub fn upsert_node(config: &Config, node: &GraphNode) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "INSERT INTO mem_graph_nodes (node_id, entity_type, label, properties, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(node_id) DO UPDATE SET
                label = excluded.label,
                properties = excluded.properties,
                updated_at_ms = excluded.updated_at_ms",
            params![
                node.node_id,
                node.entity_type,
                node.label,
                node.properties.to_string(),
                node.created_at.timestamp_millis(),
                node.updated_at.timestamp_millis(),
            ],
        )?;
        Ok(())
    })
}

/// Get a node by ID.
pub fn get_node(config: &Config, node_id: &str) -> Result<Option<GraphNode>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.query_row(
            "SELECT node_id, entity_type, label, properties, created_at_ms, updated_at_ms
             FROM mem_graph_nodes WHERE node_id = ?1",
            params![node_id],
            |row| {
                Ok(GraphNode {
                    node_id: row.get(0)?,
                    entity_type: row.get(1)?,
                    label: row.get(2)?,
                    properties: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    created_at: ms_to_dt(row.get(4)?),
                    updated_at: ms_to_dt(row.get(5)?),
                })
            },
        )
        .optional()
        .context("get_node")
    })
}

/// List nodes, optionally filtered by entity_type.
pub fn list_nodes(
    config: &Config,
    entity_type: Option<&str>,
    limit: usize,
) -> Result<Vec<GraphNode>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let (sql, param_values): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match entity_type {
            Some(t) => (
                "SELECT node_id, entity_type, label, properties, created_at_ms, updated_at_ms
                 FROM mem_graph_nodes WHERE entity_type = ?1 ORDER BY updated_at_ms DESC LIMIT ?2",
                vec![Box::new(t.to_string()), Box::new(limit as i64)],
            ),
            None => (
                "SELECT node_id, entity_type, label, properties, created_at_ms, updated_at_ms
                 FROM mem_graph_nodes ORDER BY updated_at_ms DESC LIMIT ?1",
                vec![Box::new(limit as i64)],
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_values.iter()), |row| {
                Ok(GraphNode {
                    node_id: row.get(0)?,
                    entity_type: row.get(1)?,
                    label: row.get(2)?,
                    properties: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    created_at: ms_to_dt(row.get(4)?),
                    updated_at: ms_to_dt(row.get(5)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("list_nodes")?;
        Ok(rows)
    })
}

/// Delete a node and all its edges.
pub fn delete_node(config: &Config, node_id: &str) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "DELETE FROM mem_graph_edges WHERE source_id = ?1 OR target_id = ?1",
            params![node_id],
        )?;
        conn.execute("DELETE FROM mem_graph_nodes WHERE node_id = ?1", params![node_id])?;
        Ok(())
    })
}

// ─── Edge CRUD ───────────────────────────────────────────────────────────────

/// Add or update an edge. If edge_id exists, updates weight/evidence/last_seen.
/// If edge_id is empty, generates one from (source, target, relation).
pub fn upsert_edge(config: &Config, edge: &GraphEdgePersistent) -> Result<()> {
    let edge_id = if edge.edge_id.is_empty() {
        make_edge_id(&edge.source_id, &edge.target_id, &edge.relation_type)
    } else {
        edge.edge_id.clone()
    };
    let evidence_json = serde_json::to_string(&edge.evidence).unwrap_or_else(|_| "[]".into());

    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "INSERT INTO mem_graph_edges (edge_id, source_id, target_id, relation_type, weight, evidence, first_seen_ms, last_seen_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(edge_id) DO UPDATE SET
                weight = excluded.weight,
                evidence = excluded.evidence,
                last_seen_ms = excluded.last_seen_ms",
            params![
                edge_id,
                edge.source_id,
                edge.target_id,
                edge.relation_type,
                edge.weight,
                evidence_json,
                edge.first_seen.timestamp_millis(),
                edge.last_seen.timestamp_millis(),
            ],
        )?;
        Ok(())
    })
}

/// Get edges from a node, optionally filtered by relation_type.
pub fn edges_from(
    config: &Config,
    source_id: &str,
    relation_type: Option<&str>,
    limit: usize,
) -> Result<Vec<GraphEdgePersistent>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        if let Some(rt) = relation_type {
            let mut stmt = conn.prepare(
                "SELECT edge_id, source_id, target_id, relation_type, weight, evidence, first_seen_ms, last_seen_ms
                 FROM mem_graph_edges WHERE source_id = ?1 AND relation_type = ?2
                 ORDER BY weight DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![source_id, rt, limit as i64], row_to_edge)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("edges_from filtered")?;
            Ok(rows)
        } else {
            let mut stmt = conn.prepare(
                "SELECT edge_id, source_id, target_id, relation_type, weight, evidence, first_seen_ms, last_seen_ms
                 FROM mem_graph_edges WHERE source_id = ?1
                 ORDER BY weight DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![source_id, limit as i64], row_to_edge)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("edges_from")?;
            Ok(rows)
        }
    })
}

/// Get all edges involving a node (as source OR target).
pub fn edges_involving(config: &Config, node_id: &str, limit: usize) -> Result<Vec<GraphEdgePersistent>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let mut stmt = conn.prepare(
            "SELECT edge_id, source_id, target_id, relation_type, weight, evidence, first_seen_ms, last_seen_ms
             FROM mem_graph_edges WHERE source_id = ?1 OR target_id = ?1
             ORDER BY weight DESC LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![node_id, limit as i64], row_to_edge)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("edges_involving")?;
        Ok(rows)
    })
}

/// Remove an edge by ID.
pub fn remove_edge(config: &Config, edge_id: &str) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute("DELETE FROM mem_graph_edges WHERE edge_id = ?1", params![edge_id])?;
        Ok(())
    })
}

/// Increment edge weight (or create if not exists).
pub fn increment_edge_weight(
    config: &Config,
    source_id: &str,
    target_id: &str,
    relation_type: &str,
    increment: f64,
) -> Result<()> {
    let edge_id = make_edge_id(source_id, target_id, relation_type);
    let now_ms = Utc::now().timestamp_millis();

    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "INSERT INTO mem_graph_edges (edge_id, source_id, target_id, relation_type, weight, evidence, first_seen_ms, last_seen_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, '[]', ?6, ?6)
             ON CONFLICT(edge_id) DO UPDATE SET
                weight = weight + ?5,
                last_seen_ms = ?6",
            params![edge_id, source_id, target_id, relation_type, increment, now_ms],
        )?;
        Ok(())
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_edge_id(source: &str, target: &str, relation: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(source.as_bytes());
    h.update(b"\0");
    h.update(target.as_bytes());
    h.update(b"\0");
    h.update(relation.as_bytes());
    format!("{:x}", h.finalize())[..24].to_string()
}

fn ms_to_dt(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or_default()
}

fn row_to_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<GraphEdgePersistent> {
    let evidence_str: String = row.get(5)?;
    let evidence: Vec<EvidenceRef> =
        serde_json::from_str(&evidence_str).unwrap_or_default();
    Ok(GraphEdgePersistent {
        edge_id: row.get(0)?,
        source_id: row.get(1)?,
        target_id: row.get(2)?,
        relation_type: row.get(3)?,
        weight: row.get(4)?,
        evidence,
        first_seen: ms_to_dt(row.get(6)?),
        last_seen: ms_to_dt(row.get(7)?),
    })
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

    fn make_node(id: &str, entity_type: &str, label: &str) -> GraphNode {
        let now = Utc::now();
        GraphNode {
            node_id: id.into(),
            entity_type: entity_type.into(),
            label: label.into(),
            properties: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn node_upsert_and_get() {
        let (_tmp, cfg) = test_config();
        let node = make_node("person:alice", "Person", "Alice");
        upsert_node(&cfg, &node).unwrap();

        let fetched = get_node(&cfg, "person:alice").unwrap().unwrap();
        assert_eq!(fetched.label, "Alice");
        assert_eq!(fetched.entity_type, "Person");
    }

    #[test]
    fn node_update_preserves_created_at() {
        let (_tmp, cfg) = test_config();
        let mut node = make_node("person:bob", "Person", "Bob");
        upsert_node(&cfg, &node).unwrap();

        node.label = "Robert".into();
        node.updated_at = Utc::now();
        upsert_node(&cfg, &node).unwrap();

        let fetched = get_node(&cfg, "person:bob").unwrap().unwrap();
        assert_eq!(fetched.label, "Robert");
    }

    #[test]
    fn edge_upsert_and_query() {
        let (_tmp, cfg) = test_config();
        upsert_node(&cfg, &make_node("person:alice", "Person", "Alice")).unwrap();
        upsert_node(&cfg, &make_node("org:google", "Organization", "Google")).unwrap();

        let edge = GraphEdgePersistent {
            edge_id: String::new(),
            source_id: "person:alice".into(),
            target_id: "org:google".into(),
            relation_type: "works_at".into(),
            weight: 1.0,
            evidence: vec![EvidenceRef {
                chunk_id: "chunk-1".into(),
                snippet: "Alice works at Google".into(),
            }],
            first_seen: Utc::now(),
            last_seen: Utc::now(),
        };
        upsert_edge(&cfg, &edge).unwrap();

        let edges = edges_from(&cfg, "person:alice", None, 10).unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target_id, "org:google");
        assert_eq!(edges[0].relation_type, "works_at");
    }

    #[test]
    fn increment_weight_creates_and_updates() {
        let (_tmp, cfg) = test_config();
        upsert_node(&cfg, &make_node("a", "Person", "A")).unwrap();
        upsert_node(&cfg, &make_node("b", "Person", "B")).unwrap();

        increment_edge_weight(&cfg, "a", "b", "co_occurs_with", 1.0).unwrap();
        increment_edge_weight(&cfg, "a", "b", "co_occurs_with", 1.0).unwrap();
        increment_edge_weight(&cfg, "a", "b", "co_occurs_with", 1.0).unwrap();

        let edges = edges_from(&cfg, "a", Some("co_occurs_with"), 10).unwrap();
        assert_eq!(edges.len(), 1);
        assert!((edges[0].weight - 3.0).abs() < 0.01);
    }

    #[test]
    fn delete_node_cascades_edges() {
        let (_tmp, cfg) = test_config();
        upsert_node(&cfg, &make_node("x", "Person", "X")).unwrap();
        upsert_node(&cfg, &make_node("y", "Person", "Y")).unwrap();
        increment_edge_weight(&cfg, "x", "y", "knows", 1.0).unwrap();

        delete_node(&cfg, "x").unwrap();

        assert!(get_node(&cfg, "x").unwrap().is_none());
        let edges = edges_involving(&cfg, "x", 10).unwrap();
        assert!(edges.is_empty());
    }

    #[test]
    fn list_nodes_filters_by_type() {
        let (_tmp, cfg) = test_config();
        upsert_node(&cfg, &make_node("p1", "Person", "P1")).unwrap();
        upsert_node(&cfg, &make_node("p2", "Person", "P2")).unwrap();
        upsert_node(&cfg, &make_node("o1", "Organization", "O1")).unwrap();

        let people = list_nodes(&cfg, Some("Person"), 100).unwrap();
        assert_eq!(people.len(), 2);

        let all = list_nodes(&cfg, None, 100).unwrap();
        assert_eq!(all.len(), 3);
    }
}
