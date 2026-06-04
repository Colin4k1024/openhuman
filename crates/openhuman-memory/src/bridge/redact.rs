//! Source-tag redaction for debug/log identifiers.

/// Redact a string for debug output (truncate + mask).
pub fn redact(s: &str) -> String {
    if s.len() <= 8 {
        return s.to_string();
    }
    format!("{}…{}", &s[..4], &s[s.len() - 4..])
}
