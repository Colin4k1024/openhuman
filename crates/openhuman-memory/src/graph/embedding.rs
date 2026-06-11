//! Graph embeddings — TransE-based node/relation embedding for similarity search.
//!
//! TransE models relationships as translations in embedding space:
//!   head + relation ≈ tail
//!
//! Training minimizes: ||h + r - t|| for positive triples while maximizing
//! distance for negative (corrupted) triples.
//!
//! After training, each node has a fixed-dimension vector enabling:
//! - Nearest-neighbor node search
//! - Relation prediction (link completion)
//! - Cluster discovery

use anyhow::{Context, Result};
use chrono::Utc;
use rand::Rng;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;

// ─── Schema ──────────────────────────────────────────────────────────────────

const EMBEDDING_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mem_graph_node_embeddings (
    node_id         TEXT PRIMARY KEY,
    embedding       BLOB NOT NULL,
    dimension       INTEGER NOT NULL,
    model_version   INTEGER NOT NULL DEFAULT 0,
    updated_at_ms   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mem_graph_relation_embeddings (
    relation_type   TEXT PRIMARY KEY,
    embedding       BLOB NOT NULL,
    dimension       INTEGER NOT NULL,
    model_version   INTEGER NOT NULL DEFAULT 0,
    updated_at_ms   INTEGER NOT NULL
);
";

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(EMBEDDING_SCHEMA)
        .context("create graph embedding schema")?;
    Ok(())
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// Configuration for TransE training.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransEConfig {
    /// Embedding dimension (default: 64).
    pub dimension: usize,
    /// Learning rate (default: 0.01).
    pub learning_rate: f64,
    /// Margin for ranking loss (default: 1.0).
    pub margin: f64,
    /// Number of training epochs (default: 100).
    pub epochs: usize,
    /// Negative samples per positive triple (default: 5).
    pub negative_samples: usize,
}

impl Default for TransEConfig {
    fn default() -> Self {
        Self {
            dimension: 64,
            learning_rate: 0.01,
            margin: 1.0,
            epochs: 100,
            negative_samples: 5,
        }
    }
}

/// A stored embedding vector for a node.
#[derive(Clone, Debug)]
pub struct NodeEmbedding {
    pub node_id: String,
    pub vector: Vec<f64>,
    pub model_version: u32,
}

/// A neighbor found by vector similarity search.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimilarNode {
    pub node_id: String,
    pub distance: f64,
}

/// A triple (head, relation, tail) used for training.
#[derive(Clone, Debug)]
struct Triple {
    head: String,
    relation: String,
    tail: String,
}

// ─── Training ────────────────────────────────────────────────────────────────

/// Train TransE embeddings from the current graph and persist results.
///
/// Reads all edges from `mem_graph_edges`, trains entity and relation embeddings,
/// then stores them in the embedding tables.
pub fn train_embeddings(config: &Config, transe_config: &TransEConfig) -> Result<TrainResult> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        super::persistent_store::ensure_schema_inner(conn)?;

        // Load triples
        let triples = load_triples(conn)?;
        if triples.is_empty() {
            return Ok(TrainResult {
                nodes_embedded: 0,
                relations_embedded: 0,
                model_version: 0,
            });
        }

        // Collect unique entities and relations
        use std::collections::HashMap;
        let mut entities: Vec<String> = Vec::new();
        let mut relations: Vec<String> = Vec::new();
        {
            let mut entity_set: HashMap<&str, ()> = HashMap::new();
            let mut relation_set: HashMap<&str, ()> = HashMap::new();
            for t in &triples {
                if entity_set.insert(t.head.as_str(), ()).is_none() {
                    entities.push(t.head.clone());
                }
                if entity_set.insert(t.tail.as_str(), ()).is_none() {
                    entities.push(t.tail.clone());
                }
                if relation_set.insert(t.relation.as_str(), ()).is_none() {
                    relations.push(t.relation.clone());
                }
            }
        }
        let entity_idx: HashMap<&str, usize> = entities.iter().enumerate().map(|(i, e)| (e.as_str(), i)).collect();
        let relation_idx: HashMap<&str, usize> = relations.iter().enumerate().map(|(i, r)| (r.as_str(), i)).collect();

        let dim = transe_config.dimension;
        let mut rng = rand::thread_rng();

        // Initialize embeddings randomly (uniform in [-6/sqrt(dim), 6/sqrt(dim)])
        let bound = 6.0 / (dim as f64).sqrt();
        let mut entity_vecs: Vec<Vec<f64>> = entities
            .iter()
            .map(|_| (0..dim).map(|_| rng.gen_range(-bound..bound)).collect())
            .collect();
        let mut relation_vecs: Vec<Vec<f64>> = relations
            .iter()
            .map(|_| (0..dim).map(|_| rng.gen_range(-bound..bound)).collect())
            .collect();

        // Normalize entity vectors
        for v in &mut entity_vecs {
            normalize_vec(v);
        }

        // Training loop
        for _epoch in 0..transe_config.epochs {
            for triple in &triples {
                let h_idx = entity_idx[triple.head.as_str()];
                let t_idx = entity_idx[triple.tail.as_str()];
                let r_idx = relation_idx[triple.relation.as_str()];

                // Positive distance: ||h + r - t||
                let pos_dist = distance(&entity_vecs[h_idx], &relation_vecs[r_idx], &entity_vecs[t_idx]);

                // Negative sampling: corrupt head or tail
                for _ in 0..transe_config.negative_samples {
                    let corrupt_entity = rng.gen_range(0..entities.len());
                    let corrupt_head = rng.gen_bool(0.5);

                    let neg_dist = if corrupt_head {
                        distance(&entity_vecs[corrupt_entity], &relation_vecs[r_idx], &entity_vecs[t_idx])
                    } else {
                        distance(&entity_vecs[h_idx], &relation_vecs[r_idx], &entity_vecs[corrupt_entity])
                    };

                    // Margin-based ranking loss: max(0, margin + pos - neg)
                    let loss = transe_config.margin + pos_dist - neg_dist;
                    if loss > 0.0 {
                        let lr = transe_config.learning_rate;

                        // Gradient update
                        for d in 0..dim {
                            let grad = 2.0 * (entity_vecs[h_idx][d] + relation_vecs[r_idx][d] - entity_vecs[t_idx][d]);
                            entity_vecs[h_idx][d] -= lr * grad;
                            entity_vecs[t_idx][d] += lr * grad;
                            relation_vecs[r_idx][d] -= lr * grad;

                            if corrupt_head {
                                let neg_grad = 2.0 * (entity_vecs[corrupt_entity][d] + relation_vecs[r_idx][d] - entity_vecs[t_idx][d]);
                                entity_vecs[corrupt_entity][d] += lr * neg_grad;
                            } else {
                                let neg_grad = 2.0 * (entity_vecs[h_idx][d] + relation_vecs[r_idx][d] - entity_vecs[corrupt_entity][d]);
                                entity_vecs[corrupt_entity][d] += lr * neg_grad;
                            }
                        }
                    }
                }

                // Re-normalize
                normalize_vec(&mut entity_vecs[h_idx]);
                normalize_vec(&mut entity_vecs[t_idx]);
            }
        }

        // Get next model version
        let model_version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(model_version), 0) + 1 FROM mem_graph_node_embeddings",
                [],
                |r| r.get(0),
            )
            .unwrap_or(1);

        let now_ms = Utc::now().timestamp_millis();

        // Persist entity embeddings
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO mem_graph_node_embeddings (node_id, embedding, dimension, model_version, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (i, entity) in entities.iter().enumerate() {
            let blob = vec_to_blob(&entity_vecs[i]);
            stmt.execute(params![entity, blob, dim as i64, model_version, now_ms])?;
        }

        // Persist relation embeddings
        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO mem_graph_relation_embeddings (relation_type, embedding, dimension, model_version, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (i, rel) in relations.iter().enumerate() {
            let blob = vec_to_blob(&relation_vecs[i]);
            stmt.execute(params![rel, blob, dim as i64, model_version, now_ms])?;
        }

        Ok(TrainResult {
            nodes_embedded: entities.len(),
            relations_embedded: relations.len(),
            model_version,
        })
    })
}

/// Result of a training run.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrainResult {
    pub nodes_embedded: usize,
    pub relations_embedded: usize,
    pub model_version: u32,
}

// ─── Query ───────────────────────────────────────────────────────────────────

/// Find the N most similar nodes to a given node by embedding distance.
pub fn nearest_nodes(config: &Config, node_id: &str, limit: usize) -> Result<Vec<SimilarNode>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;

        // Get the target embedding
        let target_blob: Vec<u8> = conn
            .query_row(
                "SELECT embedding FROM mem_graph_node_embeddings WHERE node_id = ?1",
                params![node_id],
                |r| r.get(0),
            )
            .optional()
            .context("query target embedding")?
            .ok_or_else(|| anyhow::anyhow!("no embedding for node {node_id}"))?;

        let target = blob_to_vec(&target_blob);

        // Load all embeddings (brute-force for small graphs; fine for < 100k nodes)
        let mut stmt = conn.prepare(
            "SELECT node_id, embedding FROM mem_graph_node_embeddings WHERE node_id != ?1",
        )?;
        let mut results: Vec<SimilarNode> = stmt
            .query_map(params![node_id], |row| {
                let nid: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                Ok((nid, blob))
            })?
            .filter_map(|r| r.ok())
            .map(|(nid, blob)| {
                let v = blob_to_vec(&blob);
                let dist = euclidean_distance(&target, &v);
                SimilarNode {
                    node_id: nid,
                    distance: dist,
                }
            })
            .collect();

        results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok(results)
    })
}

/// Get the embedding vector for a node.
pub fn get_node_embedding(config: &Config, node_id: &str) -> Result<Option<NodeEmbedding>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.query_row(
            "SELECT node_id, embedding, model_version FROM mem_graph_node_embeddings WHERE node_id = ?1",
            params![node_id],
            |row| {
                let nid: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                let ver: u32 = row.get(2)?;
                Ok(NodeEmbedding {
                    node_id: nid,
                    vector: blob_to_vec(&blob),
                    model_version: ver,
                })
            },
        )
        .optional()
        .context("get_node_embedding")
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn load_triples(conn: &Connection) -> Result<Vec<Triple>> {
    let mut stmt = conn.prepare(
        "SELECT source_id, relation_type, target_id FROM mem_graph_edges",
    )?;
    let triples = stmt
        .query_map([], |row| {
            Ok(Triple {
                head: row.get(0)?,
                relation: row.get(1)?,
                tail: row.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("load triples")?;
    Ok(triples)
}

fn distance(h: &[f64], r: &[f64], t: &[f64]) -> f64 {
    h.iter()
        .zip(r.iter())
        .zip(t.iter())
        .map(|((hi, ri), ti)| (hi + ri - ti).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(ai, bi)| (ai - bi).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn normalize_vec(v: &mut [f64]) {
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 1e-10 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn vec_to_blob(v: &[f64]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn blob_to_vec(blob: &[u8]) -> Vec<f64> {
    blob.chunks_exact(8)
        .map(|chunk| f64::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

use rusqlite::OptionalExtension;

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

    fn setup_triangle(cfg: &Config) {
        let now = Utc::now();
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
        for (src, rel, tgt) in [("a", "knows", "b"), ("b", "knows", "c"), ("a", "works_with", "c")] {
            upsert_edge(cfg, &GraphEdgePersistent {
                edge_id: String::new(),
                source_id: src.into(),
                target_id: tgt.into(),
                relation_type: rel.into(),
                weight: 1.0,
                evidence: vec![],
                first_seen: now,
                last_seen: now,
            }).unwrap();
        }
    }

    #[test]
    fn train_and_query_embeddings() {
        let (_tmp, cfg) = test_config();
        setup_triangle(&cfg);

        let result = train_embeddings(&cfg, &TransEConfig {
            dimension: 16,
            epochs: 50,
            ..Default::default()
        }).unwrap();

        assert_eq!(result.nodes_embedded, 3);
        assert_eq!(result.relations_embedded, 2);
        assert_eq!(result.model_version, 1);

        // Query nearest nodes to "a"
        let neighbors = nearest_nodes(&cfg, "a", 5).unwrap();
        assert_eq!(neighbors.len(), 2);
        // All distances should be finite
        assert!(neighbors.iter().all(|n| n.distance.is_finite()));
    }

    #[test]
    fn get_embedding_returns_none_for_missing() {
        let (_tmp, cfg) = test_config();
        let result = get_node_embedding(&cfg, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn retrain_increments_version() {
        let (_tmp, cfg) = test_config();
        setup_triangle(&cfg);

        let tc = TransEConfig { dimension: 8, epochs: 10, ..Default::default() };
        let r1 = train_embeddings(&cfg, &tc).unwrap();
        let r2 = train_embeddings(&cfg, &tc).unwrap();
        assert_eq!(r2.model_version, r1.model_version + 1);
    }
}
