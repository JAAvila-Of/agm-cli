//! Rendering a `DiffReport` to text, JSON, or Markdown.

use super::{ChangeKind, ChangeSeverity, DiffReport};
use crate::diff::fields::FieldValueSnapshot;

/// Output format for diff reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffFormat {
    Text,
    Json,
    Markdown,
}

/// Renders a `DiffReport` in the requested format.
#[must_use]
pub fn render_diff(report: &DiffReport, format: DiffFormat) -> String {
    match format {
        DiffFormat::Text => render_text(report),
        DiffFormat::Json => render_json(report),
        DiffFormat::Markdown => render_markdown(report),
    }
}

// ---------------------------------------------------------------------------
// JSON renderer
// ---------------------------------------------------------------------------

fn render_json(report: &DiffReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Text renderer
// ---------------------------------------------------------------------------

fn render_text(report: &DiffReport) -> String {
    let mut out = String::new();

    out.push_str("=== AGM Semantic Diff ===\n");

    // Header changes
    if !report.header_changes.is_empty() {
        out.push('\n');
        out.push_str("--- Header Changes ---\n");
        for hc in &report.header_changes {
            let prefix = change_prefix(hc.kind);
            let severity_tag = severity_tag(hc.severity);
            let old = hc.old_value.as_deref().unwrap_or("--");
            let new = hc.new_value.as_deref().unwrap_or("--");
            match hc.kind {
                ChangeKind::Modified => {
                    out.push_str(&format!(
                        "  {prefix} {}: {:?} -> {:?} {}\n",
                        hc.field, old, new, severity_tag
                    ));
                }
                ChangeKind::Added => {
                    out.push_str(&format!(
                        "  {prefix} {}: {:?} {}\n",
                        hc.field, new, severity_tag
                    ));
                }
                ChangeKind::Removed => {
                    out.push_str(&format!(
                        "  {prefix} {}: {:?} {}\n",
                        hc.field, old, severity_tag
                    ));
                }
            }
        }
    }

    // Added nodes
    if !report.added_nodes.is_empty() {
        out.push('\n');
        out.push_str(&format!(
            "--- Nodes Added ({}) ---\n",
            report.added_nodes.len()
        ));
        for id in &report.added_nodes {
            out.push_str(&format!("  + {id}\n"));
        }
    }

    // Removed nodes
    if !report.removed_nodes.is_empty() {
        out.push('\n');
        out.push_str(&format!(
            "--- Nodes Removed ({}) ---\n",
            report.removed_nodes.len()
        ));
        for id in &report.removed_nodes {
            out.push_str(&format!("  - {id} [BREAKING]\n"));
        }
    }

    // Modified nodes
    if !report.modified_nodes.is_empty() {
        out.push('\n');
        out.push_str(&format!(
            "--- Nodes Modified ({}) ---\n",
            report.modified_nodes.len()
        ));
        for nd in &report.modified_nodes {
            let breaking_count = nd
                .field_changes
                .iter()
                .filter(|fc| fc.severity == ChangeSeverity::Breaking)
                .count();
            if breaking_count > 0 {
                out.push_str(&format!(
                    "  ~ {} ({} changes, {} breaking)\n",
                    nd.node_id,
                    nd.field_changes.len(),
                    breaking_count
                ));
            } else {
                out.push_str(&format!(
                    "  ~ {} ({} change{})\n",
                    nd.node_id,
                    nd.field_changes.len(),
                    if nd.field_changes.len() == 1 { "" } else { "s" }
                ));
            }
            for fc in &nd.field_changes {
                let prefix = change_prefix(fc.kind);
                let severity_tag = severity_tag(fc.severity);
                let old = snapshot_display(fc.old_value.as_ref());
                let new = snapshot_display(fc.new_value.as_ref());
                match fc.kind {
                    ChangeKind::Modified => {
                        out.push_str(&format!(
                            "    {prefix} {}: {} -> {} {}\n",
                            fc.field, old, new, severity_tag
                        ));
                    }
                    ChangeKind::Added => {
                        out.push_str(&format!(
                            "    {prefix} {}: {} {}\n",
                            fc.field, new, severity_tag
                        ));
                    }
                    ChangeKind::Removed => {
                        out.push_str(&format!(
                            "    {prefix} {}: {} {}\n",
                            fc.field, old, severity_tag
                        ));
                    }
                }
            }
        }
    }

    // Summary
    out.push('\n');
    out.push_str("--- Summary ---\n");
    out.push_str(&format!(
        "  Added: {} | Removed: {} | Modified: {} | Unchanged: {}\n",
        report.summary.nodes_added,
        report.summary.nodes_removed,
        report.summary.nodes_modified,
        report.summary.nodes_unchanged,
    ));
    let breaking_str = if report.summary.has_breaking_changes {
        "YES"
    } else {
        "NO"
    };
    out.push_str(&format!("  Breaking changes: {breaking_str}\n"));

    out
}

// ---------------------------------------------------------------------------
// Markdown renderer
// ---------------------------------------------------------------------------

fn render_markdown(report: &DiffReport) -> String {
    let mut out = String::new();

    out.push_str("# AGM Semantic Diff\n");

    // Summary table
    out.push_str("\n## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("|---|---|\n");
    out.push_str(&format!(
        "| Nodes added | {} |\n",
        report.summary.nodes_added
    ));
    out.push_str(&format!(
        "| Nodes removed | {} |\n",
        report.summary.nodes_removed
    ));
    out.push_str(&format!(
        "| Nodes modified | {} |\n",
        report.summary.nodes_modified
    ));
    out.push_str(&format!(
        "| Nodes unchanged | {} |\n",
        report.summary.nodes_unchanged
    ));
    out.push_str(&format!(
        "| Header changes | {} |\n",
        report.summary.header_changes
    ));
    let breaking_val = if report.summary.has_breaking_changes {
        "**Yes**"
    } else {
        "No"
    };
    out.push_str(&format!("| **Breaking changes** | {breaking_val} |\n"));

    // Header changes table
    if !report.header_changes.is_empty() {
        out.push_str("\n## Header Changes\n\n");
        out.push_str("| Field | Change | Old | New | Severity |\n");
        out.push_str("|---|---|---|---|---|\n");
        for hc in &report.header_changes {
            let old = hc.old_value.as_deref().unwrap_or("--");
            let new = hc.new_value.as_deref().unwrap_or("--");
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                hc.field,
                hc.kind,
                md_escape(old),
                md_escape(new),
                hc.severity
            ));
        }
    }

    // Added nodes
    if !report.added_nodes.is_empty() {
        out.push_str("\n## Added Nodes\n\n");
        for id in &report.added_nodes {
            out.push_str(&format!("- `{id}`\n"));
        }
    }

    // Removed nodes
    if !report.removed_nodes.is_empty() {
        out.push_str("\n## Removed Nodes\n\n");
        for id in &report.removed_nodes {
            out.push_str(&format!("- `{id}` (BREAKING)\n"));
        }
    }

    // Modified nodes
    if !report.modified_nodes.is_empty() {
        out.push_str("\n## Modified Nodes\n");
        for nd in &report.modified_nodes {
            out.push_str(&format!("\n### {}\n\n", nd.node_id));
            out.push_str("| Field | Change | Old | New | Severity |\n");
            out.push_str("|---|---|---|---|---|\n");
            for fc in &nd.field_changes {
                let old = snapshot_display(fc.old_value.as_ref());
                let new = snapshot_display(fc.new_value.as_ref());
                let sev = if fc.severity == ChangeSeverity::Breaking {
                    format!("**{}**", fc.severity)
                } else {
                    fc.severity.to_string()
                };
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    fc.field,
                    fc.kind,
                    md_escape(&old),
                    md_escape(&new),
                    sev
                ));
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn change_prefix(kind: ChangeKind) -> char {
    match kind {
        ChangeKind::Added => '+',
        ChangeKind::Removed => '-',
        ChangeKind::Modified => '~',
    }
}

fn severity_tag(severity: ChangeSeverity) -> &'static str {
    match severity {
        ChangeSeverity::Breaking => "[BREAKING]",
        ChangeSeverity::Minor => "(minor)",
        ChangeSeverity::Info => "(info)",
    }
}

fn snapshot_display(snap: Option<&FieldValueSnapshot>) -> String {
    match snap {
        None => "--".to_owned(),
        Some(FieldValueSnapshot::Scalar(s)) => s.clone(),
        Some(FieldValueSnapshot::List(v)) => format!("[{}]", v.join(", ")),
        Some(FieldValueSnapshot::Block(b)) => b.clone(),
        Some(FieldValueSnapshot::Complex(c)) => c.clone(),
    }
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crate::diff::{
        ChangeKind, ChangeSeverity, DiffReport, DiffSummary, fields::FieldChange,
        fields::FieldValueSnapshot, header::HeaderChange, node::NodeDiff,
    };

    use super::*;

    fn empty_summary() -> DiffSummary {
        DiffSummary {
            nodes_added: 0,
            nodes_removed: 0,
            nodes_modified: 0,
            nodes_unchanged: 0,
            header_changes: 0,
            total_field_changes: 0,
            has_breaking_changes: false,
        }
    }

    fn empty_report() -> DiffReport {
        DiffReport {
            header_changes: vec![],
            added_nodes: vec![],
            removed_nodes: vec![],
            modified_nodes: vec![],
            summary: empty_summary(),
        }
    }

    fn full_report() -> DiffReport {
        DiffReport {
            header_changes: vec![HeaderChange {
                field: "version".to_owned(),
                kind: ChangeKind::Modified,
                severity: ChangeSeverity::Info,
                old_value: Some("0.1.0".to_owned()),
                new_value: Some("0.2.0".to_owned()),
            }],
            added_nodes: vec!["auth.mfa".to_owned()],
            removed_nodes: vec!["auth.legacy".to_owned()],
            modified_nodes: vec![
                NodeDiff {
                    node_id: "auth.login".to_owned(),
                    field_changes: vec![
                        FieldChange {
                            field: "type".to_owned(),
                            kind: ChangeKind::Modified,
                            severity: ChangeSeverity::Breaking,
                            old_value: Some(FieldValueSnapshot::Scalar("workflow".to_owned())),
                            new_value: Some(FieldValueSnapshot::Scalar("rules".to_owned())),
                        },
                        FieldChange {
                            field: "summary".to_owned(),
                            kind: ChangeKind::Modified,
                            severity: ChangeSeverity::Minor,
                            old_value: Some(FieldValueSnapshot::Scalar("old summary".to_owned())),
                            new_value: Some(FieldValueSnapshot::Scalar("new summary".to_owned())),
                        },
                    ],
                    has_breaking_change: true,
                },
                NodeDiff {
                    node_id: "auth.session".to_owned(),
                    field_changes: vec![FieldChange {
                        field: "priority".to_owned(),
                        kind: ChangeKind::Added,
                        severity: ChangeSeverity::Info,
                        old_value: None,
                        new_value: Some(FieldValueSnapshot::Scalar("critical".to_owned())),
                    }],
                    has_breaking_change: false,
                },
            ],
            summary: DiffSummary {
                nodes_added: 1,
                nodes_removed: 1,
                nodes_modified: 2,
                nodes_unchanged: 5,
                header_changes: 1,
                total_field_changes: 3,
                has_breaking_changes: true,
            },
        }
    }

    #[test]
    fn test_render_text_empty_report() {
        let report = empty_report();
        let output = render_diff(&report, DiffFormat::Text);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_render_text_full_report() {
        let report = full_report();
        let output = render_diff(&report, DiffFormat::Text);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_render_text_breaking_only_format() {
        let report = full_report().breaking_only();
        let output = render_diff(&report, DiffFormat::Text);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_render_json_roundtrip() {
        let report = full_report();
        let json = render_diff(&report, DiffFormat::Json);
        let back: DiffReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report, back);
    }

    #[test]
    fn test_render_markdown_empty_report() {
        let report = empty_report();
        let output = render_diff(&report, DiffFormat::Markdown);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn test_render_markdown_full_report() {
        let report = full_report();
        let output = render_diff(&report, DiffFormat::Markdown);
        insta::assert_snapshot!(output);
    }
}
