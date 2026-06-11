//! Local pattern extraction — discover behavioral patterns from user memory.
//!
//! Pattern types:
//! - **Temporal**: event A tends to follow event B.
//! - **Preference**: user prefers approach Y in context X.
//! - **Knowledge**: user has depth in certain domains.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;

// ─── Schema ──────────────────────────────────────────────────────────────────

const PATTERNS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mem_patterns (
    pattern_id      TEXT PRIMARY KEY,
    pattern_type    TEXT NOT NULL,
    trigger_context TEXT NOT NULL,
    action_taken    TEXT NOT NULL,
    frequency       INTEGER NOT NULL DEFAULT 1,
    confidence      REAL NOT NULL DEFAULT 0.0,
    first_seen_ms   INTEGER NOT NULL,
    last_seen_ms    INTEGER NOT NULL,
    metadata        TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_patterns_type ON mem_patterns(pattern_type);
CREATE INDEX IF NOT EXISTS idx_patterns_confidence ON mem_patterns(confidence DESC);
";

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(PATTERNS_SCHEMA)
        .context("create patterns schema")?;
    Ok(())
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// A behavioral pattern extracted from user memory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    /// Context that triggers this pattern (e.g., "after meeting", "when coding in Rust").
    pub trigger_context: String,
    /// The action/behavior that follows (e.g., "writes summary", "uses iterators").
    pub action_taken: String,
    /// How many times this pattern was observed.
    pub frequency: u32,
    /// Confidence score (0.0–1.0): frequency × consistency.
    pub confidence: f64,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

/// Classification of pattern type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternType {
    /// Temporal: A happens after B.
    Temporal,
    /// Preference: user prefers X over Y.
    Preference,
    /// Knowledge: user demonstrates understanding of topic.
    Knowledge,
}

impl PatternType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Temporal => "temporal",
            Self::Preference => "preference",
            Self::Knowledge => "knowledge",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "temporal" => Self::Temporal,
            "preference" => Self::Preference,
            "knowledge" => Self::Knowledge,
            _ => Self::Temporal,
        }
    }
}

/// Input for registering a pattern observation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternObservation {
    pub pattern_type: PatternType,
    pub trigger_context: String,
    pub action_taken: String,
    pub metadata: Option<serde_json::Value>,
}

// ─── Operations ──────────────────────────────────────────────────────────────

/// Record a pattern observation. Creates or increments an existing pattern.
pub fn observe_pattern(config: &Config, observation: &PatternObservation) -> Result<Pattern> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;

        let pattern_id = make_pattern_id(
            &observation.pattern_type,
            &observation.trigger_context,
            &observation.action_taken,
        );
        let now_ms = Utc::now().timestamp_millis();

        // Try to increment existing
        let existing: Option<(u32, i64)> = conn
            .query_row(
                "SELECT frequency, first_seen_ms FROM mem_patterns WHERE pattern_id = ?1",
                params![pattern_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let (frequency, first_seen_ms) = if let Some((freq, fs)) = existing {
            let new_freq = freq + 1;
            conn.execute(
                "UPDATE mem_patterns SET frequency = ?1, last_seen_ms = ?2, confidence = ?3
                 WHERE pattern_id = ?4",
                params![new_freq, now_ms, compute_confidence(new_freq), pattern_id],
            )?;
            (new_freq, fs)
        } else {
            let meta = observation.metadata.as_ref().map(|m| m.to_string()).unwrap_or_else(|| "{}".into());
            conn.execute(
                "INSERT INTO mem_patterns (pattern_id, pattern_type, trigger_context, action_taken, frequency, confidence, first_seen_ms, last_seen_ms, metadata)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?6, ?7)",
                params![
                    pattern_id,
                    observation.pattern_type.as_str(),
                    observation.trigger_context,
                    observation.action_taken,
                    compute_confidence(1),
                    now_ms,
                    meta,
                ],
            )?;
            (1, now_ms)
        };

        Ok(Pattern {
            pattern_id,
            pattern_type: observation.pattern_type.clone(),
            trigger_context: observation.trigger_context.clone(),
            action_taken: observation.action_taken.clone(),
            frequency,
            confidence: compute_confidence(frequency),
            first_seen: DateTime::from_timestamp_millis(first_seen_ms).unwrap_or_default(),
            last_seen: DateTime::from_timestamp_millis(now_ms).unwrap_or_default(),
            metadata: observation.metadata.clone().unwrap_or_default(),
        })
    })
}

/// List patterns by type, ordered by confidence.
pub fn list_patterns(
    config: &Config,
    pattern_type: Option<&PatternType>,
    min_confidence: f64,
    limit: usize,
) -> Result<Vec<Pattern>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let (sql, param_values): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match pattern_type {
            Some(pt) => (
                "SELECT pattern_id, pattern_type, trigger_context, action_taken, frequency, confidence, first_seen_ms, last_seen_ms, metadata
                 FROM mem_patterns WHERE pattern_type = ?1 AND confidence >= ?2
                 ORDER BY confidence DESC LIMIT ?3",
                vec![
                    Box::new(pt.as_str().to_string()),
                    Box::new(min_confidence),
                    Box::new(limit as i64),
                ],
            ),
            None => (
                "SELECT pattern_id, pattern_type, trigger_context, action_taken, frequency, confidence, first_seen_ms, last_seen_ms, metadata
                 FROM mem_patterns WHERE confidence >= ?1
                 ORDER BY confidence DESC LIMIT ?2",
                vec![Box::new(min_confidence), Box::new(limit as i64)],
            ),
        };

        let mut stmt = conn.prepare(sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(param_values.iter()), |row| {
                let meta_str: String = row.get(8)?;
                Ok(Pattern {
                    pattern_id: row.get(0)?,
                    pattern_type: PatternType::from_str(&row.get::<_, String>(1)?),
                    trigger_context: row.get(2)?,
                    action_taken: row.get(3)?,
                    frequency: row.get(4)?,
                    confidence: row.get(5)?,
                    first_seen: DateTime::from_timestamp_millis(row.get(6)?).unwrap_or_default(),
                    last_seen: DateTime::from_timestamp_millis(row.get(7)?).unwrap_or_default(),
                    metadata: serde_json::from_str(&meta_str).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("list_patterns")?;
        Ok(rows)
    })
}

/// Get total pattern count.
pub fn pattern_count(config: &Config) -> Result<u64> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let count: u64 = conn.query_row("SELECT COUNT(*) FROM mem_patterns", [], |r| r.get(0))?;
        Ok(count)
    })
}

/// Delete patterns below a confidence threshold (cleanup).
pub fn prune_weak_patterns(config: &Config, min_confidence: f64) -> Result<usize> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let deleted = conn.execute(
            "DELETE FROM mem_patterns WHERE confidence < ?1",
            params![min_confidence],
        )?;
        Ok(deleted)
    })
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_pattern_id(ptype: &PatternType, trigger: &str, action: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(ptype.as_str().as_bytes());
    h.update(b"\0");
    h.update(trigger.as_bytes());
    h.update(b"\0");
    h.update(action.as_bytes());
    format!("pat-{}", &hex::encode(h.finalize())[..16])
}

/// Confidence = sigmoid(frequency - 3) — ramps from ~0.05 at freq=1 to ~0.95 at freq=6+.
fn compute_confidence(frequency: u32) -> f64 {
    let x = frequency as f64 - 3.0;
    1.0 / (1.0 + (-x).exp())
}

use rusqlite::OptionalExtension;

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
    fn observe_creates_pattern() {
        let (_tmp, cfg) = test_config();
        let obs = PatternObservation {
            pattern_type: PatternType::Temporal,
            trigger_context: "after standup".into(),
            action_taken: "checks email".into(),
            metadata: None,
        };

        let p = observe_pattern(&cfg, &obs).unwrap();
        assert_eq!(p.frequency, 1);
        assert!(p.confidence > 0.0);
        assert_eq!(p.pattern_type, PatternType::Temporal);
    }

    #[test]
    fn repeated_observation_increments() {
        let (_tmp, cfg) = test_config();
        let obs = PatternObservation {
            pattern_type: PatternType::Preference,
            trigger_context: "writing Rust".into(),
            action_taken: "uses iterators over loops".into(),
            metadata: None,
        };

        observe_pattern(&cfg, &obs).unwrap();
        observe_pattern(&cfg, &obs).unwrap();
        let p = observe_pattern(&cfg, &obs).unwrap();

        assert_eq!(p.frequency, 3);
        assert!(p.confidence > 0.4); // sigmoid(0) = 0.5
    }

    #[test]
    fn list_patterns_filters_by_type() {
        let (_tmp, cfg) = test_config();

        for _ in 0..5 {
            observe_pattern(&cfg, &PatternObservation {
                pattern_type: PatternType::Knowledge,
                trigger_context: "Rust topic".into(),
                action_taken: "deep explanation".into(),
                metadata: None,
            }).unwrap();
        }
        observe_pattern(&cfg, &PatternObservation {
            pattern_type: PatternType::Temporal,
            trigger_context: "morning".into(),
            action_taken: "reviews PRs".into(),
            metadata: None,
        }).unwrap();

        let knowledge = list_patterns(&cfg, Some(&PatternType::Knowledge), 0.0, 10).unwrap();
        assert_eq!(knowledge.len(), 1);
        assert_eq!(knowledge[0].frequency, 5);

        let all = list_patterns(&cfg, None, 0.0, 10).unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn prune_removes_weak_patterns() {
        let (_tmp, cfg) = test_config();

        // Single observation = low confidence
        observe_pattern(&cfg, &PatternObservation {
            pattern_type: PatternType::Temporal,
            trigger_context: "rare event".into(),
            action_taken: "rare action".into(),
            metadata: None,
        }).unwrap();

        // Many observations = high confidence
        for _ in 0..10 {
            observe_pattern(&cfg, &PatternObservation {
                pattern_type: PatternType::Preference,
                trigger_context: "coding".into(),
                action_taken: "strong preference".into(),
                metadata: None,
            }).unwrap();
        }

        let pruned = prune_weak_patterns(&cfg, 0.5).unwrap();
        assert_eq!(pruned, 1);

        let remaining = pattern_count(&cfg).unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn confidence_grows_with_frequency() {
        assert!(compute_confidence(1) < compute_confidence(3));
        assert!(compute_confidence(3) < compute_confidence(6));
        assert!(compute_confidence(10) > 0.95);
    }
}
