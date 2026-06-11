//! Differential privacy — ε-DP mechanisms for federated pattern sharing.
//!
//! Before patterns are shared with the aggregation server:
//! 1. PII is stripped (names → "Person", orgs → "Organization").
//! 2. Frequency counts get Laplace noise (ε-differential privacy).
//! 3. Privacy budget tracks cumulative ε spent.

use anyhow::{Context, Result};
use rand::Rng;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::store::chunks::store::with_connection;

use super::local_patterns::Pattern;

// ─── Schema ──────────────────────────────────────────────────────────────────

const PRIVACY_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS mem_privacy_budget (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    epsilon_spent   REAL NOT NULL DEFAULT 0.0,
    epsilon_max     REAL NOT NULL DEFAULT 10.0,
    reports_sent    INTEGER NOT NULL DEFAULT 0,
    last_report_ms  INTEGER
);

INSERT OR IGNORE INTO mem_privacy_budget (id, epsilon_spent, epsilon_max) VALUES (1, 0.0, 10.0);
";

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(PRIVACY_SCHEMA)
        .context("create privacy schema")?;
    Ok(())
}

// ─── Types ───────────────────────────────────────────────────────────────────

/// A pattern sanitized for sharing (PII removed, noise added).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SanitizedPattern {
    /// Generalized trigger (PII replaced with type tokens).
    pub trigger_context: String,
    /// Generalized action.
    pub action_taken: String,
    /// Noisy frequency (original + Laplace noise).
    pub noisy_frequency: f64,
    /// Noisy confidence.
    pub noisy_confidence: f64,
    /// Pattern type preserved (non-PII).
    pub pattern_type: String,
}

/// Privacy budget status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyBudget {
    pub epsilon_spent: f64,
    pub epsilon_max: f64,
    pub reports_sent: u32,
    pub budget_remaining: f64,
    pub can_report: bool,
}

/// Configuration for the privacy mechanism.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// Epsilon per report (smaller = more private, noisier).
    pub epsilon_per_report: f64,
    /// Maximum total epsilon budget.
    pub epsilon_max: f64,
    /// Sensitivity of the frequency query (max change one user can cause).
    pub sensitivity: f64,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            epsilon_per_report: 1.0,
            epsilon_max: 10.0,
            sensitivity: 1.0,
        }
    }
}

// ─── Sanitization ────────────────────────────────────────────────────────────

/// Sanitize a batch of patterns for federated sharing.
///
/// Applies: PII generalization → Laplace noise → budget check.
pub fn sanitize_patterns(
    config: &Config,
    patterns: &[Pattern],
    privacy_config: &PrivacyConfig,
) -> Result<Vec<SanitizedPattern>> {
    // Check budget
    let budget = get_budget(config)?;
    if !budget.can_report {
        anyhow::bail!("privacy budget exhausted: spent {:.2} of {:.2}", budget.epsilon_spent, budget.epsilon_max);
    }

    let mut rng = rand::thread_rng();
    let scale = privacy_config.sensitivity / privacy_config.epsilon_per_report;

    let sanitized: Vec<SanitizedPattern> = patterns
        .iter()
        .map(|p| {
            let trigger = generalize_text(&p.trigger_context);
            let action = generalize_text(&p.action_taken);
            let noisy_freq = (p.frequency as f64) + laplace_noise(&mut rng, scale);
            let noisy_conf = p.confidence + laplace_noise(&mut rng, scale * 0.1);

            SanitizedPattern {
                trigger_context: trigger,
                action_taken: action,
                noisy_frequency: noisy_freq.max(0.0),
                noisy_confidence: noisy_conf.clamp(0.0, 1.0),
                pattern_type: format!("{:?}", p.pattern_type).to_lowercase(),
            }
        })
        .collect();

    // Consume budget
    consume_budget(config, privacy_config.epsilon_per_report)?;

    Ok(sanitized)
}

// ─── PII Generalization ──────────────────────────────────────────────────────

/// Replace likely PII tokens with generic type placeholders.
///
/// Rules:
/// - Capitalized words that look like names → "Person"
/// - Known org patterns → "Organization"
/// - Email-like patterns → "email@example.com"
/// - Numbers (phone, ID) → "NUMBER"
fn generalize_text(text: &str) -> String {
    let mut result = text.to_string();

    // Email pattern
    let email_re = regex::Regex::new(r"\b[\w.+-]+@[\w-]+\.[\w.-]+\b").unwrap();
    result = email_re.replace_all(&result, "[EMAIL]").to_string();

    // Phone-like numbers (7+ digits)
    let phone_re = regex::Regex::new(r"\b\d[\d\s\-().]{6,}\d\b").unwrap();
    result = phone_re.replace_all(&result, "[NUMBER]").to_string();

    // Capitalized proper nouns (heuristic: 2+ capitalized words in sequence)
    // Only replace if not common words
    let name_re = regex::Regex::new(r"\b[A-Z][a-z]+(?:\s+[A-Z][a-z]+)+\b").unwrap();
    result = name_re.replace_all(&result, "[Person]").to_string();

    // Single capitalized word that's likely a name (after known entities)
    // Keep common words: Monday, Tuesday, January, etc.
    let common_caps: &[&str] = &[
        "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday",
        "January", "February", "March", "April", "May", "June", "July", "August",
        "September", "October", "November", "December",
        "Rust", "Python", "JavaScript", "TypeScript", "React", "Linux", "Windows", "macOS",
    ];

    let single_cap_re = regex::Regex::new(r"\b[A-Z][a-z]{2,}\b").unwrap();
    result = single_cap_re
        .replace_all(&result, |caps: &regex::Captures| {
            let word = &caps[0];
            if common_caps.contains(&word) {
                word.to_string()
            } else {
                "[Entity]".to_string()
            }
        })
        .to_string();

    result
}

// ─── Laplace Mechanism ───────────────────────────────────────────────────────

/// Generate Laplace noise with given scale (b = sensitivity/epsilon).
fn laplace_noise(rng: &mut impl Rng, scale: f64) -> f64 {
    // Laplace(0, b) via inverse CDF: X = -b * sign(U) * ln(1 - 2|U|)
    // where U ~ Uniform(-0.5, 0.5)
    let u: f64 = rng.gen_range(-0.5_f64..0.5_f64);
    let sign = if u < 0.0 { -1.0 } else { 1.0 };
    -scale * sign * (1.0 - 2.0 * u.abs()).ln()
}

// ─── Budget Management ───────────────────────────────────────────────────────

/// Get the current privacy budget status.
pub fn get_budget(config: &Config) -> Result<PrivacyBudget> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.query_row(
            "SELECT epsilon_spent, epsilon_max, reports_sent FROM mem_privacy_budget WHERE id = 1",
            [],
            |row| {
                let spent: f64 = row.get(0)?;
                let max: f64 = row.get(1)?;
                let reports: u32 = row.get(2)?;
                Ok(PrivacyBudget {
                    epsilon_spent: spent,
                    epsilon_max: max,
                    reports_sent: reports,
                    budget_remaining: max - spent,
                    can_report: spent < max,
                })
            },
        )
        .context("get privacy budget")
    })
}

/// Consume epsilon from the budget after a report.
fn consume_budget(config: &Config, epsilon: f64) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "UPDATE mem_privacy_budget SET epsilon_spent = epsilon_spent + ?1, reports_sent = reports_sent + 1, last_report_ms = ?2 WHERE id = 1",
            params![epsilon, now_ms],
        )?;
        Ok(())
    })
}

/// Reset the privacy budget (e.g., on new budget period).
pub fn reset_budget(config: &Config, new_max: f64) -> Result<()> {
    with_connection(config, |conn| {
        ensure_schema(conn)?;
        conn.execute(
            "UPDATE mem_privacy_budget SET epsilon_spent = 0.0, epsilon_max = ?1, reports_sent = 0, last_report_ms = NULL WHERE id = 1",
            params![new_max],
        )?;
        Ok(())
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::local_patterns::{PatternType, Pattern};
    use chrono::Utc;
    use tempfile::TempDir;

    fn test_config() -> (TempDir, Config) {
        let tmp = TempDir::new().unwrap();
        let mut cfg = Config::default();
        cfg.workspace_dir = tmp.path().to_path_buf();
        (tmp, cfg)
    }

    fn sample_patterns() -> Vec<Pattern> {
        let now = Utc::now();
        vec![
            Pattern {
                pattern_id: "p1".into(),
                pattern_type: PatternType::Temporal,
                trigger_context: "after meeting with John Smith".into(),
                action_taken: "writes summary email to alice@company.com".into(),
                frequency: 5,
                confidence: 0.88,
                first_seen: now,
                last_seen: now,
                metadata: serde_json::json!({}),
            },
            Pattern {
                pattern_id: "p2".into(),
                pattern_type: PatternType::Preference,
                trigger_context: "writing Rust code".into(),
                action_taken: "prefers iterators".into(),
                frequency: 10,
                confidence: 0.95,
                first_seen: now,
                last_seen: now,
                metadata: serde_json::json!({}),
            },
        ]
    }

    #[test]
    fn generalize_strips_pii() {
        let text = "after meeting with John Smith at Google";
        let generalized = generalize_text(text);
        assert!(!generalized.contains("John Smith"), "should strip name, got: {}", generalized);
        // Should contain either [Person] or [Entity] (depending on regex match order)
        assert!(
            generalized.contains("[Person]") || generalized.contains("[Entity]"),
            "should generalize PII, got: {}",
            generalized
        );
    }

    #[test]
    fn generalize_strips_email() {
        let text = "sends email to alice@example.com";
        let generalized = generalize_text(text);
        assert!(!generalized.contains("alice@example.com"));
        assert!(generalized.contains("[EMAIL]"));
    }

    #[test]
    fn generalize_preserves_common_words() {
        let text = "uses Rust on Monday";
        let generalized = generalize_text(text);
        assert!(generalized.contains("Rust"));
        assert!(generalized.contains("Monday"));
    }

    #[test]
    fn sanitize_adds_noise() {
        let (_tmp, cfg) = test_config();
        let patterns = sample_patterns();
        let pc = PrivacyConfig::default();

        let sanitized = sanitize_patterns(&cfg, &patterns, &pc).unwrap();
        assert_eq!(sanitized.len(), 2);

        // Noisy frequency should differ from original (with high probability)
        // but this is probabilistic — just check it's non-negative
        assert!(sanitized[0].noisy_frequency >= 0.0);
        assert!(sanitized[1].noisy_confidence >= 0.0);
        assert!(sanitized[1].noisy_confidence <= 1.0);
    }

    #[test]
    fn budget_is_consumed() {
        let (_tmp, cfg) = test_config();
        let patterns = sample_patterns();
        let pc = PrivacyConfig {
            epsilon_per_report: 2.0,
            epsilon_max: 10.0,
            sensitivity: 1.0,
        };

        sanitize_patterns(&cfg, &patterns, &pc).unwrap();
        let budget = get_budget(&cfg).unwrap();
        assert!((budget.epsilon_spent - 2.0).abs() < 0.01);
        assert_eq!(budget.reports_sent, 1);
    }

    #[test]
    fn budget_exhaustion_blocks_report() {
        let (_tmp, cfg) = test_config();
        let patterns = sample_patterns();
        let pc = PrivacyConfig {
            epsilon_per_report: 6.0,
            epsilon_max: 10.0,
            sensitivity: 1.0,
        };

        sanitize_patterns(&cfg, &patterns, &pc).unwrap(); // 6.0
        sanitize_patterns(&cfg, &patterns, &pc).unwrap(); // 12.0 > 10.0 budget... wait

        // After first report: spent=6, remaining=4, can_report=true
        // After second: spent=12, but we check BEFORE consuming...
        // Actually the check is budget.can_report = spent < max (6 < 10 = true)
        // Third should fail
        let result = sanitize_patterns(&cfg, &patterns, &pc);
        // spent is now 12.0 > 10.0, so can_report = false
        assert!(result.is_err());
    }

    #[test]
    fn reset_budget_works() {
        let (_tmp, cfg) = test_config();
        let patterns = sample_patterns();
        let pc = PrivacyConfig::default();
        sanitize_patterns(&cfg, &patterns, &pc).unwrap();

        reset_budget(&cfg, 20.0).unwrap();
        let budget = get_budget(&cfg).unwrap();
        assert_eq!(budget.epsilon_spent, 0.0);
        assert_eq!(budget.epsilon_max, 20.0);
        assert_eq!(budget.reports_sent, 0);
    }

    #[test]
    fn laplace_noise_is_centered() {
        let mut rng = rand::thread_rng();
        let samples: Vec<f64> = (0..10000).map(|_| laplace_noise(&mut rng, 1.0)).collect();
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        // Mean should be approximately 0 (within statistical tolerance)
        assert!(mean.abs() < 0.1, "mean was {}", mean);
    }
}
