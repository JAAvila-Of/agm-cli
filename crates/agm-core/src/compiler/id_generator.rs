//! Node ID generation from Markdown heading text.
//!
//! Produces valid AGM node IDs matching `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*`.

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// IdGenerator
// ---------------------------------------------------------------------------

/// Generates unique, valid AGM node IDs from heading text.
///
/// Tracks previously generated IDs to detect and resolve collisions
/// by appending numeric suffixes.
pub(crate) struct IdGenerator {
    prefix: Option<String>,
    used_ids: HashSet<String>,
}

impl IdGenerator {
    /// Creates a new generator with an optional ID prefix.
    pub fn new(prefix: Option<&str>) -> Self {
        Self {
            prefix: prefix.map(sanitize_segment),
            used_ids: HashSet::new(),
        }
    }

    /// Generates a valid node ID from heading text.
    ///
    /// Returns `(id, collision)` where `collision` is `true` if a numeric
    /// suffix was appended to avoid duplication.
    pub fn generate(&mut self, heading: &str) -> (String, bool) {
        let base = self.heading_to_id(heading);

        if base.is_empty() {
            // Completely unsanitizable heading; use a numeric fallback
            let fallback = self.make_unique("node".to_owned());
            return (fallback.0, fallback.1);
        }

        self.make_unique(base)
    }

    /// Converts heading text to a base ID (without collision resolution).
    fn heading_to_id(&self, heading: &str) -> String {
        let segment = sanitize_segment(heading);

        match &self.prefix {
            Some(prefix) if !prefix.is_empty() => format!("{prefix}.{segment}"),
            _ => segment,
        }
    }

    /// Ensures the ID is unique; appends `.1`, `.2`, etc. on collision.
    fn make_unique(&mut self, base: String) -> (String, bool) {
        if self.used_ids.insert(base.clone()) {
            return (base, false);
        }

        // Collision: append suffix
        let mut counter = 1u32;
        loop {
            let candidate = format!("{base}.{counter}");
            if self.used_ids.insert(candidate.clone()) {
                return (candidate, true);
            }
            counter += 1;
        }
    }
}

/// Sanitizes a string into a valid AGM ID segment.
///
/// - Converts to lowercase
/// - Replaces spaces and hyphens with underscores
/// - Removes characters that are not `[a-z0-9_]`
/// - Ensures the segment starts with `[a-z]` (prepends 'n' if needed)
/// - Collapses consecutive underscores
fn sanitize_segment(text: &str) -> String {
    let mut result = String::with_capacity(text.len());

    for ch in text.chars() {
        match ch {
            'A'..='Z' => result.push(ch.to_ascii_lowercase()),
            'a'..='z' | '0'..='9' => result.push(ch),
            ' ' | '-' | '_' => result.push('_'),
            _ => {} // drop non-ASCII and special chars
        }
    }

    // Collapse consecutive underscores
    let mut collapsed = String::with_capacity(result.len());
    let mut prev_underscore = false;
    for ch in result.chars() {
        if ch == '_' {
            if !prev_underscore {
                collapsed.push('_');
            }
            prev_underscore = true;
        } else {
            collapsed.push(ch);
            prev_underscore = false;
        }
    }

    // Trim leading/trailing underscores
    let trimmed = collapsed.trim_matches('_').to_owned();

    // Ensure starts with [a-z]
    if trimmed.is_empty() {
        return String::new();
    }

    if trimmed.as_bytes()[0].is_ascii_digit() || trimmed.as_bytes()[0] == b'_' {
        format!("n_{trimmed}")
    } else {
        trimmed
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_segment_simple() {
        assert_eq!(sanitize_segment("Login Flow"), "login_flow");
    }

    #[test]
    fn test_sanitize_segment_hyphens() {
        assert_eq!(sanitize_segment("error-recovery"), "error_recovery");
    }

    #[test]
    fn test_sanitize_segment_special_chars_removed() {
        assert_eq!(sanitize_segment("What's this?!"), "whats_this");
    }

    #[test]
    fn test_sanitize_segment_leading_digit_prefixed() {
        assert_eq!(sanitize_segment("3rd Party Auth"), "n_3rd_party_auth");
    }

    #[test]
    fn test_sanitize_segment_consecutive_spaces_collapsed() {
        assert_eq!(sanitize_segment("Login   Flow"), "login_flow");
    }

    #[test]
    fn test_sanitize_segment_unicode_dropped() {
        assert_eq!(sanitize_segment("Flujo de Login"), "flujo_de_login");
    }

    #[test]
    fn test_sanitize_segment_empty_returns_empty() {
        assert_eq!(sanitize_segment("!!!"), "");
    }

    #[test]
    fn test_generate_with_prefix() {
        let mut id_gen = IdGenerator::new(Some("auth"));
        let (id, collision) = id_gen.generate("Login Flow");
        assert_eq!(id, "auth.login_flow");
        assert!(!collision);
    }

    #[test]
    fn test_generate_without_prefix() {
        let mut id_gen = IdGenerator::new(None);
        let (id, collision) = id_gen.generate("Login Flow");
        assert_eq!(id, "login_flow");
        assert!(!collision);
    }

    #[test]
    fn test_generate_collision_appends_suffix() {
        let mut id_gen = IdGenerator::new(None);
        let (id1, c1) = id_gen.generate("Login Flow");
        let (id2, c2) = id_gen.generate("Login Flow");
        assert_eq!(id1, "login_flow");
        assert!(!c1);
        assert_eq!(id2, "login_flow.1");
        assert!(c2);
    }

    #[test]
    fn test_generate_multiple_collisions() {
        let mut id_gen = IdGenerator::new(None);
        id_gen.generate("Test");
        id_gen.generate("Test");
        let (id3, c3) = id_gen.generate("Test");
        assert_eq!(id3, "test.2");
        assert!(c3);
    }

    #[test]
    fn test_generate_empty_heading_uses_fallback() {
        let mut id_gen = IdGenerator::new(None);
        let (id, _) = id_gen.generate("!!!");
        assert!(id.starts_with("node"));
    }
}
