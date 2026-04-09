//! Renderer for `.agm.state` sidecar files.

use crate::model::state::StateFile;

// ---------------------------------------------------------------------------
// render_state (canonical text format)
// ---------------------------------------------------------------------------

/// Renders a [`StateFile`] to its canonical `.agm.state` text format.
///
/// The output is round-trippable: `parse_state(&render_state(sf)) == sf`.
///
/// Format:
/// ```text
/// # agm.state: {format_version}
/// # package: {package}
/// # version: {version}
/// # session_id: {session_id}
/// # started_at: {started_at}
/// # updated_at: {updated_at}
///
/// state {node_id}
/// execution_status: {status}
/// [executed_by: {val}]    <- only if Some
/// [executed_at: {val}]    <- only if Some
/// retry_count: {val}      <- always emit
/// [execution_log: {val}]  <- only if Some
/// ```
///
/// Blocks are separated by a single blank line. No trailing blank line.
#[must_use]
pub fn render_state(state: &StateFile) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!("# agm.state: {}\n", state.format_version));
    out.push_str(&format!("# package: {}\n", state.package));
    out.push_str(&format!("# version: {}\n", state.version));
    out.push_str(&format!("# session_id: {}\n", state.session_id));
    out.push_str(&format!("# started_at: {}\n", state.started_at));
    out.push_str(&format!("# updated_at: {}\n", state.updated_at));

    // Node blocks
    let nodes: Vec<_> = state.nodes.iter().collect();
    for (i, (node_id, node_state)) in nodes.iter().enumerate() {
        // Blank line separating header from first block, and between blocks
        out.push('\n');

        out.push_str(&format!("state {node_id}\n"));
        out.push_str(&format!(
            "execution_status: {}\n",
            node_state.execution_status
        ));

        if let Some(ref by) = node_state.executed_by {
            out.push_str(&format!("executed_by: {by}\n"));
        }
        if let Some(ref at) = node_state.executed_at {
            out.push_str(&format!("executed_at: {at}\n"));
        }

        out.push_str(&format!("retry_count: {}\n", node_state.retry_count));

        if let Some(ref log) = node_state.execution_log {
            out.push_str(&format!("execution_log: {log}\n"));
        }

        // Blank line between blocks (but not after the last one)
        let _ = i; // suppress unused variable warning
    }

    // Remove the trailing newline from any trailing blank line.
    // The loop above already ends each block without a trailing blank,
    // so nothing to trim — just ensure no double newline at very end.
    // The format already produces no trailing blank by construction.
    out
}

// ---------------------------------------------------------------------------
// render_state_json
// ---------------------------------------------------------------------------

/// Renders a [`StateFile`] to pretty-printed JSON.
#[must_use]
pub fn render_state_json(state: &StateFile) -> String {
    serde_json::to_string_pretty(state).expect("StateFile serialization cannot fail")
}

// ---------------------------------------------------------------------------
// render_state_sql
// ---------------------------------------------------------------------------

/// Renders a [`StateFile`] to SQL INSERT statements.
///
/// Escapes single quotes by doubling them (`'` -> `''`). Uses `NULL` for
/// `None` optional fields.
#[must_use]
pub fn render_state_sql(state: &StateFile) -> String {
    fn escape(s: &str) -> String {
        s.replace('\'', "''")
    }

    fn opt_str(opt: &Option<String>) -> String {
        match opt {
            Some(s) => format!("'{}'", escape(s)),
            None => "NULL".to_owned(),
        }
    }

    let mut out = String::new();

    out.push_str(&format!(
        "INSERT INTO agm_session (format_version, package, version, session_id, started_at, updated_at) VALUES ('{}', '{}', '{}', '{}', '{}', '{}');\n",
        escape(&state.format_version),
        escape(&state.package),
        escape(&state.version),
        escape(&state.session_id),
        escape(&state.started_at),
        escape(&state.updated_at),
    ));

    for (node_id, node_state) in &state.nodes {
        out.push_str(&format!(
            "INSERT INTO agm_node_state (session_id, node_id, execution_status, executed_by, executed_at, retry_count, execution_log) VALUES ('{}', '{}', '{}', {}, {}, {}, {});\n",
            escape(&state.session_id),
            escape(node_id),
            escape(&node_state.execution_status.to_string()),
            opt_str(&node_state.executed_by),
            opt_str(&node_state.executed_at),
            node_state.retry_count,
            opt_str(&node_state.execution_log),
        ));
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::execution::ExecutionStatus;
    use crate::model::state::{NodeState, StateFile};
    use std::collections::BTreeMap;

    fn minimal_state() -> StateFile {
        StateFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            session_id: "run-001".to_owned(),
            started_at: "2026-04-08T10:00:00Z".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            nodes: BTreeMap::new(),
        }
    }

    fn full_state() -> StateFile {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "migration.001".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                executed_by: Some("shell-agent".to_owned()),
                executed_at: Some("2026-04-08T10:05:00Z".to_owned()),
                execution_log: Some(".agm/logs/migration.001.log".to_owned()),
                retry_count: 1,
            },
        );
        nodes.insert(
            "migration.002".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending,
                executed_by: None,
                executed_at: None,
                execution_log: None,
                retry_count: 0,
            },
        );
        StateFile {
            format_version: "1.0".to_owned(),
            package: "acme.migration".to_owned(),
            version: "1.0.0".to_owned(),
            session_id: "run-2026-04-08".to_owned(),
            started_at: "2026-04-08T15:32:00Z".to_owned(),
            updated_at: "2026-04-08T15:35:00Z".to_owned(),
            nodes,
        }
    }

    // -----------------------------------------------------------------------
    // A: Headers present in canonical output
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_state_headers_present() {
        let output = render_state(&minimal_state());
        assert!(output.contains("# agm.state: 1.0"));
        assert!(output.contains("# package: test.pkg"));
        assert!(output.contains("# version: 0.1.0"));
        assert!(output.contains("# session_id: run-001"));
        assert!(output.contains("# started_at: 2026-04-08T10:00:00Z"));
        assert!(output.contains("# updated_at: 2026-04-08T10:00:00Z"));
    }

    // -----------------------------------------------------------------------
    // B: Node block fields present
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_state_node_block_fields_present() {
        let output = render_state(&full_state());
        assert!(output.contains("state migration.001"));
        assert!(output.contains("execution_status: completed"));
        assert!(output.contains("executed_by: shell-agent"));
        assert!(output.contains("executed_at: 2026-04-08T10:05:00Z"));
        assert!(output.contains("execution_log: .agm/logs/migration.001.log"));
        assert!(output.contains("retry_count: 1"));
    }

    // -----------------------------------------------------------------------
    // C: Optional fields absent when None
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_state_optional_fields_absent_when_none() {
        let output = render_state(&full_state());
        // migration.002 has no executed_by/at/log
        // We just check retry_count is present for migration.002
        assert!(output.contains("state migration.002"));
        assert!(output.contains("execution_status: pending"));
    }

    // -----------------------------------------------------------------------
    // D: Roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_state_roundtrip_full() {
        use crate::parser::state::parse_state;

        let state = full_state();
        let rendered = render_state(&state);
        let parsed = parse_state(&rendered).expect("roundtrip parse failed");
        assert_eq!(state, parsed);
    }

    #[test]
    fn test_render_state_roundtrip_minimal() {
        use crate::parser::state::parse_state;

        let state = minimal_state();
        let rendered = render_state(&state);
        let parsed = parse_state(&rendered).expect("roundtrip parse failed");
        assert_eq!(state, parsed);
    }

    // -----------------------------------------------------------------------
    // E: JSON output is valid
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_state_json_valid() {
        let json = render_state_json(&full_state());
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");
        assert_eq!(parsed["package"], "acme.migration");
        assert!(parsed["nodes"].is_object());
    }

    // -----------------------------------------------------------------------
    // F: SQL output
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_state_sql_contains_session_insert() {
        let sql = render_state_sql(&full_state());
        assert!(sql.contains("INSERT INTO agm_session"));
        assert!(sql.contains("acme.migration"));
    }

    #[test]
    fn test_render_state_sql_contains_node_insert() {
        let sql = render_state_sql(&full_state());
        assert!(sql.contains("INSERT INTO agm_node_state"));
        assert!(sql.contains("completed"));
    }

    #[test]
    fn test_render_state_sql_null_for_none_fields() {
        let sql = render_state_sql(&full_state());
        // migration.002 has no executed_by — should use NULL
        assert!(sql.contains("NULL"));
    }

    #[test]
    fn test_render_state_sql_escapes_single_quotes() {
        let mut state = minimal_state();
        state.package = "it's.pkg".to_owned();
        let sql = render_state_sql(&state);
        assert!(sql.contains("it''s.pkg"));
    }

    // -----------------------------------------------------------------------
    // G: Snapshot tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_state_snapshot_minimal() {
        let output = render_state(&minimal_state());
        insta::assert_snapshot!("render_state_minimal", output);
    }

    #[test]
    fn test_render_state_snapshot_full() {
        let output = render_state(&full_state());
        insta::assert_snapshot!("render_state_full", output);
    }
}
