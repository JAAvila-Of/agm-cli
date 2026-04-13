//! Semantic diff for AGM files.
//!
//! Compares two `AgmFile` values at the structural level -- nodes, fields,
//! and relationships -- rather than raw text. Produces a typed `DiffReport`
//! that classifies every change by kind and severity.

use serde::{Deserialize, Serialize};

use crate::model::file::AgmFile;

pub mod fields;
pub mod header;
pub mod node;
pub mod relation;
pub mod render;

// Re-exports for convenience
pub use fields::{FieldChange, FieldValueSnapshot};
pub use header::HeaderChange;
pub use node::NodeDiff;
pub use render::DiffFormat;

// ---------------------------------------------------------------------------
// ChangeKind
// ---------------------------------------------------------------------------

/// The kind of structural change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
}

impl std::fmt::Display for ChangeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added => write!(f, "added"),
            Self::Removed => write!(f, "removed"),
            Self::Modified => write!(f, "modified"),
        }
    }
}

// ---------------------------------------------------------------------------
// ChangeSeverity
// ---------------------------------------------------------------------------

/// Semantic severity of a change, used for CI gating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSeverity {
    /// Informational (detail text, notes, examples changed).
    Info,
    /// Minor structural change (tags, summary, operational fields).
    Minor,
    /// Potentially breaking (type changed, dependency removed, node removed).
    Breaking,
}

impl std::fmt::Display for ChangeSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Minor => write!(f, "minor"),
            Self::Breaking => write!(f, "breaking"),
        }
    }
}

// ---------------------------------------------------------------------------
// DiffSummary
// ---------------------------------------------------------------------------

/// Aggregate statistics for a diff report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub nodes_added: usize,
    pub nodes_removed: usize,
    pub nodes_modified: usize,
    pub nodes_unchanged: usize,
    pub header_changes: usize,
    pub total_field_changes: usize,
    pub has_breaking_changes: bool,
}

// ---------------------------------------------------------------------------
// DiffReport
// ---------------------------------------------------------------------------

/// The complete semantic diff between two AGM files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiffReport {
    pub header_changes: Vec<HeaderChange>,
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub modified_nodes: Vec<NodeDiff>,
    pub summary: DiffSummary,
}

impl DiffReport {
    /// Returns true if no semantic differences were found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.header_changes.is_empty()
            && self.added_nodes.is_empty()
            && self.removed_nodes.is_empty()
            && self.modified_nodes.is_empty()
    }

    /// Returns true if any breaking change was detected.
    #[must_use]
    pub fn has_breaking_changes(&self) -> bool {
        self.summary.has_breaking_changes
    }

    /// Returns a new report containing only breaking changes.
    #[must_use]
    pub fn breaking_only(&self) -> DiffReport {
        let header_changes: Vec<HeaderChange> = self
            .header_changes
            .iter()
            .filter(|c| c.severity == ChangeSeverity::Breaking)
            .cloned()
            .collect();

        // added_nodes are never breaking (additive)
        let added_nodes: Vec<String> = vec![];

        // removed_nodes are always breaking
        let removed_nodes = self.removed_nodes.clone();

        // Keep only modified nodes that have at least one breaking field change,
        // and within those, keep only the breaking field changes.
        let modified_nodes: Vec<NodeDiff> = self
            .modified_nodes
            .iter()
            .filter(|nd| nd.has_breaking_change)
            .map(|nd| {
                let breaking_changes: Vec<FieldChange> = nd
                    .field_changes
                    .iter()
                    .filter(|fc| fc.severity == ChangeSeverity::Breaking)
                    .cloned()
                    .collect();
                NodeDiff {
                    node_id: nd.node_id.clone(),
                    field_changes: breaking_changes,
                    has_breaking_change: true,
                }
            })
            .collect();

        let total_field_changes = modified_nodes
            .iter()
            .map(|nd| nd.field_changes.len())
            .sum::<usize>()
            + header_changes.len();

        let has_breaking = !removed_nodes.is_empty()
            || header_changes
                .iter()
                .any(|c| c.severity == ChangeSeverity::Breaking)
            || modified_nodes.iter().any(|nd| nd.has_breaking_change);

        let summary = DiffSummary {
            nodes_added: 0,
            nodes_removed: removed_nodes.len(),
            nodes_modified: modified_nodes.len(),
            nodes_unchanged: self.summary.nodes_unchanged,
            header_changes: header_changes.len(),
            total_field_changes,
            has_breaking_changes: has_breaking,
        };

        DiffReport {
            header_changes,
            added_nodes,
            removed_nodes,
            modified_nodes,
            summary,
        }
    }
}

// ---------------------------------------------------------------------------
// diff() -- public entry point
// ---------------------------------------------------------------------------

/// Computes the semantic diff between two AGM files.
///
/// Nodes are matched by ID. Nodes present only in `left` are reported as
/// removed; nodes present only in `right` are reported as added; nodes
/// present in both are compared field-by-field.
///
/// Header fields are compared independently of nodes.
#[must_use]
pub fn diff(left: &AgmFile, right: &AgmFile) -> DiffReport {
    let header_changes = header::diff_headers(&left.header, &right.header);
    let node_result = node::diff_nodes(left, right);

    let has_breaking = !node_result.removed.is_empty()
        || header_changes
            .iter()
            .any(|c| c.severity == ChangeSeverity::Breaking)
        || node_result.modified.iter().any(|nd| nd.has_breaking_change);

    let total_field_changes = node_result
        .modified
        .iter()
        .map(|nd| nd.field_changes.len())
        .sum::<usize>()
        + header_changes.len();

    let summary = DiffSummary {
        nodes_added: node_result.added.len(),
        nodes_removed: node_result.removed.len(),
        nodes_modified: node_result.modified.len(),
        nodes_unchanged: node_result.unchanged_count,
        header_changes: header_changes.len(),
        total_field_changes,
        has_breaking_changes: has_breaking,
    };

    DiffReport {
        header_changes,
        added_nodes: node_result.added,
        removed_nodes: node_result.removed,
        modified_nodes: node_result.modified,
        summary,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_report() -> DiffReport {
        DiffReport {
            header_changes: vec![],
            added_nodes: vec![],
            removed_nodes: vec![],
            modified_nodes: vec![],
            summary: DiffSummary {
                nodes_added: 0,
                nodes_removed: 0,
                nodes_modified: 0,
                nodes_unchanged: 0,
                header_changes: 0,
                total_field_changes: 0,
                has_breaking_changes: false,
            },
        }
    }

    #[test]
    fn test_diff_report_is_empty_when_no_changes_returns_true() {
        assert!(empty_report().is_empty());
    }

    #[test]
    fn test_diff_report_is_empty_when_has_changes_returns_false() {
        let mut r = empty_report();
        r.added_nodes.push("some.node".to_owned());
        assert!(!r.is_empty());
    }

    #[test]
    fn test_diff_report_has_breaking_changes_when_none_returns_false() {
        assert!(!empty_report().has_breaking_changes());
    }

    #[test]
    fn test_diff_report_has_breaking_changes_when_present_returns_true() {
        let mut r = empty_report();
        r.summary.has_breaking_changes = true;
        assert!(r.has_breaking_changes());
    }

    #[test]
    fn test_change_kind_display_roundtrip() {
        assert_eq!(ChangeKind::Added.to_string(), "added");
        assert_eq!(ChangeKind::Removed.to_string(), "removed");
        assert_eq!(ChangeKind::Modified.to_string(), "modified");
    }

    #[test]
    fn test_change_severity_ordering() {
        assert!(ChangeSeverity::Info < ChangeSeverity::Minor);
        assert!(ChangeSeverity::Minor < ChangeSeverity::Breaking);
        assert!(ChangeSeverity::Info < ChangeSeverity::Breaking);
    }
}
