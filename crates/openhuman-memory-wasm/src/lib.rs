//! WASM bindings for openhuman-memory.
//!
//! This crate exposes lightweight, pure-compute memory functions to the browser:
//! - Text scoring (signal computation, admission gate)
//! - Entity extraction (regex-based)
//! - Cosine similarity between vectors
//!
//! Does NOT include SQLite storage or network calls — those require native runtimes.
//! For full storage, use the HTTP server (`openhuman-memory-server`) from the browser.

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};

/// Approximate token count (4 chars per token heuristic).
#[wasm_bindgen]
pub fn approx_token_count(text: &str) -> u32 {
    (text.len() / 4).max(1) as u32
}

/// Cosine similarity between two f32 vectors.
#[wasm_bindgen]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-10 {
        0.0
    } else {
        dot / denom
    }
}

/// Extract entities from text using regex patterns.
/// Returns JSON array of {text, kind} objects.
#[wasm_bindgen]
pub fn extract_entities(text: &str) -> String {
    let entities = extract_entities_inner(text);
    serde_json::to_string(&entities).unwrap_or_else(|_| "[]".to_string())
}

#[derive(Serialize, Deserialize)]
struct Entity {
    text: String,
    kind: String,
}

fn extract_entities_inner(text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();

    // Date patterns (YYYY-MM-DD, MM/DD/YYYY)
    let date_re = regex::Regex::new(
        r"\b(\d{4}-\d{2}-\d{2}|\d{1,2}/\d{1,2}/\d{4})\b"
    ).unwrap();
    for m in date_re.find_iter(text) {
        entities.push(Entity {
            text: m.as_str().to_string(),
            kind: "Date".to_string(),
        });
    }

    // Email pattern
    let email_re = regex::Regex::new(
        r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b"
    ).unwrap();
    for m in email_re.find_iter(text) {
        entities.push(Entity {
            text: m.as_str().to_string(),
            kind: "Email".to_string(),
        });
    }

    // URL pattern
    let url_re = regex::Regex::new(
        r#"https?://[^\s<>"']+"#
    ).unwrap();
    for m in url_re.find_iter(text) {
        entities.push(Entity {
            text: m.as_str().to_string(),
            kind: "Url".to_string(),
        });
    }

    entities
}

/// Score text content for memory admission. Returns JSON with score details.
/// Uses a simplified version of the scoring pipeline (unique words ratio + token density).
#[wasm_bindgen]
pub fn score_text(text: &str) -> String {
    let token_count = approx_token_count(text);
    let words: Vec<&str> = text.split_whitespace().collect();
    let unique_words: std::collections::HashSet<&str> = words.iter().copied().collect();

    let unique_ratio = if words.is_empty() {
        0.0
    } else {
        unique_words.len() as f32 / words.len() as f32
    };

    // Token density signal (normalized 0-1, peaks around 50-200 tokens)
    let token_signal = if token_count < 5 {
        0.1
    } else if token_count < 50 {
        0.3 + 0.4 * (token_count as f32 / 50.0)
    } else if token_count <= 200 {
        0.7 + 0.3 * ((token_count - 50) as f32 / 150.0)
    } else {
        1.0
    };

    let entities = extract_entities_inner(text);
    let entity_bonus = (entities.len() as f32 * 0.05).min(0.3);

    let total = (unique_ratio * 0.4 + token_signal * 0.4 + entity_bonus * 0.2).min(1.0);
    let kept = total > 0.25;

    let result = serde_json::json!({
        "total": total,
        "kept": kept,
        "token_count": token_count,
        "unique_words_ratio": unique_ratio,
        "token_signal": token_signal,
        "entity_count": entities.len(),
        "entities": entities,
    });

    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);
    }

    #[test]
    fn test_score_text() {
        let result = score_text("This is a meaningful sentence about Rust programming on 2024-03-15.");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["kept"].as_bool().unwrap());
        assert!(parsed["total"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn test_extract_entities() {
        let result = extract_entities("Meeting on 2024-03-15 with user@example.com");
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result).unwrap();
        assert!(parsed.len() >= 2);
    }

    #[test]
    fn test_trivial_text_low_score() {
        let result = score_text("ok");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        // Very short text gets low token signal even if unique ratio is high
        assert!(parsed["total"].as_f64().unwrap() < 0.6);
    }
}
