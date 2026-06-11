//! Community pattern service — aggregated group patterns and cold-start.
//!
//! Responsibilities:
//! - Receive encrypted pattern shares from clients.
//! - Decrypt aggregate when threshold is met.
//! - Generate group-level patterns for recommendation.
//! - Cold-start: provide initial pattern framework for new users.
//! - Segmentation: separate patterns by user group.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use super::aggregation::{AggregateResult, AggregationConfig, PatternVector, aggregate_vectors};

// ─── Types ───────────────────────────────────────────────────────────────────

/// A user group for segmented pattern aggregation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserGroup {
    Developer,
    Designer,
    Manager,
    Researcher,
    General,
}

/// A community pattern — aggregated across multiple users.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunityPattern {
    /// Pattern label (generalized, no PII).
    pub label: String,
    /// Average frequency across participants.
    pub avg_frequency: f64,
    /// How many users exhibit this pattern.
    pub user_count: u32,
    /// Confidence (fraction of group that shows this pattern).
    pub group_confidence: f64,
    /// Which group this pattern belongs to.
    pub group: UserGroup,
}

/// Cold-start recommendation for a new user.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColdStartProfile {
    /// Suggested initial patterns based on user group.
    pub suggested_patterns: Vec<CommunityPattern>,
    /// The group used to generate suggestions.
    pub group: UserGroup,
    /// How many users contributed to these patterns.
    pub source_user_count: u32,
}

/// Collection state for a pending aggregation round.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggregationRound {
    /// Round identifier.
    pub round_id: String,
    /// Target group.
    pub group: UserGroup,
    /// Pattern dimension labels (what each vector index represents).
    pub dimension_labels: Vec<String>,
    /// Collected vectors so far (in production, these would be encrypted shares).
    pub collected_vectors: Vec<PatternVector>,
    /// Configuration.
    pub config: AggregationConfig,
}

impl AggregationRound {
    /// Create a new aggregation round.
    pub fn new(round_id: impl Into<String>, group: UserGroup, dimension_labels: Vec<String>) -> Self {
        Self {
            round_id: round_id.into(),
            group,
            dimension_labels,
            collected_vectors: Vec::new(),
            config: AggregationConfig::default(),
        }
    }

    /// Submit a user's pattern vector to this round.
    pub fn submit(&mut self, vector: PatternVector) -> Result<()> {
        if vector.len() != self.dimension_labels.len() {
            bail!(
                "dimension mismatch: expected {}, got {}",
                self.dimension_labels.len(),
                vector.len()
            );
        }
        self.collected_vectors.push(vector);
        Ok(())
    }

    /// Check if enough participants have submitted.
    pub fn is_ready(&self) -> bool {
        self.collected_vectors.len() as u32 >= self.config.min_participants
    }

    /// Finalize the round and produce community patterns.
    pub fn finalize(&self) -> Result<Vec<CommunityPattern>> {
        if !self.is_ready() {
            bail!(
                "not enough participants: {} < {}",
                self.collected_vectors.len(),
                self.config.min_participants
            );
        }

        let result = aggregate_vectors(&self.collected_vectors, &self.config)?;
        let user_count = result.participant_count;

        let patterns: Vec<CommunityPattern> = self
            .dimension_labels
            .iter()
            .enumerate()
            .filter_map(|(i, label)| {
                let avg = result.aggregate[i] / user_count as f64;
                // Only include patterns with meaningful frequency
                if avg >= 1.0 {
                    Some(CommunityPattern {
                        label: label.clone(),
                        avg_frequency: avg,
                        user_count,
                        group_confidence: (avg / 10.0).min(1.0), // Normalize
                        group: self.group.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(patterns)
    }
}

// ─── Cold Start ──────────────────────────────────────────────────────────────

/// Generate a cold-start profile for a new user based on community patterns.
pub fn generate_cold_start(
    community_patterns: &[CommunityPattern],
    group: &UserGroup,
    max_patterns: usize,
) -> ColdStartProfile {
    let mut relevant: Vec<&CommunityPattern> = community_patterns
        .iter()
        .filter(|p| &p.group == group)
        .collect();

    relevant.sort_by(|a, b| b.group_confidence.partial_cmp(&a.group_confidence).unwrap_or(std::cmp::Ordering::Equal));
    relevant.truncate(max_patterns);

    let source_count = relevant.first().map(|p| p.user_count).unwrap_or(0);

    ColdStartProfile {
        suggested_patterns: relevant.into_iter().cloned().collect(),
        group: group.clone(),
        source_user_count: source_count,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregation_round_workflow() {
        let labels = vec![
            "writes_summary_after_meeting".into(),
            "reviews_prs_in_morning".into(),
            "uses_iterators".into(),
        ];

        let mut round = AggregationRound::new("round-1", UserGroup::Developer, labels);
        round.config.min_participants = 3;

        // Submit from 3 users
        round.submit(vec![5.0, 3.0, 8.0]).unwrap();
        round.submit(vec![4.0, 2.0, 7.0]).unwrap();
        round.submit(vec![6.0, 4.0, 9.0]).unwrap();

        assert!(round.is_ready());

        let patterns = round.finalize().unwrap();
        // avg: [5.0, 3.0, 8.0] — all >= 1.0, so all included
        assert_eq!(patterns.len(), 3);
        assert!((patterns[0].avg_frequency - 5.0).abs() < 0.01);
    }

    #[test]
    fn round_rejects_wrong_dimension() {
        let mut round = AggregationRound::new("r", UserGroup::General, vec!["a".into(), "b".into()]);
        let result = round.submit(vec![1.0, 2.0, 3.0]);
        assert!(result.is_err());
    }

    #[test]
    fn cold_start_filters_by_group() {
        let patterns = vec![
            CommunityPattern {
                label: "writes code".into(),
                avg_frequency: 8.0,
                user_count: 100,
                group_confidence: 0.8,
                group: UserGroup::Developer,
            },
            CommunityPattern {
                label: "designs mockups".into(),
                avg_frequency: 7.0,
                user_count: 50,
                group_confidence: 0.7,
                group: UserGroup::Designer,
            },
            CommunityPattern {
                label: "reviews PRs".into(),
                avg_frequency: 5.0,
                user_count: 100,
                group_confidence: 0.5,
                group: UserGroup::Developer,
            },
        ];

        let profile = generate_cold_start(&patterns, &UserGroup::Developer, 10);
        assert_eq!(profile.suggested_patterns.len(), 2);
        assert_eq!(profile.group, UserGroup::Developer);
        // Should be sorted by confidence desc
        assert!(profile.suggested_patterns[0].group_confidence >= profile.suggested_patterns[1].group_confidence);
    }

    #[test]
    fn finalize_rejects_insufficient_participants() {
        let round = AggregationRound::new("r", UserGroup::General, vec!["x".into()]);
        let result = round.finalize();
        assert!(result.is_err());
    }
}
