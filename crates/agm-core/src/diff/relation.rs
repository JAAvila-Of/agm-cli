//! Specialized element-level diff for list and relationship fields.

use std::collections::BTreeSet;

use super::ChangeKind;
use super::fields::{FieldChange, FieldValueSnapshot, classify_severity};

/// Performs element-level diffing of a relationship list field.
///
/// Instead of comparing the entire list as a single value, this function
/// reports individual elements that were added or removed. Reordering
/// is NOT reported as a change for unordered relationship fields
/// (related_to, replaces, conflicts, see_also).
///
/// For `depends`, order changes ARE reported as Minor (dependency
/// resolution order may matter for execution).
///
/// For `steps`, order changes ARE reported as Minor (steps are ordered).
///
/// For all other list fields (items, tags, etc.), order is ignored (set comparison).
#[must_use]
pub(crate) fn diff_relation_field(
    field_name: &str,
    old: &[String],
    new: &[String],
) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    if is_ordered_field(field_name) {
        diff_ordered(field_name, old, new, &mut changes);
    } else {
        diff_unordered(field_name, old, new, &mut changes);
    }

    changes
}

/// Diffs an unordered field using set semantics.
fn diff_unordered(
    field_name: &str,
    old: &[String],
    new: &[String],
    changes: &mut Vec<FieldChange>,
) {
    let old_set: BTreeSet<&str> = old.iter().map(String::as_str).collect();
    let new_set: BTreeSet<&str> = new.iter().map(String::as_str).collect();

    // Elements removed (in old, not in new)
    for elem in old_set.difference(&new_set) {
        let severity = classify_severity(field_name, ChangeKind::Removed);
        changes.push(FieldChange {
            field: field_name.to_owned(),
            kind: ChangeKind::Removed,
            severity,
            old_value: Some(FieldValueSnapshot::Scalar((*elem).to_owned())),
            new_value: None,
        });
    }

    // Elements added (in new, not in old)
    for elem in new_set.difference(&old_set) {
        let severity = classify_severity(field_name, ChangeKind::Added);
        changes.push(FieldChange {
            field: field_name.to_owned(),
            kind: ChangeKind::Added,
            severity,
            old_value: None,
            new_value: Some(FieldValueSnapshot::Scalar((*elem).to_owned())),
        });
    }
}

/// Diffs an ordered field.
///
/// Elements not in old are Added; elements not in new are Removed.
/// For `depends`, if the sets are equal but order differs, report as Minor.
fn diff_ordered(field_name: &str, old: &[String], new: &[String], changes: &mut Vec<FieldChange>) {
    let old_set: BTreeSet<&str> = old.iter().map(String::as_str).collect();
    let new_set: BTreeSet<&str> = new.iter().map(String::as_str).collect();

    // Removed elements
    for elem in old_set.difference(&new_set) {
        let severity = classify_severity(field_name, ChangeKind::Removed);
        changes.push(FieldChange {
            field: field_name.to_owned(),
            kind: ChangeKind::Removed,
            severity,
            old_value: Some(FieldValueSnapshot::Scalar((*elem).to_owned())),
            new_value: None,
        });
    }

    // Added elements
    for elem in new_set.difference(&old_set) {
        let severity = classify_severity(field_name, ChangeKind::Added);
        changes.push(FieldChange {
            field: field_name.to_owned(),
            kind: ChangeKind::Added,
            severity,
            old_value: None,
            new_value: Some(FieldValueSnapshot::Scalar((*elem).to_owned())),
        });
    }

    // Order change: sets equal but order differs
    if old_set == new_set && old != new {
        changes.push(FieldChange {
            field: field_name.to_owned(),
            kind: ChangeKind::Modified,
            severity: super::ChangeSeverity::Minor,
            old_value: Some(FieldValueSnapshot::List(old.to_vec())),
            new_value: Some(FieldValueSnapshot::List(new.to_vec())),
        });
    }
}

/// Returns true if the field should be compared as an ordered sequence
/// (order changes count as modifications) rather than as a set.
fn is_ordered_field(field_name: &str) -> bool {
    matches!(field_name, "steps" | "depends")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::ChangeSeverity;

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn test_diff_relation_depends_element_removed_returns_breaking() {
        let old = strs(&["a", "b"]);
        let new = strs(&["a"]);
        let changes = diff_relation_field("depends", &old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].severity, ChangeSeverity::Breaking);
    }

    #[test]
    fn test_diff_relation_depends_element_added_returns_minor() {
        let old = strs(&["a"]);
        let new = strs(&["a", "b"]);
        let changes = diff_relation_field("depends", &old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].severity, ChangeSeverity::Minor);
    }

    #[test]
    fn test_diff_relation_related_to_element_added_returns_info() {
        let old = strs(&["a"]);
        let new = strs(&["a", "b"]);
        let changes = diff_relation_field("related_to", &old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_relation_related_to_element_removed_returns_info() {
        let old = strs(&["a", "b"]);
        let new = strs(&["a"]);
        let changes = diff_relation_field("related_to", &old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_relation_see_also_reordered_returns_empty() {
        let old = strs(&["a", "b", "c"]);
        let new = strs(&["c", "a", "b"]);
        let changes = diff_relation_field("see_also", &old, &new);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_relation_steps_element_removed_returns_breaking() {
        let old = strs(&["step1", "step2", "step3"]);
        let new = strs(&["step1", "step3"]);
        let changes = diff_relation_field("steps", &old, &new);
        // Should have a removal for step2
        let removed: Vec<_> = changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Removed)
            .collect();
        assert!(!removed.is_empty());
        assert_eq!(removed[0].severity, ChangeSeverity::Breaking);
    }

    #[test]
    fn test_diff_relation_tags_reordered_returns_empty() {
        let old = strs(&["alpha", "beta"]);
        let new = strs(&["beta", "alpha"]);
        let changes = diff_relation_field("tags", &old, &new);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_relation_empty_to_empty_returns_empty() {
        let changes = diff_relation_field("items", &[], &[]);
        assert!(changes.is_empty());
    }

    #[test]
    fn test_diff_relation_empty_to_some_returns_added() {
        let old: Vec<String> = vec![];
        let new = strs(&["x", "y"]);
        let changes = diff_relation_field("items", &old, &new);
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().all(|c| c.kind == ChangeKind::Added));
    }

    #[test]
    fn test_diff_relation_some_to_empty_returns_removed() {
        let old = strs(&["x", "y"]);
        let new: Vec<String> = vec![];
        let changes = diff_relation_field("items", &old, &new);
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().all(|c| c.kind == ChangeKind::Removed));
    }
}
