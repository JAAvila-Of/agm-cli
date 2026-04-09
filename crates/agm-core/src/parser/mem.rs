//! Parser for `.agm.mem` sidecar files.

use std::collections::BTreeMap;
use std::str::FromStr;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::mem_file::{MemFile, MemFileEntry};
use crate::model::memory::{MemoryScope, MemoryTtl};
use crate::parser::ParseResult;
use crate::parser::sidecar::{SidecarLineKind, lex_sidecar};

// ---------------------------------------------------------------------------
// parse_mem
// ---------------------------------------------------------------------------

/// Parses raw `.agm.mem` text into a [`MemFile`].
///
/// Returns `Err(Vec<AgmError>)` if any Error-severity diagnostics are
/// produced (missing required headers/fields, bad enum values, duplicate
/// entry keys). Warnings (unknown fields) keep the parse alive.
pub fn parse_mem(input: &str) -> ParseResult<MemFile> {
    let lines = lex_sidecar(input)?;
    let mut pos = 0;
    let mut errors: Vec<AgmError> = Vec::new();

    // ------------------------------------------------------------------
    // 1. Consume header lines
    // ------------------------------------------------------------------
    let mut format_version: Option<String> = None;
    let mut package: Option<String> = None;
    let mut updated_at: Option<String> = None;

    while pos < lines.len() {
        match &lines[pos].kind {
            SidecarLineKind::Blank | SidecarLineKind::Comment(_) => {
                pos += 1;
            }
            SidecarLineKind::Header(key, value) => {
                match key.as_str() {
                    "agm.mem" => format_version = Some(value.clone()),
                    "package" => package = Some(value.clone()),
                    "updated_at" => updated_at = Some(value.clone()),
                    _ => {
                        errors.push(AgmError::new(
                            ErrorCode::P009,
                            format!("Unknown header field '{}' in mem file", key),
                            ErrorLocation::new(None, Some(lines[pos].number), None),
                        ));
                    }
                }
                pos += 1;
            }
            _ => break,
        }
    }

    // Validate required headers
    for (field, present) in [
        ("agm.mem", format_version.is_some()),
        ("package", package.is_some()),
        ("updated_at", updated_at.is_some()),
    ] {
        if !present {
            errors.push(AgmError::new(
                ErrorCode::P001,
                format!("Missing required header field '{field}' in mem file"),
                ErrorLocation::new(None, Some(1), None),
            ));
        }
    }

    // ------------------------------------------------------------------
    // 2. Parse entry blocks
    // ------------------------------------------------------------------
    let mut entries: BTreeMap<String, MemFileEntry> = BTreeMap::new();

    while pos < lines.len() {
        match &lines[pos].kind {
            SidecarLineKind::Blank | SidecarLineKind::Comment(_) => {
                pos += 1;
            }
            SidecarLineKind::BlockDecl(keyword, entry_key) if keyword == "entry" => {
                let entry_key = entry_key.clone();
                let block_line = lines[pos].number;
                pos += 1;

                let mut topic: Option<String> = None;
                let mut scope: Option<MemoryScope> = None;
                let mut ttl: Option<MemoryTtl> = None;
                let mut value: Option<String> = None;
                let mut created_at: Option<String> = None;
                let mut entry_updated_at: Option<String> = None;

                while pos < lines.len() {
                    match &lines[pos].kind {
                        SidecarLineKind::Blank => break,
                        SidecarLineKind::Comment(_) => {
                            pos += 1;
                        }
                        SidecarLineKind::BlockDecl(_, _) => break,
                        SidecarLineKind::Field(key, fvalue) => {
                            let field_line = lines[pos].number;
                            match key.as_str() {
                                "topic" => topic = Some(fvalue.clone()),
                                "scope" => match MemoryScope::from_str(fvalue) {
                                    Ok(s) => scope = Some(s),
                                    Err(_) => {
                                        errors.push(AgmError::new(
                                            ErrorCode::P003,
                                            format!(
                                                "Invalid scope value '{}' in entry '{}'",
                                                fvalue, entry_key
                                            ),
                                            ErrorLocation::new(None, Some(field_line), None),
                                        ));
                                    }
                                },
                                "ttl" => match MemoryTtl::from_str(fvalue) {
                                    Ok(t) => ttl = Some(t),
                                    Err(_) => {
                                        errors.push(AgmError::new(
                                            ErrorCode::P003,
                                            format!(
                                                "Invalid ttl value '{}' in entry '{}'",
                                                fvalue, entry_key
                                            ),
                                            ErrorLocation::new(None, Some(field_line), None),
                                        ));
                                    }
                                },
                                "value" => {
                                    // Collect potential continuation lines
                                    let mut collected = fvalue.clone();
                                    pos += 1;
                                    while pos < lines.len() {
                                        if let SidecarLineKind::Continuation(cont) =
                                            &lines[pos].kind
                                        {
                                            collected.push('\n');
                                            collected.push_str(cont);
                                            pos += 1;
                                        } else {
                                            break;
                                        }
                                    }
                                    value = Some(collected);
                                    continue; // pos already advanced
                                }
                                "created_at" => created_at = Some(fvalue.clone()),
                                "updated_at" => entry_updated_at = Some(fvalue.clone()),
                                unknown => {
                                    errors.push(AgmError::new(
                                        ErrorCode::P009,
                                        format!(
                                            "Unknown field '{}' in entry block '{}'",
                                            unknown, entry_key
                                        ),
                                        ErrorLocation::new(None, Some(field_line), None),
                                    ));
                                }
                            }
                            pos += 1;
                        }
                        SidecarLineKind::Header(_, _) | SidecarLineKind::Continuation(_) => {
                            pos += 1;
                        }
                    }
                }

                // Validate required fields
                let mut entry_errors = false;
                for (field, present) in [
                    ("topic", topic.is_some()),
                    ("scope", scope.is_some()),
                    ("ttl", ttl.is_some()),
                    ("value", value.is_some()),
                    ("created_at", created_at.is_some()),
                    ("updated_at", entry_updated_at.is_some()),
                ] {
                    if !present {
                        errors.push(AgmError::new(
                            ErrorCode::P001,
                            format!(
                                "Missing required field '{}' in entry block '{}'",
                                field, entry_key
                            ),
                            ErrorLocation::new(None, Some(block_line), None),
                        ));
                        entry_errors = true;
                    }
                }

                if !entry_errors {
                    // Duplicate entry key check
                    use std::collections::btree_map::Entry;
                    match entries.entry(entry_key.clone()) {
                        Entry::Occupied(_) => {
                            errors.push(AgmError::new(
                                ErrorCode::P006,
                                format!("Duplicate entry key '{}' in mem file", entry_key),
                                ErrorLocation::new(None, Some(block_line), None),
                            ));
                        }
                        Entry::Vacant(slot) => {
                            slot.insert(MemFileEntry {
                                topic: topic.unwrap(),
                                scope: scope.unwrap(),
                                ttl: ttl.unwrap(),
                                value: value.unwrap(),
                                created_at: created_at.unwrap(),
                                updated_at: entry_updated_at.unwrap(),
                            });
                        }
                    }
                }
            }
            SidecarLineKind::BlockDecl(keyword, _) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!(
                        "Unexpected block keyword '{}' in mem file (expected 'entry')",
                        keyword
                    ),
                    ErrorLocation::new(None, Some(lines[pos].number), None),
                ));
                pos += 1;
            }
            SidecarLineKind::Field(key, _) => {
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Field '{}' outside of an 'entry' block", key),
                    ErrorLocation::new(None, Some(lines[pos].number), None),
                ));
                pos += 1;
            }
            SidecarLineKind::Continuation(_) | SidecarLineKind::Header(_, _) => {
                pos += 1;
            }
        }
    }

    // ------------------------------------------------------------------
    // 3. Return
    // ------------------------------------------------------------------
    if errors.iter().any(|e| e.is_error()) {
        Err(errors)
    } else {
        Ok(MemFile {
            format_version: format_version.unwrap_or_default(),
            package: package.unwrap_or_default(),
            updated_at: updated_at.unwrap_or_default(),
            entries,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::model::memory::{MemoryScope, MemoryTtl};

    fn minimal_mem() -> &'static str {
        "# agm.mem: 1.0\n\
         # package: test.pkg\n\
         # updated_at: 2026-04-08T10:00:00Z\n"
    }

    fn errors_contain(errors: &[AgmError], code: ErrorCode) -> bool {
        errors.iter().any(|e| e.code == code)
    }

    fn full_entry(key: &str) -> String {
        format!(
            "entry {key}\n\
             topic: infrastructure\n\
             scope: project\n\
             ttl: permanent\n\
             value: some value\n\
             created_at: 2026-04-08T10:00:00Z\n\
             updated_at: 2026-04-08T10:00:00Z\n"
        )
    }

    // -----------------------------------------------------------------------
    // A: Valid minimal — no entries
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_minimal_valid_returns_ok() {
        let result = parse_mem(minimal_mem());
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        let mf = result.unwrap();
        assert_eq!(mf.format_version, "1.0");
        assert_eq!(mf.package, "test.pkg");
        assert!(mf.entries.is_empty());
    }

    // -----------------------------------------------------------------------
    // B: Valid full — with entries
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_full_valid_returns_entries() {
        let input = format!(
            "{}\n\
             entry project.db_version\n\
             topic: infrastructure\n\
             scope: project\n\
             ttl: permanent\n\
             value: PostgreSQL 15.2\n\
             created_at: 2026-04-08T15:30:00Z\n\
             updated_at: 2026-04-08T15:30:00Z\n",
            minimal_mem()
        );
        let mf = parse_mem(&input).unwrap();
        assert_eq!(mf.entries.len(), 1);
        let entry = &mf.entries["project.db_version"];
        assert_eq!(entry.topic, "infrastructure");
        assert_eq!(entry.scope, MemoryScope::Project);
        assert_eq!(entry.ttl, MemoryTtl::Permanent);
        assert_eq!(entry.value, "PostgreSQL 15.2");
    }

    // -----------------------------------------------------------------------
    // C: Multi-line value
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_multiline_value_joined_with_newlines() {
        // Continuation lines must start with exactly 2 spaces in the raw text.
        let input = format!(
            "{base}\nentry project.notes\ntopic: documentation\nscope: project\nttl: permanent\nvalue: Primera linea del valor\n  Segunda linea continuada\n  Tercera linea continuada\ncreated_at: 2026-04-08T10:00:00Z\nupdated_at: 2026-04-08T10:00:00Z\n",
            base = minimal_mem()
        );
        let mf = parse_mem(&input).unwrap();
        let entry = &mf.entries["project.notes"];
        assert!(entry.value.contains('\n'));
        assert!(entry.value.contains("Primera linea"));
        assert!(entry.value.contains("Segunda linea"));
        assert!(entry.value.contains("Tercera linea"));
    }

    // -----------------------------------------------------------------------
    // D: Missing required header
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_missing_agm_mem_header_returns_p001() {
        let input = "# package: test.pkg\n# updated_at: 2026-04-08T10:00:00Z\n";
        let errors = parse_mem(input).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("agm.mem"))
        );
    }

    #[test]
    fn test_parse_mem_missing_package_returns_p001() {
        let input = "# agm.mem: 1.0\n# updated_at: 2026-04-08T10:00:00Z\n";
        let errors = parse_mem(input).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("package"))
        );
    }

    #[test]
    fn test_parse_mem_missing_updated_at_returns_p001() {
        let input = "# agm.mem: 1.0\n# package: test.pkg\n";
        let errors = parse_mem(input).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("updated_at"))
        );
    }

    // -----------------------------------------------------------------------
    // E: Duplicate entry key
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_duplicate_entry_key_returns_p006() {
        let input = format!(
            "{}\n{}\n{}",
            minimal_mem(),
            full_entry("dup.key"),
            full_entry("dup.key")
        );
        let errors = parse_mem(&input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P006));
    }

    // -----------------------------------------------------------------------
    // F: Invalid scope
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_bad_scope_returns_p003() {
        let input = format!(
            "{}\n\
             entry bad.scope\n\
             topic: infra\n\
             scope: workspace\n\
             ttl: permanent\n\
             value: test\n\
             created_at: 2026-04-08T10:00:00Z\n\
             updated_at: 2026-04-08T10:00:00Z\n",
            minimal_mem()
        );
        let errors = parse_mem(&input).unwrap_err();
        assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
    }

    // -----------------------------------------------------------------------
    // G: Invalid TTL
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_bad_ttl_returns_p003() {
        let input = format!(
            "{}\n\
             entry bad.ttl\n\
             topic: infra\n\
             scope: project\n\
             ttl: forever\n\
             value: test\n\
             created_at: 2026-04-08T10:00:00Z\n\
             updated_at: 2026-04-08T10:00:00Z\n",
            minimal_mem()
        );
        let errors = parse_mem(&input).unwrap_err();
        assert!(errors.iter().any(|e| e.code == ErrorCode::P003));
    }

    // -----------------------------------------------------------------------
    // H: Missing required entry field
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_missing_entry_field_returns_p001() {
        let input = format!(
            "{}\n\
             entry missing.field\n\
             topic: infra\n\
             scope: project\n\
             ttl: permanent\n\
             created_at: 2026-04-08T10:00:00Z\n\
             updated_at: 2026-04-08T10:00:00Z\n",
            minimal_mem()
        );
        // value is missing
        let errors = parse_mem(&input).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("value"))
        );
    }

    // -----------------------------------------------------------------------
    // I: Unknown field produces P009 (warning — parse succeeds)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_unknown_field_returns_p009_warning() {
        let input = format!(
            "{}\n\
             entry ok.entry\n\
             topic: infra\n\
             scope: project\n\
             ttl: permanent\n\
             value: test\n\
             created_at: 2026-04-08T10:00:00Z\n\
             updated_at: 2026-04-08T10:00:00Z\n\
             mystery_field: some value\n",
            minimal_mem()
        );
        // P009 is warning — should parse OK
        let result = parse_mem(&input);
        assert!(
            result.is_ok(),
            "expected Ok with warnings, got: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // J: Duration TTL parsed correctly
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_duration_ttl_parsed_correctly() {
        let input = format!(
            "{}\n\
             entry dur.entry\n\
             topic: infra\n\
             scope: session\n\
             ttl: duration:P30D\n\
             value: test\n\
             created_at: 2026-04-08T10:00:00Z\n\
             updated_at: 2026-04-08T10:00:00Z\n",
            minimal_mem()
        );
        let mf = parse_mem(&input).unwrap();
        assert_eq!(
            mf.entries["dur.entry"].ttl,
            MemoryTtl::Duration("P30D".to_owned())
        );
    }

    // -----------------------------------------------------------------------
    // K: All scopes accepted
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_all_scopes_accepted() {
        for scope_str in ["node", "session", "project", "global"] {
            let input = format!(
                "{}\n\
                 entry scope.test\n\
                 topic: infra\n\
                 scope: {}\n\
                 ttl: permanent\n\
                 value: test\n\
                 created_at: 2026-04-08T10:00:00Z\n\
                 updated_at: 2026-04-08T10:00:00Z\n",
                minimal_mem(),
                scope_str
            );
            let result = parse_mem(&input);
            assert!(
                result.is_ok(),
                "failed for scope '{}': {:?}",
                scope_str,
                result
            );
        }
    }

    // -----------------------------------------------------------------------
    // L: Multiple entries in one file
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_multiple_entries_all_parsed() {
        let input = format!(
            "{}\n{}\n{}",
            minimal_mem(),
            full_entry("key.one"),
            full_entry("key.two")
        );
        let mf = parse_mem(&input).unwrap();
        assert_eq!(mf.entries.len(), 2);
        assert!(mf.entries.contains_key("key.one"));
        assert!(mf.entries.contains_key("key.two"));
    }

    // -----------------------------------------------------------------------
    // M: Comments inside entry block are ignored
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_comments_inside_block_ignored() {
        let input = format!(
            "{}\n\
             entry commented.entry\n\
             # this is a comment\n\
             topic: infra\n\
             scope: project\n\
             ttl: permanent\n\
             value: test\n\
             created_at: 2026-04-08T10:00:00Z\n\
             updated_at: 2026-04-08T10:00:00Z\n",
            minimal_mem()
        );
        let mf = parse_mem(&input).unwrap();
        assert_eq!(mf.entries["commented.entry"].topic, "infra");
    }

    // -----------------------------------------------------------------------
    // N: Multiline value — continuation lines have 2-space prefix stripped
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_multiline_value_two_spaces_stripped() {
        // Continuation line must start with exactly 2 spaces in the raw text.
        let input = format!(
            "{base}\nentry ml.entry\ntopic: docs\nscope: project\nttl: permanent\nvalue: line one\n  line two\ncreated_at: 2026-04-08T10:00:00Z\nupdated_at: 2026-04-08T10:00:00Z\n",
            base = minimal_mem()
        );
        let mf = parse_mem(&input).unwrap();
        let val = &mf.entries["ml.entry"].value;
        assert_eq!(val, "line one\nline two");
    }

    // -----------------------------------------------------------------------
    // O: Empty file is error
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_mem_empty_input_returns_error() {
        let result = parse_mem("");
        assert!(result.is_err());
    }
}
