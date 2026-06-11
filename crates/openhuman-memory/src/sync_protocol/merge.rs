//! Conflict resolution — CRDT-inspired merge strategies for memory data.
//!
//! Strategies per store type:
//! - **KV**: Last-Writer-Wins (LWW) using Lamport timestamp + device_id tiebreak.
//! - **Documents**: merge by namespace+key, keep highest updated_at.
//! - **Chunks**: idempotent by id (same id = same chunk, skip).
//! - **Entities**: union merge (same entity, different handles → combine).
//! - **Unresolvable**: logged to `mem_conflicts` for user resolution.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;
use super::changelog::ChangeEntry;

// ─── Schema ──────────────────────────────────────────────────────────────────

const CONFLICTS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mem_conflicts (
    conflict_id     TEXT PRIMARY KEY,
    store           TEXT NOT NULL,
    key             TEXT NOT NULL,
    local_value     TEXT,
    remote_value    TEXT,
    local_ts        INTEGER NOT NULL,
    remote_ts       INTEGER NOT NULL,
    local_device    TEXT NOT NULL,
    remote_device   TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    resolution      TEXT,
    created_at_ms   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conflicts_status ON mem_conflicts(status);
";

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(CONFLICTS_SCHEMA)
        .context("create conflicts schema")?;
    Ok(())
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// Strategy for resolving a conflict between local and remote changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// Last-Writer-Wins: highest Lamport ts wins; device_id lexicographic tiebreak.
    Lww,
    /// Keep the value with the most recent wall-clock time (updated_at).
    LatestTimestamp,
    /// Both values are valid; merge into a union (e.g., entity handles).
    UnionMerge,
    /// Same ID means same content; skip the remote entry.
    IdempotentById,
    /// Cannot auto-resolve; log for user.
    Manual,
}

/// The outcome of merging a single change entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MergeOutcome {
    /// Remote wins — apply the remote change.
    AcceptRemote,
    /// Local wins — discard the remote change.
    KeepLocal,
    /// Both merged (union of values).
    Merged { merged_value: serde_json::Value },
    /// Skipped (idempotent, already have it).
    Skipped,
    /// Conflict logged for manual resolution.
    Conflict { conflict_id: String },
}

/// A stored conflict awaiting user resolution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConflictRecord {
    pub conflict_id: String,
    pub store: String,
    pub key: String,
    pub local_value: Option<serde_json::Value>,
    pub remote_value: Option<serde_json::Value>,
    pub local_ts: u64,
    pub remote_ts: u64,
    pub local_device: String,
    pub remote_device: String,
    pub status: String,
    pub resolution: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Result of merging a batch of changes.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MergeBatchResult {
    pub accepted: usize,
    pub kept_local: usize,
    pub merged: usize,
    pub skipped: usize,
    pub conflicts: usize,
}

// ─── Core merge logic ────────────────────────────────────────────────────────

/// Determine the merge strategy for a given store type.
pub fn strategy_for_store(store: &str) -> MergeStrategy {
    match store {
        "kv" => MergeStrategy::Lww,
        "documents" => MergeStrategy::LatestTimestamp,
        "chunks" => MergeStrategy::IdempotentById,
        "entities" => MergeStrategy::UnionMerge,
        _ => MergeStrategy::Manual,
    }
}

/// Resolve a single conflict between local and remote state.
pub fn resolve_conflict(
    strategy: &MergeStrategy,
    local_ts: u64,
    remote_ts: u64,
    local_device: &str,
    remote_device: &str,
    local_value: Option<&serde_json::Value>,
    remote_value: Option<&serde_json::Value>,
) -> MergeOutcome {
    match strategy {
        MergeStrategy::Lww => {
            if remote_ts > local_ts {
                MergeOutcome::AcceptRemote
            } else if remote_ts < local_ts {
                MergeOutcome::KeepLocal
            } else {
                // Tiebreak: lexicographic device_id comparison
                if remote_device > local_device {
                    MergeOutcome::AcceptRemote
                } else {
                    MergeOutcome::KeepLocal
                }
            }
        }
        MergeStrategy::LatestTimestamp => {
            // For documents: compare updated_at from payload
            if remote_ts > local_ts {
                MergeOutcome::AcceptRemote
            } else {
                MergeOutcome::KeepLocal
            }
        }
        MergeStrategy::IdempotentById => {
            MergeOutcome::Skipped
        }
        MergeStrategy::UnionMerge => {
            // Merge arrays from both values
            let merged = match (local_value, remote_value) {
                (Some(local), Some(remote)) => union_json_arrays(local, remote),
                (None, Some(remote)) => remote.clone(),
                (Some(local), None) => local.clone(),
                (None, None) => serde_json::Value::Null,
            };
            MergeOutcome::Merged { merged_value: merged }
        }
        MergeStrategy::Manual => {
            MergeOutcome::Conflict {
                conflict_id: format!("conflict-{}", uuid::Uuid::new_v4().simple()),
            }
        }
    }
}

/// Merge a batch of remote changes against local state.
///
/// For each entry, determines strategy, resolves, and logs conflicts.
pub fn merge_batch(
    config: &Config,
    local_device: &str,
    remote_entries: &[ChangeEntry],
    local_clock: u64,
) -> Result<MergeBatchResult> {
    let mut result = MergeBatchResult::default();

    for entry in remote_entries {
        let store = entry
            .payload
            .get("store")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let strategy = strategy_for_store(store);

        let outcome = resolve_conflict(
            &strategy,
            local_clock,
            entry.lamport_ts,
            local_device,
            &entry.device_id,
            None, // In a real impl, look up local value by key
            entry.payload.get("value"),
        );

        match outcome {
            MergeOutcome::AcceptRemote => result.accepted += 1,
            MergeOutcome::KeepLocal => result.kept_local += 1,
            MergeOutcome::Merged { .. } => result.merged += 1,
            MergeOutcome::Skipped => result.skipped += 1,
            MergeOutcome::Conflict { conflict_id } => {
                log_conflict(config, &conflict_id, store, entry, local_device, local_clock)?;
                result.conflicts += 1;
            }
        }
    }

    Ok(result)
}

// ─── Conflict log ────────────────────────────────────────────────────────────

/// Log an unresolvable conflict for user resolution.
fn log_conflict(
    config: &Config,
    conflict_id: &str,
    store: &str,
    remote_entry: &ChangeEntry,
    local_device: &str,
    local_ts: u64,
) -> Result<()> {
    let key = remote_entry
        .payload
        .get("key")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO mem_conflicts
             (conflict_id, store, key, local_value, remote_value, local_ts, remote_ts, local_device, remote_device, status, created_at_ms)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, 'pending', ?9)",
            params![
                conflict_id,
                store,
                key,
                remote_entry.payload.to_string(),
                local_ts,
                remote_entry.lamport_ts,
                local_device,
                remote_entry.device_id,
                Utc::now().timestamp_millis(),
            ],
        )?;
        Ok(())
    })
}

/// List pending conflicts.
pub fn list_conflicts(config: &Config, limit: usize) -> Result<Vec<ConflictRecord>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let mut stmt = conn.prepare(
            "SELECT conflict_id, store, key, local_value, remote_value, local_ts, remote_ts, local_device, remote_device, status, resolution, created_at_ms
             FROM mem_conflicts WHERE status = 'pending' ORDER BY created_at_ms DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ConflictRecord {
                    conflict_id: row.get(0)?,
                    store: row.get(1)?,
                    key: row.get(2)?,
                    local_value: row.get::<_, Option<String>>(3)?.and_then(|s| serde_json::from_str(&s).ok()),
                    remote_value: row.get::<_, Option<String>>(4)?.and_then(|s| serde_json::from_str(&s).ok()),
                    local_ts: row.get(5)?,
                    remote_ts: row.get(6)?,
                    local_device: row.get(7)?,
                    remote_device: row.get(8)?,
                    status: row.get(9)?,
                    resolution: row.get(10)?,
                    created_at: DateTime::from_timestamp_millis(row.get(11)?).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("list_conflicts")?;
        Ok(rows)
    })
}

/// Resolve a conflict (user picks local, remote, or custom).
pub fn resolve_conflict_record(
    config: &Config,
    conflict_id: &str,
    resolution: &str,
) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "UPDATE mem_conflicts SET status = 'resolved', resolution = ?1 WHERE conflict_id = ?2",
            params![resolution, conflict_id],
        )?;
        Ok(())
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Union two JSON values — if both are arrays, concatenate unique items.
fn union_json_arrays(a: &serde_json::Value, b: &serde_json::Value) -> serde_json::Value {
    match (a.as_array(), b.as_array()) {
        (Some(arr_a), Some(arr_b)) => {
            let mut merged = arr_a.clone();
            for item in arr_b {
                if !merged.contains(item) {
                    merged.push(item.clone());
                }
            }
            serde_json::Value::Array(merged)
        }
        _ => {
            // If not both arrays, prefer remote (b)
            b.clone()
        }
    }
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

    #[test]
    fn lww_remote_wins_higher_ts() {
        let outcome = resolve_conflict(
            &MergeStrategy::Lww, 5, 10, "dev-a", "dev-b", None, None,
        );
        assert!(matches!(outcome, MergeOutcome::AcceptRemote));
    }

    #[test]
    fn lww_local_wins_higher_ts() {
        let outcome = resolve_conflict(
            &MergeStrategy::Lww, 10, 5, "dev-a", "dev-b", None, None,
        );
        assert!(matches!(outcome, MergeOutcome::KeepLocal));
    }

    #[test]
    fn lww_tiebreak_by_device_id() {
        let outcome = resolve_conflict(
            &MergeStrategy::Lww, 5, 5, "aaa", "zzz", None, None,
        );
        assert!(matches!(outcome, MergeOutcome::AcceptRemote));

        let outcome = resolve_conflict(
            &MergeStrategy::Lww, 5, 5, "zzz", "aaa", None, None,
        );
        assert!(matches!(outcome, MergeOutcome::KeepLocal));
    }

    #[test]
    fn idempotent_always_skips() {
        let outcome = resolve_conflict(
            &MergeStrategy::IdempotentById, 1, 1, "a", "b", None, None,
        );
        assert!(matches!(outcome, MergeOutcome::Skipped));
    }

    #[test]
    fn union_merge_combines_arrays() {
        let local = serde_json::json!(["handle1", "handle2"]);
        let remote = serde_json::json!(["handle2", "handle3"]);
        let outcome = resolve_conflict(
            &MergeStrategy::UnionMerge, 1, 2, "a", "b",
            Some(&local), Some(&remote),
        );
        if let MergeOutcome::Merged { merged_value } = outcome {
            let arr = merged_value.as_array().unwrap();
            assert_eq!(arr.len(), 3);
        } else {
            panic!("expected Merged");
        }
    }

    #[test]
    fn strategy_selection() {
        assert_eq!(strategy_for_store("kv"), MergeStrategy::Lww);
        assert_eq!(strategy_for_store("documents"), MergeStrategy::LatestTimestamp);
        assert_eq!(strategy_for_store("chunks"), MergeStrategy::IdempotentById);
        assert_eq!(strategy_for_store("entities"), MergeStrategy::UnionMerge);
        assert_eq!(strategy_for_store("unknown_store"), MergeStrategy::Manual);
    }

    #[test]
    fn conflict_logging_and_resolution() {
        let (_tmp, cfg) = test_config();
        let entry = ChangeEntry {
            entry_id: "e1".into(),
            lamport_ts: 10,
            device_id: "dev-remote".into(),
            op_type: super::super::changelog::OpType::Update,
            payload: serde_json::json!({"store": "custom", "key": "k1", "value": "v1"}),
            created_at: Utc::now(),
        };

        log_conflict(&cfg, "c1", "custom", &entry, "dev-local", 5).unwrap();

        let conflicts = list_conflicts(&cfg, 10).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_id, "c1");

        resolve_conflict_record(&cfg, "c1", "keep_remote").unwrap();
        let conflicts = list_conflicts(&cfg, 10).unwrap();
        assert_eq!(conflicts.len(), 0); // No more pending
    }
}
