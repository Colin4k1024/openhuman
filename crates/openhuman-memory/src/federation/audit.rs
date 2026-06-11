//! Privacy audit — user visibility, opt-out, compliance, and GDPR erasure.
//!
//! Provides:
//! - View what patterns the user has contributed (sanitized form).
//! - One-click opt-out: remove user's contribution from the aggregation pool.
//! - Compliance report: track epsilon spent per period.
//! - GDPR Right to Erasure: delete all federated data for a user.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;

// ─── Schema ──────────────────────────────────────────────────────────────────

const AUDIT_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mem_federation_audit (
    report_id       TEXT PRIMARY KEY,
    report_type     TEXT NOT NULL,
    epsilon_spent   REAL NOT NULL,
    patterns_shared INTEGER NOT NULL,
    created_at_ms   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mem_federation_contributions (
    contribution_id TEXT PRIMARY KEY,
    round_id        TEXT NOT NULL,
    pattern_summary TEXT NOT NULL,
    sanitized       INTEGER NOT NULL DEFAULT 1,
    created_at_ms   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mem_federation_opt_out (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    opted_out       INTEGER NOT NULL DEFAULT 0,
    opted_out_at_ms INTEGER,
    reason          TEXT
);

INSERT OR IGNORE INTO mem_federation_opt_out (id, opted_out) VALUES (1, 0);
";

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(AUDIT_SCHEMA)
        .context("create audit schema")?;
    Ok(())
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// A record of a federation contribution (what was shared).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContributionRecord {
    pub contribution_id: String,
    pub round_id: String,
    pub pattern_summary: String,
    pub sanitized: bool,
    pub created_at: DateTime<Utc>,
}

/// A compliance report entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditReport {
    pub report_id: String,
    pub report_type: String,
    pub epsilon_spent: f64,
    pub patterns_shared: u32,
    pub created_at: DateTime<Utc>,
}

/// Opt-out status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptOutStatus {
    pub opted_out: bool,
    pub opted_out_at: Option<DateTime<Utc>>,
    pub reason: Option<String>,
}

/// Summary of all federation activity for compliance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComplianceSummary {
    pub total_reports: u32,
    pub total_epsilon_spent: f64,
    pub total_patterns_shared: u32,
    pub opt_out_status: OptOutStatus,
    pub contributions: Vec<ContributionRecord>,
}

// ─── Contribution Tracking ───────────────────────────────────────────────────

/// Record that patterns were shared in a federation round.
pub fn record_contribution(
    config: &Config,
    round_id: &str,
    pattern_summary: &str,
) -> Result<String> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let id = format!("contrib-{}", uuid::Uuid::new_v4().simple());
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO mem_federation_contributions (contribution_id, round_id, pattern_summary, sanitized, created_at_ms)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![id, round_id, pattern_summary, now_ms],
        )?;
        Ok(id)
    })
}

/// List all contributions this user has made.
pub fn list_contributions(config: &Config, limit: usize) -> Result<Vec<ContributionRecord>> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let mut stmt = conn.prepare(
            "SELECT contribution_id, round_id, pattern_summary, sanitized, created_at_ms
             FROM mem_federation_contributions ORDER BY created_at_ms DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(ContributionRecord {
                    contribution_id: row.get(0)?,
                    round_id: row.get(1)?,
                    pattern_summary: row.get(2)?,
                    sanitized: row.get::<_, i64>(3)? != 0,
                    created_at: DateTime::from_timestamp_millis(row.get(4)?).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("list contributions")?;
        Ok(rows)
    })
}

// ─── Audit Reports ───────────────────────────────────────────────────────────

/// Record an audit report (tracks what was spent in a reporting period).
pub fn record_audit_report(
    config: &Config,
    report_type: &str,
    epsilon_spent: f64,
    patterns_shared: u32,
) -> Result<String> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let id = format!("audit-{}", uuid::Uuid::new_v4().simple());
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO mem_federation_audit (report_id, report_type, epsilon_spent, patterns_shared, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, report_type, epsilon_spent, patterns_shared, now_ms],
        )?;
        Ok(id)
    })
}

// ─── Opt-Out ─────────────────────────────────────────────────────────────────

/// Opt out of federation (one-click).
pub fn opt_out(config: &Config, reason: Option<&str>) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE mem_federation_opt_out SET opted_out = 1, opted_out_at_ms = ?1, reason = ?2 WHERE id = 1",
            params![now_ms, reason],
        )?;
        Ok(())
    })
}

/// Opt back in.
pub fn opt_in(config: &Config) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "UPDATE mem_federation_opt_out SET opted_out = 0, opted_out_at_ms = NULL, reason = NULL WHERE id = 1",
            [],
        )?;
        Ok(())
    })
}

/// Get current opt-out status.
pub fn get_opt_out_status(config: &Config) -> Result<OptOutStatus> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.query_row(
            "SELECT opted_out, opted_out_at_ms, reason FROM mem_federation_opt_out WHERE id = 1",
            [],
            |row| {
                let opted: i64 = row.get(0)?;
                let at_ms: Option<i64> = row.get(1)?;
                let reason: Option<String> = row.get(2)?;
                Ok(OptOutStatus {
                    opted_out: opted != 0,
                    opted_out_at: at_ms.and_then(DateTime::from_timestamp_millis),
                    reason,
                })
            },
        )
        .context("get opt-out status")
    })
}

/// Check if user is opted out (convenience).
pub fn is_opted_out(config: &Config) -> Result<bool> {
    Ok(get_opt_out_status(config)?.opted_out)
}

// ─── GDPR Erasure ────────────────────────────────────────────────────────────

/// Execute GDPR Right to Erasure — delete ALL federation data.
///
/// This removes:
/// - All contribution records
/// - All audit reports
/// - Privacy budget history
/// - Sets opt-out permanently
///
/// Returns the number of records deleted.
pub fn gdpr_erase(config: &Config) -> Result<GdprErasureResult> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;

        let contributions_deleted = conn.execute("DELETE FROM mem_federation_contributions", [])?;
        let reports_deleted = conn.execute("DELETE FROM mem_federation_audit", [])?;

        // Reset privacy budget
        conn.execute(
            "UPDATE mem_privacy_budget SET epsilon_spent = 0, reports_sent = 0, last_report_ms = NULL WHERE id = 1",
            [],
        ).ok(); // May not exist if privacy module wasn't used

        // Set permanent opt-out
        let now_ms = Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE mem_federation_opt_out SET opted_out = 1, opted_out_at_ms = ?1, reason = 'GDPR erasure' WHERE id = 1",
            params![now_ms],
        )?;

        Ok(GdprErasureResult {
            contributions_deleted,
            reports_deleted,
            opted_out: true,
        })
    })
}

/// Result of a GDPR erasure operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GdprErasureResult {
    pub contributions_deleted: usize,
    pub reports_deleted: usize,
    pub opted_out: bool,
}

// ─── Compliance Summary ──────────────────────────────────────────────────────

/// Generate a full compliance summary for the user.
pub fn compliance_summary(config: &Config) -> Result<ComplianceSummary> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;

        let (total_reports, total_epsilon, total_patterns): (u32, f64, u32) = conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(epsilon_spent), 0), COALESCE(SUM(patterns_shared), 0) FROM mem_federation_audit",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap_or((0, 0.0, 0));

        let opt_out_status = conn.query_row(
            "SELECT opted_out, opted_out_at_ms, reason FROM mem_federation_opt_out WHERE id = 1",
            [],
            |row| {
                let opted: i64 = row.get(0)?;
                let at_ms: Option<i64> = row.get(1)?;
                let reason: Option<String> = row.get(2)?;
                Ok(OptOutStatus {
                    opted_out: opted != 0,
                    opted_out_at: at_ms.and_then(DateTime::from_timestamp_millis),
                    reason,
                })
            },
        )?;

        let mut stmt = conn.prepare(
            "SELECT contribution_id, round_id, pattern_summary, sanitized, created_at_ms
             FROM mem_federation_contributions ORDER BY created_at_ms DESC LIMIT 50",
        )?;
        let contributions = stmt
            .query_map([], |row| {
                Ok(ContributionRecord {
                    contribution_id: row.get(0)?,
                    round_id: row.get(1)?,
                    pattern_summary: row.get(2)?,
                    sanitized: row.get::<_, i64>(3)? != 0,
                    created_at: DateTime::from_timestamp_millis(row.get(4)?).unwrap_or_default(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(ComplianceSummary {
            total_reports,
            total_epsilon_spent: total_epsilon,
            total_patterns_shared: total_patterns,
            opt_out_status,
            contributions,
        })
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
    fn contribution_tracking() {
        let (_tmp, cfg) = test_config();
        record_contribution(&cfg, "round-1", "temporal: after meeting → writes summary").unwrap();
        record_contribution(&cfg, "round-1", "preference: uses Rust iterators").unwrap();

        let contribs = list_contributions(&cfg, 10).unwrap();
        assert_eq!(contribs.len(), 2);
        assert!(contribs[0].sanitized);
    }

    #[test]
    fn opt_out_and_in() {
        let (_tmp, cfg) = test_config();

        assert!(!is_opted_out(&cfg).unwrap());

        opt_out(&cfg, Some("privacy concerns")).unwrap();
        assert!(is_opted_out(&cfg).unwrap());

        let status = get_opt_out_status(&cfg).unwrap();
        assert_eq!(status.reason.as_deref(), Some("privacy concerns"));

        opt_in(&cfg).unwrap();
        assert!(!is_opted_out(&cfg).unwrap());
    }

    #[test]
    fn gdpr_erasure_deletes_everything() {
        let (_tmp, cfg) = test_config();
        record_contribution(&cfg, "r1", "pattern A").unwrap();
        record_contribution(&cfg, "r2", "pattern B").unwrap();
        record_audit_report(&cfg, "monthly", 2.0, 5).unwrap();

        let result = gdpr_erase(&cfg).unwrap();
        assert_eq!(result.contributions_deleted, 2);
        assert_eq!(result.reports_deleted, 1);
        assert!(result.opted_out);

        // Verify everything is gone
        let contribs = list_contributions(&cfg, 10).unwrap();
        assert!(contribs.is_empty());
        assert!(is_opted_out(&cfg).unwrap());
    }

    #[test]
    fn compliance_summary_aggregates() {
        let (_tmp, cfg) = test_config();
        record_audit_report(&cfg, "weekly", 1.0, 3).unwrap();
        record_audit_report(&cfg, "weekly", 1.5, 4).unwrap();
        record_contribution(&cfg, "r1", "test").unwrap();

        let summary = compliance_summary(&cfg).unwrap();
        assert_eq!(summary.total_reports, 2);
        assert!((summary.total_epsilon_spent - 2.5).abs() < 0.01);
        assert_eq!(summary.total_patterns_shared, 7);
        assert_eq!(summary.contributions.len(), 1);
        assert!(!summary.opt_out_status.opted_out);
    }
}
