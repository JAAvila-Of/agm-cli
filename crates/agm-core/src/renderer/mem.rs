//! Renderer for `.agm.mem` sidecar files.

use crate::model::mem_file::MemFile;

// ---------------------------------------------------------------------------
// render_mem (canonical text format)
// ---------------------------------------------------------------------------

/// Renders a [`MemFile`] to its canonical `.agm.mem` text format.
///
/// The output is round-trippable: `parse_mem(&render_mem(mf)) == mf`.
///
/// Multi-line values are serialised with the first line on the `value:` line,
/// and subsequent lines indented with 2 spaces.
///
/// Format:
/// ```text
/// # agm.mem: {format_version}
/// # package: {package}
/// # updated_at: {updated_at}
///
/// entry {key}
/// topic: {topic}
/// scope: {scope}
/// ttl: {ttl}
/// value: {first_line}
///   {continuation_line_2}
///   {continuation_line_3}
/// created_at: {ts}
/// updated_at: {ts}
/// ```
///
/// Blocks are separated by a single blank line. No trailing blank line.
#[must_use]
pub fn render_mem(mem: &MemFile) -> String {
    let mut out = String::new();

    // Header
    out.push_str(&format!("# agm.mem: {}\n", mem.format_version));
    out.push_str(&format!("# package: {}\n", mem.package));
    out.push_str(&format!("# updated_at: {}\n", mem.updated_at));

    // Entry blocks
    for (key, entry) in &mem.entries {
        out.push('\n');

        out.push_str(&format!("entry {key}\n"));
        out.push_str(&format!("topic: {}\n", entry.topic));
        out.push_str(&format!("scope: {}\n", entry.scope));
        out.push_str(&format!("ttl: {}\n", entry.ttl));

        // Multi-line value: first line on `value:` line, rest indented 2 spaces
        let mut value_lines = entry.value.split('\n');
        if let Some(first) = value_lines.next() {
            out.push_str(&format!("value: {first}\n"));
            for cont in value_lines {
                out.push_str(&format!("  {cont}\n"));
            }
        } else {
            out.push_str("value: \n");
        }

        out.push_str(&format!("created_at: {}\n", entry.created_at));
        out.push_str(&format!("updated_at: {}\n", entry.updated_at));
    }

    out
}

// ---------------------------------------------------------------------------
// render_mem_json
// ---------------------------------------------------------------------------

/// Renders a [`MemFile`] to pretty-printed JSON.
#[must_use]
pub fn render_mem_json(mem: &MemFile) -> String {
    serde_json::to_string_pretty(mem).expect("MemFile serialization cannot fail")
}

// ---------------------------------------------------------------------------
// render_mem_sql
// ---------------------------------------------------------------------------

/// Renders a [`MemFile`] to SQL INSERT statements.
///
/// Escapes single quotes by doubling them (`'` -> `''`).
#[must_use]
pub fn render_mem_sql(mem: &MemFile) -> String {
    fn escape(s: &str) -> String {
        s.replace('\'', "''")
    }

    let mut out = String::new();

    for (key, entry) in &mem.entries {
        out.push_str(&format!(
            "INSERT INTO agm_memory (package, entry_key, topic, scope, ttl, value, created_at, updated_at) VALUES ('{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}');\n",
            escape(&mem.package),
            escape(key),
            escape(&entry.topic),
            escape(&entry.scope.to_string()),
            escape(&entry.ttl.to_string()),
            escape(&entry.value),
            escape(&entry.created_at),
            escape(&entry.updated_at),
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
    use crate::model::mem_file::{MemFile, MemFileEntry};
    use crate::model::memory::{MemoryScope, MemoryTtl};
    use std::collections::BTreeMap;

    fn minimal_mem() -> MemFile {
        MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries: BTreeMap::new(),
        }
    }

    fn make_entry(
        key: &str,
        value: &str,
        ttl: MemoryTtl,
        scope: MemoryScope,
    ) -> (String, MemFileEntry) {
        (
            key.to_owned(),
            MemFileEntry {
                topic: "infrastructure".to_owned(),
                scope,
                ttl,
                value: value.to_owned(),
                created_at: "2026-04-08T10:00:00Z".to_owned(),
                updated_at: "2026-04-08T10:00:00Z".to_owned(),
            },
        )
    }

    fn full_mem() -> MemFile {
        let mut entries = BTreeMap::new();
        let (k, e) = make_entry(
            "project.db_version",
            "PostgreSQL 15.2",
            MemoryTtl::Permanent,
            MemoryScope::Project,
        );
        entries.insert(k, e);
        MemFile {
            format_version: "1.0".to_owned(),
            package: "acme.migration".to_owned(),
            updated_at: "2026-04-08T15:30:00Z".to_owned(),
            entries,
        }
    }

    // -----------------------------------------------------------------------
    // A: Headers present
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_headers_present() {
        let output = render_mem(&minimal_mem());
        assert!(output.contains("# agm.mem: 1.0"));
        assert!(output.contains("# package: test.pkg"));
        assert!(output.contains("# updated_at: 2026-04-08T10:00:00Z"));
    }

    // -----------------------------------------------------------------------
    // B: Entry fields present
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_entry_fields_present() {
        let output = render_mem(&full_mem());
        assert!(output.contains("entry project.db_version"));
        assert!(output.contains("topic: infrastructure"));
        assert!(output.contains("scope: project"));
        assert!(output.contains("ttl: permanent"));
        assert!(output.contains("value: PostgreSQL 15.2"));
        assert!(output.contains("created_at: 2026-04-08T10:00:00Z"));
        assert!(output.contains("updated_at: 2026-04-08T10:00:00Z"));
    }

    // -----------------------------------------------------------------------
    // C: Multi-line value rendered with 2-space indent
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_multiline_value_indented() {
        let mut entries = BTreeMap::new();
        let (k, e) = make_entry(
            "ml.entry",
            "line one\nline two\nline three",
            MemoryTtl::Permanent,
            MemoryScope::Project,
        );
        entries.insert(k, e);
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries,
        };
        let output = render_mem(&mem);
        assert!(output.contains("value: line one\n  line two\n  line three\n"));
    }

    // -----------------------------------------------------------------------
    // D: Roundtrip — single-line value
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_roundtrip_single_line_value() {
        use crate::parser::mem::parse_mem;

        let mem = full_mem();
        let rendered = render_mem(&mem);
        let parsed = parse_mem(&rendered).expect("roundtrip parse failed");
        assert_eq!(mem, parsed);
    }

    // -----------------------------------------------------------------------
    // E: Roundtrip — multi-line value
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_roundtrip_multiline_value() {
        use crate::parser::mem::parse_mem;

        let mut entries = BTreeMap::new();
        let (k, e) = make_entry(
            "ml.entry",
            "first line\nsecond line\nthird line",
            MemoryTtl::Permanent,
            MemoryScope::Project,
        );
        entries.insert(k, e);
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries,
        };
        let rendered = render_mem(&mem);
        let parsed = parse_mem(&rendered).expect("roundtrip parse failed");
        assert_eq!(mem, parsed);
    }

    // -----------------------------------------------------------------------
    // F: Roundtrip — minimal (no entries)
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_roundtrip_minimal() {
        use crate::parser::mem::parse_mem;

        let mem = minimal_mem();
        let rendered = render_mem(&mem);
        let parsed = parse_mem(&rendered).expect("roundtrip parse failed");
        assert_eq!(mem, parsed);
    }

    // -----------------------------------------------------------------------
    // G: JSON output is valid
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_json_valid() {
        let json = render_mem_json(&full_mem());
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");
        assert_eq!(parsed["package"], "acme.migration");
        assert!(parsed["entries"].is_object());
    }

    // -----------------------------------------------------------------------
    // H: SQL output
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_sql_contains_insert() {
        let sql = render_mem_sql(&full_mem());
        assert!(sql.contains("INSERT INTO agm_memory"));
        assert!(sql.contains("acme.migration"));
        assert!(sql.contains("project.db_version"));
    }

    #[test]
    fn test_render_mem_sql_escapes_single_quotes() {
        let mut entries = BTreeMap::new();
        let (k, e) = make_entry(
            "test.entry",
            "it's a value",
            MemoryTtl::Permanent,
            MemoryScope::Project,
        );
        entries.insert(k, e);
        let mem = MemFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            entries,
        };
        let sql = render_mem_sql(&mem);
        assert!(sql.contains("it''s a value"));
    }

    // -----------------------------------------------------------------------
    // I: Snapshot tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_mem_snapshot_minimal() {
        let output = render_mem(&minimal_mem());
        insta::assert_snapshot!("render_mem_minimal", output);
    }

    #[test]
    fn test_render_mem_snapshot_full() {
        let output = render_mem(&full_mem());
        insta::assert_snapshot!("render_mem_full", output);
    }
}
