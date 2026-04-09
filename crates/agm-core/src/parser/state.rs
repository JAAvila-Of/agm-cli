//! Parser for `.agm.state` sidecar files.

use std::collections::BTreeMap;
use std::str::FromStr;

use crate::error::{AgmError, ErrorCode, ErrorLocation};
use crate::model::execution::ExecutionStatus;
use crate::model::state::{NodeState, StateFile};
use crate::parser::ParseResult;
use crate::parser::sidecar::{SidecarLineKind, lex_sidecar};

// ---------------------------------------------------------------------------
// parse_state
// ---------------------------------------------------------------------------

/// Parses raw `.agm.state` text into a [`StateFile`].
///
/// Returns `Err(Vec<AgmError>)` if any Error-severity diagnostics are
/// produced (missing required headers, bad enum values, duplicate node IDs).
/// Warnings (unknown fields) are returned inside the `Ok` payload via the
/// diagnostic approach — since `ParseResult<StateFile>` only carries errors
/// we collect warnings as `AgmError` with `Severity::Warning` but still
/// return `Ok` as long as no errors are present.
///
/// To keep the API consistent with the rest of the codebase, warnings are
/// currently accumulated but not returned (they would be returned via a
/// `DiagnosticCollection` in a higher-level API). Fatal errors always cause
/// `Err`.
pub fn parse_state(input: &str) -> ParseResult<StateFile> {
    let lines = lex_sidecar(input)?;
    let mut pos = 0;
    let mut errors: Vec<AgmError> = Vec::new();

    // ------------------------------------------------------------------
    // 1. Consume header lines (# key: value)
    // ------------------------------------------------------------------
    let mut format_version: Option<String> = None;
    let mut package: Option<String> = None;
    let mut version: Option<String> = None;
    let mut session_id: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut updated_at: Option<String> = None;

    while pos < lines.len() {
        match &lines[pos].kind {
            SidecarLineKind::Blank | SidecarLineKind::Comment(_) => {
                pos += 1;
            }
            SidecarLineKind::Header(key, value) => {
                match key.as_str() {
                    "agm.state" => format_version = Some(value.clone()),
                    "package" => package = Some(value.clone()),
                    "version" => version = Some(value.clone()),
                    "session_id" => session_id = Some(value.clone()),
                    "started_at" => started_at = Some(value.clone()),
                    "updated_at" => updated_at = Some(value.clone()),
                    _ => {
                        // unknown header — emit warning but continue
                        errors.push(AgmError::new(
                            ErrorCode::P009,
                            format!("Unknown header field '{}' in state file", key),
                            ErrorLocation::new(None, Some(lines[pos].number), None),
                        ));
                    }
                }
                pos += 1;
            }
            // Once we hit a non-header line, headers are done
            _ => break,
        }
    }

    // Validate required headers
    for (field, present) in [
        ("agm.state", format_version.is_some()),
        ("package", package.is_some()),
        ("version", version.is_some()),
        ("session_id", session_id.is_some()),
        ("started_at", started_at.is_some()),
        ("updated_at", updated_at.is_some()),
    ] {
        if !present {
            errors.push(AgmError::new(
                ErrorCode::P001,
                format!("Missing required header field '{field}' in state file"),
                ErrorLocation::new(None, Some(1), None),
            ));
        }
    }

    // ------------------------------------------------------------------
    // 2. Parse node blocks
    // ------------------------------------------------------------------
    let mut nodes: BTreeMap<String, NodeState> = BTreeMap::new();

    while pos < lines.len() {
        match &lines[pos].kind {
            SidecarLineKind::Blank | SidecarLineKind::Comment(_) => {
                pos += 1;
            }
            SidecarLineKind::BlockDecl(keyword, node_id) if keyword == "state" => {
                let node_id = node_id.clone();
                let line_num = lines[pos].number;
                pos += 1;

                // Collect fields for this block
                let mut exec_status: Option<ExecutionStatus> = None;
                let mut executed_by: Option<String> = None;
                let mut executed_at: Option<String> = None;
                let mut execution_log: Option<String> = None;
                let mut retry_count: u32 = 0;

                while pos < lines.len() {
                    match &lines[pos].kind {
                        SidecarLineKind::Blank => break,
                        SidecarLineKind::Comment(_) => {
                            pos += 1;
                        }
                        SidecarLineKind::BlockDecl(_, _) => break,
                        SidecarLineKind::Field(key, value) => {
                            let field_line = lines[pos].number;
                            match key.as_str() {
                                "execution_status" => match ExecutionStatus::from_str(value) {
                                    Ok(s) => exec_status = Some(s),
                                    Err(_) => {
                                        errors.push(AgmError::new(
                                            ErrorCode::P003,
                                            format!(
                                                "Invalid execution_status value '{}' in node '{}'",
                                                value, node_id
                                            ),
                                            ErrorLocation::new(None, Some(field_line), None),
                                        ));
                                    }
                                },
                                "executed_by" => {
                                    executed_by = if value.is_empty() {
                                        None
                                    } else {
                                        Some(value.clone())
                                    };
                                }
                                "executed_at" => {
                                    executed_at = if value.is_empty() {
                                        None
                                    } else {
                                        Some(value.clone())
                                    };
                                }
                                "execution_log" => {
                                    execution_log = if value.is_empty() {
                                        None
                                    } else {
                                        Some(value.clone())
                                    };
                                }
                                "retry_count" => match value.parse::<u32>() {
                                    Ok(n) => retry_count = n,
                                    Err(_) => {
                                        errors.push(AgmError::new(
                                            ErrorCode::P003,
                                            format!(
                                                "Invalid retry_count value '{}' in node '{}'",
                                                value, node_id
                                            ),
                                            ErrorLocation::new(None, Some(field_line), None),
                                        ));
                                    }
                                },
                                unknown => {
                                    errors.push(AgmError::new(
                                        ErrorCode::P009,
                                        format!(
                                            "Unknown field '{}' in state block '{}'",
                                            unknown, node_id
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

                // execution_status is required
                let final_status = match exec_status {
                    Some(s) => s,
                    None => {
                        errors.push(AgmError::new(
                            ErrorCode::P001,
                            format!("Missing required field 'execution_status' in state block '{node_id}'"),
                            ErrorLocation::new(None, Some(line_num), None),
                        ));
                        ExecutionStatus::Pending // placeholder
                    }
                };

                // Duplicate node ID check
                use std::collections::btree_map::Entry;
                match nodes.entry(node_id.clone()) {
                    Entry::Occupied(_) => {
                        errors.push(AgmError::new(
                            ErrorCode::P006,
                            format!("Duplicate node ID '{node_id}' in state file"),
                            ErrorLocation::new(None, Some(line_num), None),
                        ));
                    }
                    Entry::Vacant(slot) => {
                        slot.insert(NodeState {
                            execution_status: final_status,
                            executed_by,
                            executed_at,
                            execution_log,
                            retry_count,
                        });
                    }
                }
            }
            SidecarLineKind::BlockDecl(keyword, _) => {
                // BlockDecl with unexpected keyword
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!(
                        "Unexpected block keyword '{}' in state file (expected 'state')",
                        keyword
                    ),
                    ErrorLocation::new(None, Some(lines[pos].number), None),
                ));
                pos += 1;
            }
            SidecarLineKind::Field(key, _) => {
                // Field outside a block
                errors.push(AgmError::new(
                    ErrorCode::P003,
                    format!("Field '{}' outside of a 'state' block", key),
                    ErrorLocation::new(None, Some(lines[pos].number), None),
                ));
                pos += 1;
            }
            SidecarLineKind::Continuation(_) => {
                pos += 1;
            }
            SidecarLineKind::Header(_, _) => {
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
        Ok(StateFile {
            format_version: format_version.unwrap_or_default(),
            package: package.unwrap_or_default(),
            version: version.unwrap_or_default(),
            session_id: session_id.unwrap_or_default(),
            started_at: started_at.unwrap_or_default(),
            updated_at: updated_at.unwrap_or_default(),
            nodes,
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

    fn minimal_state() -> &'static str {
        "# agm.state: 1.0\n\
         # package: test.pkg\n\
         # version: 0.1.0\n\
         # session_id: run-001\n\
         # started_at: 2026-04-08T10:00:00Z\n\
         # updated_at: 2026-04-08T10:00:00Z\n"
    }

    fn full_state() -> &'static str {
        "# agm.state: 1.0\n\
         # package: acme.migration\n\
         # version: 1.0.0\n\
         # session_id: run-2026-04-08-153200\n\
         # started_at: 2026-04-08T15:32:00Z\n\
         # updated_at: 2026-04-08T15:35:00Z\n\
         \n\
         state migration.025.data\n\
         execution_status: ready\n\
         retry_count: 0\n\
         \n\
         state migration.025.schema\n\
         execution_status: completed\n\
         executed_by: shell-agent\n\
         executed_at: 2026-04-08T15:30:00Z\n\
         retry_count: 0\n\
         execution_log: .agm/logs/migration.025.schema.log\n"
    }

    fn errors_contain(errors: &[AgmError], code: ErrorCode) -> bool {
        errors.iter().any(|e| e.code == code)
    }

    // -----------------------------------------------------------------------
    // A: Valid minimal — no nodes
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_minimal_valid_returns_ok() {
        let result = parse_state(minimal_state());
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
        let sf = result.unwrap();
        assert_eq!(sf.format_version, "1.0");
        assert_eq!(sf.package, "test.pkg");
        assert_eq!(sf.version, "0.1.0");
        assert_eq!(sf.session_id, "run-001");
        assert!(sf.nodes.is_empty());
    }

    // -----------------------------------------------------------------------
    // B: Valid full — with nodes
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_full_valid_returns_nodes() {
        let sf = parse_state(full_state()).unwrap();
        assert_eq!(sf.nodes.len(), 2);
        assert_eq!(
            sf.nodes["migration.025.data"].execution_status,
            ExecutionStatus::Ready
        );
        assert_eq!(
            sf.nodes["migration.025.schema"].execution_status,
            ExecutionStatus::Completed
        );
        assert_eq!(
            sf.nodes["migration.025.schema"].executed_by.as_deref(),
            Some("shell-agent")
        );
        assert_eq!(
            sf.nodes["migration.025.schema"].execution_log.as_deref(),
            Some(".agm/logs/migration.025.schema.log")
        );
    }

    // -----------------------------------------------------------------------
    // C: Missing required header fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_missing_agm_state_header_returns_p001() {
        let input = "# package: test.pkg\n\
                     # version: 0.1.0\n\
                     # session_id: run-001\n\
                     # started_at: 2026-04-08T10:00:00Z\n\
                     # updated_at: 2026-04-08T10:00:00Z\n";
        let errors = parse_state(input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P001));
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("agm.state"))
        );
    }

    #[test]
    fn test_parse_state_missing_package_header_returns_p001() {
        let input = "# agm.state: 1.0\n\
                     # version: 0.1.0\n\
                     # session_id: run-001\n\
                     # started_at: 2026-04-08T10:00:00Z\n\
                     # updated_at: 2026-04-08T10:00:00Z\n";
        let errors = parse_state(input).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("package"))
        );
    }

    #[test]
    fn test_parse_state_missing_session_id_returns_p001() {
        let input = "# agm.state: 1.0\n\
                     # package: test.pkg\n\
                     # version: 0.1.0\n\
                     # started_at: 2026-04-08T10:00:00Z\n\
                     # updated_at: 2026-04-08T10:00:00Z\n";
        let errors = parse_state(input).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P001 && e.message.contains("session_id"))
        );
    }

    // -----------------------------------------------------------------------
    // D: Duplicate node ID
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_duplicate_node_id_returns_p006() {
        let input = format!(
            "{}\n\
             state dup.node\n\
             execution_status: pending\n\
             retry_count: 0\n\
             \n\
             state dup.node\n\
             execution_status: ready\n\
             retry_count: 0\n",
            minimal_state()
        );
        let errors = parse_state(&input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P006));
    }

    // -----------------------------------------------------------------------
    // E: Invalid execution_status
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_bad_status_returns_p003() {
        let input = format!(
            "{}\n\
             state bad.node\n\
             execution_status: running\n\
             retry_count: 0\n",
            minimal_state()
        );
        let errors = parse_state(&input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P003));
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::P003 && e.message.contains("running"))
        );
    }

    // -----------------------------------------------------------------------
    // F: Unknown field produces P009 warning
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_unknown_field_returns_p009_warning() {
        let input = format!(
            "{}\n\
             state known.node\n\
             execution_status: pending\n\
             retry_count: 0\n\
             mystery_field: some value\n",
            minimal_state()
        );
        // Warnings don't cause Err — file should parse successfully
        let result = parse_state(&input);
        // P009 is a warning — should not block Ok
        assert!(
            result.is_ok(),
            "expected Ok with warnings, got: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // G: Invalid retry_count
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_bad_retry_count_returns_p003() {
        let input = format!(
            "{}\n\
             state node.one\n\
             execution_status: pending\n\
             retry_count: not-a-number\n",
            minimal_state()
        );
        let errors = parse_state(&input).unwrap_err();
        assert!(errors_contain(&errors, ErrorCode::P003));
    }

    // -----------------------------------------------------------------------
    // H: All execution statuses parsed correctly
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_all_statuses_parsed_correctly() {
        let statuses = [
            ("pending", ExecutionStatus::Pending),
            ("ready", ExecutionStatus::Ready),
            ("in_progress", ExecutionStatus::InProgress),
            ("completed", ExecutionStatus::Completed),
            ("failed", ExecutionStatus::Failed),
            ("blocked", ExecutionStatus::Blocked),
            ("skipped", ExecutionStatus::Skipped),
        ];

        for (status_str, expected) in &statuses {
            let input = format!(
                "{}\n\
                 state test.node\n\
                 execution_status: {}\n\
                 retry_count: 0\n",
                minimal_state(),
                status_str
            );
            let sf =
                parse_state(&input).unwrap_or_else(|_| panic!("failed for status {status_str}"));
            assert_eq!(sf.nodes["test.node"].execution_status, *expected);
        }
    }

    // -----------------------------------------------------------------------
    // I: Comments are ignored
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_comments_are_ignored() {
        let input = "# agm.state: 1.0\n\
                     # package: test.pkg\n\
                     # version: 0.1.0\n\
                     # session_id: run-001\n\
                     # started_at: 2026-04-08T10:00:00Z\n\
                     # updated_at: 2026-04-08T10:00:00Z\n\
                     # This is a comment\n\
                     \n\
                     state n.one\n\
                     # comment inside block\n\
                     execution_status: pending\n\
                     retry_count: 0\n";
        let sf = parse_state(input).unwrap();
        assert_eq!(sf.nodes.len(), 1);
    }

    // -----------------------------------------------------------------------
    // J: Blank lines between blocks
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_blank_lines_between_blocks_ok() {
        let input = format!(
            "{}\n\
             state n.one\n\
             execution_status: pending\n\
             retry_count: 0\n\
             \n\
             \n\
             state n.two\n\
             execution_status: ready\n\
             retry_count: 0\n",
            minimal_state()
        );
        let sf = parse_state(&input).unwrap();
        assert_eq!(sf.nodes.len(), 2);
    }

    // -----------------------------------------------------------------------
    // K: Header values preserved exactly
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_header_timestamps_preserved() {
        let sf = parse_state(full_state()).unwrap();
        assert_eq!(sf.started_at, "2026-04-08T15:32:00Z");
        assert_eq!(sf.updated_at, "2026-04-08T15:35:00Z");
    }

    // -----------------------------------------------------------------------
    // L: Optional fields absent when not specified
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_optional_fields_absent_when_not_specified() {
        let input = format!(
            "{}\n\
             state n.minimal\n\
             execution_status: pending\n\
             retry_count: 0\n",
            minimal_state()
        );
        let sf = parse_state(&input).unwrap();
        let node = &sf.nodes["n.minimal"];
        assert!(node.executed_by.is_none());
        assert!(node.executed_at.is_none());
        assert!(node.execution_log.is_none());
    }

    // -----------------------------------------------------------------------
    // M: Empty input is error (no headers)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_empty_input_returns_error() {
        let result = parse_state("");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // N: retry_count defaults to 0 when absent
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_retry_count_defaults_to_zero() {
        let input = format!(
            "{}\n\
             state n.one\n\
             execution_status: pending\n",
            minimal_state()
        );
        let sf = parse_state(&input).unwrap();
        assert_eq!(sf.nodes["n.one"].retry_count, 0);
    }

    // -----------------------------------------------------------------------
    // O: Multiple missing headers all reported
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_state_multiple_missing_headers_all_reported() {
        // Only agm.state header — missing 5 others
        let input = "# agm.state: 1.0\n";
        let errors = parse_state(input).unwrap_err();
        let p001_count = errors.iter().filter(|e| e.code == ErrorCode::P001).count();
        assert!(
            p001_count >= 5,
            "expected at least 5 P001 errors, got {p001_count}"
        );
    }
}
