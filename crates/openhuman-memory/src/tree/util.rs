//! Utility functions for the tree module.

/// Floor char boundary — find the largest index ≤ `max` that is a valid char boundary.
pub fn floor_char_boundary(s: &str, max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    let mut i = max;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_boundary() {
        assert_eq!(floor_char_boundary("hello", 3), 3);
    }

    #[test]
    fn unicode_boundary() {
        let s = "héllo";
        let b = floor_char_boundary(s, 2);
        assert!(s.is_char_boundary(b));
    }
}
