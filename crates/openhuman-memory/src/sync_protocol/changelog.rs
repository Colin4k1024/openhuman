//! Change log — Lamport-timestamped operations for sync.
//!
//! Every memory mutation (insert, update, delete) generates a `ChangeEntry`
//! stored in `mem_change_log`. Remote peers call `get_changes_since(ts)` to
//! pull deltas for replication.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;

// ─── Schema ──────────────────────────────────────────────────────────────────

const CHANGELOG_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mem_change_log (
    entry_id        TEXT PRIMARY KEY,
    lamport_ts      INTEGER NOT NULL,
    device_id       TEXT NOT NULL,
    op_type         TEXT NOT NULL,
    payload         TEXT NOT NULL,
    created_at_ms   INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_change_log_lamport ON mem_change_log(lamport_ts);
CREATE INDEX IF NOT EXISTS idx_change_log_device ON mem_change_log(device_id);

-- Persistent Lamport clock counter
CREATE TABLE IF NOT EXISTS mem_lamport_clock (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    current_ts      INTEGER NOT NULL DEFAULT 0
);

INSERT OR IGNORE INTO mem_lamport_clock (id, current_ts) VALUES (1, 0);
";

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(CHANGELOG_SCHEMA)
        .context("create changelog schema")?;
    Ok(())
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// The type of mutation that occurred.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpType {
    Insert,
    Update,
    Delete,
}

impl OpType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "insert" => Self::Insert,
            "update" => Self::Update,
            "delete" => Self::Delete,
            _ => Self::Update,
        }
    }
}

/// A single change entry in the log.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub entry_id: String,
    pub lamport_ts: u64,
    pub device_id: String,
    pub op_type: OpType,
    /// JSON payload describing the mutation (table, key, old/new values).
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Payload for a change entry — describes what was mutated.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangePayload {
    /// Which store/table the change applies to.
    pub store: String,
    /// Primary key of the affected record.
    pub key: String,
    /// Namespace (for documents/chunks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// The new value (for insert/update), absent for delete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

// ─── Clock ───────────────────────────────────────────────────────────────────

/// Increment the Lamport clock and return the new timestamp.
fn tick(conn: &Connection) -> Result<u64> {
    conn.execute("UPDATE mem_lamport_clock SET current_ts = current_ts + 1 WHERE id = 1", [])?;
    let ts: u64 = conn.query_row(
        "SELECT current_ts FROM mem_lamport_clock WHERE id = 1",
        [],
        |r| r.get(0),
    )?;
    Ok(ts)
}

/// Merge a remote clock value: set local = max(local, remote) + 1.
pub fn merge_clock(config: &Config, remote_ts: u64) -> Result<u64> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let current: u64 = conn.query_row(
            "SELECT current_ts FROM mem_lamport_clock WHERE id = 1",
            [],
            |r| r.get(0),
        )?;
        let merged = current.max(remote_ts) + 1;
        conn.execute(
            "UPDATE mem_lamport_clock SET current_ts = ?1 WHERE id = 1",
            params![merged],
        )?;
        Ok(merged)
    })
}

/// Get the current Lamport timestamp without incrementing.
pub fn current_clock(config: &Config) -> Result<u64> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let ts: u64 = conn.query_row(
            "SELECT current_ts FROM mem_lamport_clock WHERE id = 1",
            [],
            |r| r.get(0),
        )?;
        Ok(ts)
    })
}

// ─── Write ───────────────────────────────────────────────────────────────────

/// Record a change entry. Automatically increments the Lamport clock.
pub fn record_change(
    config: &Config,
    device_id: &str,
    op_type: OpType,
    payload: &ChangePayload,
) -> Result<ChangeEntry> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let lamport_ts = tick(conn)?;
        let entry_id = format!("{}-{}-{}", device_id, lamport_ts, uuid::Uuid::new_v4().simple());
        let now = Utc::now();
        let payload_json = serde_json::to_value(payload).unwrap_or_default();

        conn.execute(
            "INSERT INTO mem_change_log (entry_id, lamport_ts, device_id, op_type, payload, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry_id,
                lamport_ts,
                device_id,
                op_type.as_str(),
                payload_json.to_string(),
                now.timestamp_millis(),
            ],
        )?;

        Ok(ChangeEntry {
            entry_id,
            lamport_ts,
            device_id: device_id.to_string(),
            op_type,
            payload: payload_json,
            created_at: now,
        })
    })
}

// ─── Read ────────────────────────────────────────────────────────────────────

/// Get all changes with lamport_ts > `since_ts`, ordered by lamport_ts.
pub fn get_changes_since(config: &Config, since_ts: u64) -> Result<Vec<ChangeEntry>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let mut stmt = conn.prepare(
            "SELECT entry_id, lamport_ts, device_id, op_type, payload, created_at_ms
             FROM mem_change_log
             WHERE lamport_ts > ?1
             ORDER BY lamport_ts ASC",
        )?;
        let rows = stmt
            .query_map(params![since_ts], |row| {
                let payload_str: String = row.get(4)?;
                Ok(ChangeEntry {
                    entry_id: row.get(0)?,
                    lamport_ts: row.get(1)?,
                    device_id: row.get(2)?,
                    op_type: OpType::from_str(&row.get::<_, String>(3)?),
                    payload: serde_json::from_str(&payload_str).unwrap_or_default(),
                    created_at: DateTime::from_timestamp_millis(row.get(5)?).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("get_changes_since")?;
        Ok(rows)
    })
}

/// Get changes from a specific device since a given timestamp.
pub fn get_changes_from_device(
    config: &Config,
    device_id: &str,
    since_ts: u64,
) -> Result<Vec<ChangeEntry>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let mut stmt = conn.prepare(
            "SELECT entry_id, lamport_ts, device_id, op_type, payload, created_at_ms
             FROM mem_change_log
             WHERE device_id = ?1 AND lamport_ts > ?2
             ORDER BY lamport_ts ASC",
        )?;
        let rows = stmt
            .query_map(params![device_id, since_ts], |row| {
                let payload_str: String = row.get(4)?;
                Ok(ChangeEntry {
                    entry_id: row.get(0)?,
                    lamport_ts: row.get(1)?,
                    device_id: row.get(2)?,
                    op_type: OpType::from_str(&row.get::<_, String>(3)?),
                    payload: serde_json::from_str(&payload_str).unwrap_or_default(),
                    created_at: DateTime::from_timestamp_millis(row.get(5)?).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("get_changes_from_device")?;
        Ok(rows)
    })
}

/// Count total entries in the change log.
pub fn change_log_count(config: &Config) -> Result<u64> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let count: u64 = conn.query_row(
            "SELECT COUNT(*) FROM mem_change_log",
            [],
            |r| r.get(0),
        )?;
        Ok(count)
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

    #[test]
    fn record_and_retrieve_changes() {
        let (_tmp, cfg) = test_config();

        let payload = ChangePayload {
            store: "documents".into(),
            key: "doc-1".into(),
            namespace: Some("default".into()),
            value: Some(serde_json::json!({"title": "Hello"})),
        };

        let e1 = record_change(&cfg, "device-a", OpType::Insert, &payload).unwrap();
        let e2 = record_change(&cfg, "device-a", OpType::Update, &payload).unwrap();

        assert_eq!(e1.lamport_ts, 1);
        assert_eq!(e2.lamport_ts, 2);

        let changes = get_changes_since(&cfg, 0).unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].op_type, OpType::Insert);
        assert_eq!(changes[1].op_type, OpType::Update);
    }

    #[test]
    fn get_changes_since_filters() {
        let (_tmp, cfg) = test_config();
        let payload = ChangePayload {
            store: "kv".into(),
            key: "k1".into(),
            namespace: None,
            value: None,
        };

        record_change(&cfg, "d1", OpType::Insert, &payload).unwrap();
        record_change(&cfg, "d1", OpType::Update, &payload).unwrap();
        record_change(&cfg, "d1", OpType::Delete, &payload).unwrap();

        let changes = get_changes_since(&cfg, 2).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].lamport_ts, 3);
    }

    #[test]
    fn merge_clock_advances() {
        let (_tmp, cfg) = test_config();
        let payload = ChangePayload {
            store: "kv".into(),
            key: "k".into(),
            namespace: None,
            value: None,
        };
        record_change(&cfg, "d1", OpType::Insert, &payload).unwrap(); // ts=1

        // Remote has ts=100
        let merged = merge_clock(&cfg, 100).unwrap();
        assert_eq!(merged, 101);

        // Next local write should be 102
        let e = record_change(&cfg, "d1", OpType::Insert, &payload).unwrap();
        assert_eq!(e.lamport_ts, 102);
    }

    #[test]
    fn filter_by_device() {
        let (_tmp, cfg) = test_config();
        let payload = ChangePayload {
            store: "chunks".into(),
            key: "c1".into(),
            namespace: None,
            value: None,
        };
        record_change(&cfg, "device-a", OpType::Insert, &payload).unwrap();
        record_change(&cfg, "device-b", OpType::Insert, &payload).unwrap();
        record_change(&cfg, "device-a", OpType::Update, &payload).unwrap();

        let from_a = get_changes_from_device(&cfg, "device-a", 0).unwrap();
        assert_eq!(from_a.len(), 2);

        let from_b = get_changes_from_device(&cfg, "device-b", 0).unwrap();
        assert_eq!(from_b.len(), 1);
    }
}
