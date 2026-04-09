//! Shared sidecar line classifier and lexer.
//!
//! Used by both the state parser (`parser/state.rs`) and the mem parser
//! (`parser/mem.rs`). Do NOT modify `lexer.rs` — this is a separate lexer
//! for sidecar file formats.

use crate::error::{AgmError, ErrorCode, ErrorLocation};

// ---------------------------------------------------------------------------
// SidecarLineKind
// ---------------------------------------------------------------------------

/// Classification of a single line in a sidecar file.
#[derive(Debug, Clone, PartialEq)]
pub enum SidecarLineKind {
    /// A commented header line: `# key: value`
    Header(String, String),
    /// A block declaration: `state <id>` or `entry <key>`
    BlockDecl(String, String),
    /// A key-value field: `key: value` (value may be empty)
    Field(String, String),
    /// An indented continuation line (2+ leading spaces); content has the 2 spaces stripped.
    Continuation(String),
    /// A blank or whitespace-only line.
    Blank,
    /// A comment that does not match the `# key: value` header pattern.
    Comment(String),
}

// ---------------------------------------------------------------------------
// SidecarLine
// ---------------------------------------------------------------------------

/// A classified line with its original position and raw content.
#[derive(Debug, Clone, PartialEq)]
pub struct SidecarLine {
    pub kind: SidecarLineKind,
    pub number: usize,
    pub raw: String,
}

// ---------------------------------------------------------------------------
// classify_sidecar_line
// ---------------------------------------------------------------------------

/// Classifies a single raw line from a sidecar file.
///
/// Priority order:
/// 1. Empty/whitespace -> [`SidecarLineKind::Blank`]
/// 2. Starts with `"# "` -> Header if matches `# key: value`, else Comment
/// 3. Starts with `"#"` alone -> Comment("")
/// 4. Matches `^(state|entry) (.+)$` -> [`SidecarLineKind::BlockDecl`]
/// 5. Starts with 2+ spaces -> [`SidecarLineKind::Continuation`] (2 spaces stripped)
/// 6. Matches `^([a-z_][a-z0-9_]*): (.*)$` or `^([a-z_][a-z0-9_]*):$` -> Field
#[must_use]
pub fn classify_sidecar_line(line: &str) -> SidecarLineKind {
    // 1. Blank
    if line.trim().is_empty() {
        return SidecarLineKind::Blank;
    }

    // 2 & 3. Comment / Header
    if let Some(rest_of_hash) = line.strip_prefix('#') {
        let after_hash = if let Some(stripped) = line.strip_prefix("# ") {
            stripped
        } else {
            // bare `#` or `##...`
            return SidecarLineKind::Comment(rest_of_hash.trim_start().to_owned());
        };

        // Try to parse as `key: value` header
        if let Some(colon_pos) = after_hash.find(": ") {
            let key = &after_hash[..colon_pos];
            let value = &after_hash[colon_pos + 2..];
            if is_header_key(key) {
                return SidecarLineKind::Header(key.to_owned(), value.to_owned());
            }
        }
        // Also handle `key:` with empty value (no trailing space)
        if let Some(key) = after_hash.strip_suffix(':') {
            if is_header_key(key) {
                return SidecarLineKind::Header(key.to_owned(), String::new());
            }
        }
        return SidecarLineKind::Comment(after_hash.to_owned());
    }

    // 4. BlockDecl: `state <id>` or `entry <key>`
    if let Some(rest) = line.strip_prefix("state ") {
        let id = rest.trim_end();
        if !id.is_empty() {
            return SidecarLineKind::BlockDecl("state".to_owned(), id.to_owned());
        }
    }
    if let Some(rest) = line.strip_prefix("entry ") {
        let id = rest.trim_end();
        if !id.is_empty() {
            return SidecarLineKind::BlockDecl("entry".to_owned(), id.to_owned());
        }
    }

    // 5. Continuation (2+ leading spaces)
    if let Some(stripped) = line.strip_prefix("  ") {
        return SidecarLineKind::Continuation(stripped.to_owned());
    }

    // 6. Field: `key: value` or `key:`
    if let Some(colon_pos) = line.find(':') {
        let key = &line[..colon_pos];
        if is_field_key(key) {
            let rest = &line[colon_pos + 1..];
            let value = if let Some(stripped) = rest.strip_prefix(' ') {
                stripped.to_owned()
            } else {
                rest.to_owned()
            };
            return SidecarLineKind::Field(key.to_owned(), value);
        }
    }

    // Fallback: treat as a comment / unknown
    SidecarLineKind::Comment(line.to_owned())
}

/// Returns true if `s` looks like a valid header key: `[a-z][a-z0-9_.]*`
fn is_header_key(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.')
}

/// Returns true if `s` looks like a valid field key: `[a-z_][a-z0-9_]*`
fn is_field_key(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

// ---------------------------------------------------------------------------
// lex_sidecar
// ---------------------------------------------------------------------------

/// Tokenises every line of a sidecar file.
///
/// Returns a `Vec<SidecarLine>` on success. Returns `Err(Vec<AgmError>)` if
/// any line contains a tab character (error P004).
pub fn lex_sidecar(input: &str) -> Result<Vec<SidecarLine>, Vec<AgmError>> {
    let mut lines = Vec::new();
    let mut errors = Vec::new();

    for (index, raw) in input.lines().enumerate() {
        let number = index + 1;

        // P004: tabs not allowed
        if raw.contains('\t') {
            errors.push(AgmError::new(
                ErrorCode::P004,
                format!("Tab character in indentation at line {number} (spaces required)"),
                ErrorLocation::new(None, Some(number), None),
            ));
            continue;
        }

        let kind = classify_sidecar_line(raw);
        lines.push(SidecarLine {
            kind,
            number,
            raw: raw.to_owned(),
        });
    }

    if errors.is_empty() {
        Ok(lines)
    } else {
        Err(errors)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // A: Blank lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_empty_string_is_blank() {
        assert_eq!(classify_sidecar_line(""), SidecarLineKind::Blank);
    }

    #[test]
    fn test_classify_whitespace_only_is_blank() {
        assert_eq!(classify_sidecar_line("   "), SidecarLineKind::Blank);
    }

    // -----------------------------------------------------------------------
    // B: Header lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_hash_key_value_is_header() {
        assert_eq!(
            classify_sidecar_line("# agm.state: 1.0"),
            SidecarLineKind::Header("agm.state".to_owned(), "1.0".to_owned())
        );
    }

    #[test]
    fn test_classify_header_package() {
        assert_eq!(
            classify_sidecar_line("# package: test.pkg"),
            SidecarLineKind::Header("package".to_owned(), "test.pkg".to_owned())
        );
    }

    #[test]
    fn test_classify_header_session_id() {
        assert_eq!(
            classify_sidecar_line("# session_id: run-001"),
            SidecarLineKind::Header("session_id".to_owned(), "run-001".to_owned())
        );
    }

    #[test]
    fn test_classify_comment_no_colon() {
        assert_eq!(
            classify_sidecar_line("# just a comment"),
            SidecarLineKind::Comment("just a comment".to_owned())
        );
    }

    #[test]
    fn test_classify_comment_uppercase_key_not_header() {
        // Key has uppercase — not a valid header key
        assert_eq!(
            classify_sidecar_line("# Package: test.pkg"),
            SidecarLineKind::Comment("Package: test.pkg".to_owned())
        );
    }

    #[test]
    fn test_classify_bare_hash_is_comment() {
        assert_eq!(
            classify_sidecar_line("#"),
            SidecarLineKind::Comment(String::new())
        );
    }

    // -----------------------------------------------------------------------
    // C: BlockDecl lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_state_block_decl() {
        assert_eq!(
            classify_sidecar_line("state migration.025.data"),
            SidecarLineKind::BlockDecl("state".to_owned(), "migration.025.data".to_owned())
        );
    }

    #[test]
    fn test_classify_entry_block_decl() {
        assert_eq!(
            classify_sidecar_line("entry project.db_version"),
            SidecarLineKind::BlockDecl("entry".to_owned(), "project.db_version".to_owned())
        );
    }

    // -----------------------------------------------------------------------
    // D: Field lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_field_with_value() {
        assert_eq!(
            classify_sidecar_line("execution_status: completed"),
            SidecarLineKind::Field("execution_status".to_owned(), "completed".to_owned())
        );
    }

    #[test]
    fn test_classify_field_empty_value() {
        assert_eq!(
            classify_sidecar_line("execution_log:"),
            SidecarLineKind::Field("execution_log".to_owned(), String::new())
        );
    }

    #[test]
    fn test_classify_field_retry_count() {
        assert_eq!(
            classify_sidecar_line("retry_count: 0"),
            SidecarLineKind::Field("retry_count".to_owned(), "0".to_owned())
        );
    }

    // -----------------------------------------------------------------------
    // E: Continuation lines
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_two_spaces_is_continuation() {
        assert_eq!(
            classify_sidecar_line("  continuation content"),
            SidecarLineKind::Continuation("continuation content".to_owned())
        );
    }

    #[test]
    fn test_classify_four_spaces_is_continuation_strips_two() {
        assert_eq!(
            classify_sidecar_line("    deeper content"),
            SidecarLineKind::Continuation("  deeper content".to_owned())
        );
    }

    // -----------------------------------------------------------------------
    // F: lex_sidecar
    // -----------------------------------------------------------------------

    #[test]
    fn test_lex_sidecar_simple_input_returns_ok() {
        let input =
            "# agm.state: 1.0\n# package: test.pkg\n\nstate node.one\nexecution_status: pending\n";
        let lines = lex_sidecar(input).unwrap();
        assert_eq!(lines.len(), 5);
        assert_eq!(
            lines[0].kind,
            SidecarLineKind::Header("agm.state".to_owned(), "1.0".to_owned())
        );
        assert_eq!(lines[2].kind, SidecarLineKind::Blank);
    }

    #[test]
    fn test_lex_sidecar_tab_returns_error_p004() {
        let input = "# agm.state: 1.0\n\texecution_status: pending\n";
        let errors = lex_sidecar(input).unwrap_err();
        assert!(!errors.is_empty());
        assert_eq!(errors[0].code, ErrorCode::P004);
    }

    #[test]
    fn test_lex_sidecar_line_numbers_start_at_one() {
        let input = "# agm.state: 1.0\n# package: test\n";
        let lines = lex_sidecar(input).unwrap();
        assert_eq!(lines[0].number, 1);
        assert_eq!(lines[1].number, 2);
    }
}
