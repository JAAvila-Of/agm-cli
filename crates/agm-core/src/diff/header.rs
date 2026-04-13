//! Header field-by-field comparison.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::model::file::Header;

use super::{ChangeKind, ChangeSeverity};

/// A change to a single header field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderChange {
    pub field: String,
    pub kind: ChangeKind,
    pub severity: ChangeSeverity,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

/// Compares two `Header` structs field-by-field.
///
/// Returns a list of changes. Unchanged fields are not included.
#[must_use]
pub fn diff_headers(left: &Header, right: &Header) -> Vec<HeaderChange> {
    let mut changes = Vec::new();

    // Required scalars: agm, package, version
    compare_scalar(
        &mut changes,
        "agm",
        &left.agm,
        &right.agm,
        header_field_severity,
    );
    compare_scalar(
        &mut changes,
        "package",
        &left.package,
        &right.package,
        header_field_severity,
    );
    compare_scalar(
        &mut changes,
        "version",
        &left.version,
        &right.version,
        header_field_severity,
    );

    // Optional scalars
    compare_opt_scalar(
        &mut changes,
        "title",
        left.title.as_deref(),
        right.title.as_deref(),
        header_field_severity,
    );
    compare_opt_scalar(
        &mut changes,
        "owner",
        left.owner.as_deref(),
        right.owner.as_deref(),
        header_field_severity,
    );
    compare_opt_scalar(
        &mut changes,
        "description",
        left.description.as_deref(),
        right.description.as_deref(),
        header_field_severity,
    );
    compare_opt_scalar(
        &mut changes,
        "status",
        left.status.as_deref(),
        right.status.as_deref(),
        header_field_severity,
    );
    compare_opt_scalar(
        &mut changes,
        "default_load",
        left.default_load.as_deref(),
        right.default_load.as_deref(),
        header_field_severity,
    );
    compare_opt_scalar(
        &mut changes,
        "target_runtime",
        left.target_runtime.as_deref(),
        right.target_runtime.as_deref(),
        header_field_severity,
    );

    // Optional list: tags (set comparison)
    compare_opt_tag_list(
        &mut changes,
        "tags",
        left.tags.as_deref(),
        right.tags.as_deref(),
    );

    // Optional imports: element-level diff by package name
    compare_imports(
        &mut changes,
        left.imports.as_deref(),
        right.imports.as_deref(),
    );

    // Optional load_profiles: element-level diff by key
    compare_load_profiles(
        &mut changes,
        left.load_profiles.as_ref(),
        right.load_profiles.as_ref(),
    );

    changes
}

/// Classifies severity for a header field change.
fn header_field_severity(field: &str, kind: ChangeKind) -> ChangeSeverity {
    match (field, kind) {
        ("agm", _) => ChangeSeverity::Breaking,
        ("package", _) => ChangeSeverity::Breaking,
        ("version", _) => ChangeSeverity::Info,
        ("title", _) => ChangeSeverity::Info,
        ("owner", _) => ChangeSeverity::Info,
        ("description", _) => ChangeSeverity::Info,
        ("tags", _) => ChangeSeverity::Info,
        ("status", ChangeKind::Added) => ChangeSeverity::Info,
        ("status", _) => ChangeSeverity::Minor,
        ("default_load", ChangeKind::Added) => ChangeSeverity::Info,
        ("default_load", _) => ChangeSeverity::Minor,
        ("imports", ChangeKind::Added) => ChangeSeverity::Minor,
        ("imports", ChangeKind::Removed) => ChangeSeverity::Breaking,
        ("imports", ChangeKind::Modified) => ChangeSeverity::Minor,
        ("load_profiles", ChangeKind::Added) => ChangeSeverity::Info,
        ("load_profiles", _) => ChangeSeverity::Minor,
        ("target_runtime", ChangeKind::Added) => ChangeSeverity::Info,
        ("target_runtime", _) => ChangeSeverity::Minor,
        _ => ChangeSeverity::Info,
    }
}

fn compare_scalar(
    changes: &mut Vec<HeaderChange>,
    field: &str,
    left: &str,
    right: &str,
    severity_fn: fn(&str, ChangeKind) -> ChangeSeverity,
) {
    if left != right {
        changes.push(HeaderChange {
            field: field.to_owned(),
            kind: ChangeKind::Modified,
            severity: severity_fn(field, ChangeKind::Modified),
            old_value: Some(left.to_owned()),
            new_value: Some(right.to_owned()),
        });
    }
}

fn compare_opt_scalar(
    changes: &mut Vec<HeaderChange>,
    field: &str,
    left: Option<&str>,
    right: Option<&str>,
    severity_fn: fn(&str, ChangeKind) -> ChangeSeverity,
) {
    match (left, right) {
        (None, None) => {}
        (None, Some(r)) => changes.push(HeaderChange {
            field: field.to_owned(),
            kind: ChangeKind::Added,
            severity: severity_fn(field, ChangeKind::Added),
            old_value: None,
            new_value: Some(r.to_owned()),
        }),
        (Some(l), None) => changes.push(HeaderChange {
            field: field.to_owned(),
            kind: ChangeKind::Removed,
            severity: severity_fn(field, ChangeKind::Removed),
            old_value: Some(l.to_owned()),
            new_value: None,
        }),
        (Some(l), Some(r)) => {
            if l != r {
                changes.push(HeaderChange {
                    field: field.to_owned(),
                    kind: ChangeKind::Modified,
                    severity: severity_fn(field, ChangeKind::Modified),
                    old_value: Some(l.to_owned()),
                    new_value: Some(r.to_owned()),
                });
            }
        }
    }
}

fn compare_opt_tag_list(
    changes: &mut Vec<HeaderChange>,
    field: &str,
    left: Option<&[String]>,
    right: Option<&[String]>,
) {
    let left_set: BTreeSet<&str> = left
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();
    let right_set: BTreeSet<&str> = right
        .unwrap_or_default()
        .iter()
        .map(String::as_str)
        .collect();

    if left_set != right_set {
        let old_val = if left.is_some() {
            Some(format!("[{}]", left.unwrap_or_default().join(", ")))
        } else {
            None
        };
        let new_val = if right.is_some() {
            Some(format!("[{}]", right.unwrap_or_default().join(", ")))
        } else {
            None
        };

        let kind = match (
            left.is_none() || left_set.is_empty(),
            right.is_none() || right_set.is_empty(),
        ) {
            (true, false) => ChangeKind::Added,
            (false, true) => ChangeKind::Removed,
            _ => ChangeKind::Modified,
        };

        changes.push(HeaderChange {
            field: field.to_owned(),
            kind,
            severity: ChangeSeverity::Info,
            old_value: old_val,
            new_value: new_val,
        });
    }
}

fn compare_imports(
    changes: &mut Vec<HeaderChange>,
    left: Option<&[crate::model::imports::ImportEntry]>,
    right: Option<&[crate::model::imports::ImportEntry]>,
) {
    let left_map: std::collections::BTreeMap<&str, &crate::model::imports::ImportEntry> = left
        .unwrap_or_default()
        .iter()
        .map(|e| (e.package.as_str(), e))
        .collect();
    let right_map: std::collections::BTreeMap<&str, &crate::model::imports::ImportEntry> = right
        .unwrap_or_default()
        .iter()
        .map(|e| (e.package.as_str(), e))
        .collect();

    // Detect removed (in left, not in right) -> Breaking
    for (pkg, entry) in &left_map {
        if !right_map.contains_key(pkg) {
            changes.push(HeaderChange {
                field: "imports".to_owned(),
                kind: ChangeKind::Removed,
                severity: ChangeSeverity::Breaking,
                old_value: Some(entry.to_string()),
                new_value: None,
            });
        }
    }

    // Detect added (in right, not in left) -> Minor
    for (pkg, entry) in &right_map {
        if !left_map.contains_key(pkg) {
            changes.push(HeaderChange {
                field: "imports".to_owned(),
                kind: ChangeKind::Added,
                severity: ChangeSeverity::Minor,
                old_value: None,
                new_value: Some(entry.to_string()),
            });
        }
    }

    // Detect modified (constraint changed)
    for (pkg, left_entry) in &left_map {
        if let Some(right_entry) = right_map.get(pkg) {
            if left_entry.version_constraint != right_entry.version_constraint {
                changes.push(HeaderChange {
                    field: "imports".to_owned(),
                    kind: ChangeKind::Modified,
                    severity: ChangeSeverity::Minor,
                    old_value: Some(left_entry.to_string()),
                    new_value: Some(right_entry.to_string()),
                });
            }
        }
    }
}

fn compare_load_profiles(
    changes: &mut Vec<HeaderChange>,
    left: Option<&std::collections::BTreeMap<String, crate::model::file::LoadProfile>>,
    right: Option<&std::collections::BTreeMap<String, crate::model::file::LoadProfile>>,
) {
    let empty = std::collections::BTreeMap::new();
    let left_map = left.unwrap_or(&empty);
    let right_map = right.unwrap_or(&empty);

    if left_map == right_map {
        return;
    }

    // Keys only in left -> Removed -> Minor
    for key in left_map.keys() {
        if !right_map.contains_key(key) {
            changes.push(HeaderChange {
                field: "load_profiles".to_owned(),
                kind: ChangeKind::Removed,
                severity: ChangeSeverity::Minor,
                old_value: Some(key.clone()),
                new_value: None,
            });
        }
    }

    // Keys only in right -> Added -> Info
    for key in right_map.keys() {
        if !left_map.contains_key(key) {
            changes.push(HeaderChange {
                field: "load_profiles".to_owned(),
                kind: ChangeKind::Added,
                severity: ChangeSeverity::Info,
                old_value: None,
                new_value: Some(key.clone()),
            });
        }
    }

    // Keys in both but different values -> Modified -> Minor
    for (key, left_lp) in left_map {
        if let Some(right_lp) = right_map.get(key) {
            if left_lp != right_lp {
                changes.push(HeaderChange {
                    field: "load_profiles".to_owned(),
                    kind: ChangeKind::Modified,
                    severity: ChangeSeverity::Minor,
                    old_value: Some(key.clone()),
                    new_value: Some(key.clone()),
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::model::file::{Header, LoadProfile};
    use crate::model::imports::ImportEntry;

    use super::*;

    fn base_header() -> Header {
        Header {
            agm: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            title: None,
            owner: None,
            imports: None,
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        }
    }

    #[test]
    fn test_diff_headers_identical_returns_empty() {
        let h = base_header();
        assert!(diff_headers(&h, &h).is_empty());
    }

    #[test]
    fn test_diff_headers_version_changed_returns_info() {
        let left = base_header();
        let mut right = left.clone();
        right.version = "0.2.0".to_owned();
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "version");
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
        assert_eq!(changes[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn test_diff_headers_package_changed_returns_breaking() {
        let left = base_header();
        let mut right = left.clone();
        right.package = "other.pkg".to_owned();
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "package");
        assert_eq!(changes[0].severity, ChangeSeverity::Breaking);
    }

    #[test]
    fn test_diff_headers_agm_changed_returns_breaking() {
        let left = base_header();
        let mut right = left.clone();
        right.agm = "2.0".to_owned();
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "agm");
        assert_eq!(changes[0].severity, ChangeSeverity::Breaking);
    }

    #[test]
    fn test_diff_headers_title_added_returns_info() {
        let left = base_header();
        let mut right = left.clone();
        right.title = Some("My Title".to_owned());
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "title");
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_headers_title_removed_returns_info() {
        let mut left = base_header();
        left.title = Some("My Title".to_owned());
        let right = base_header();
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "title");
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_headers_status_changed_returns_minor() {
        let mut left = base_header();
        left.status = Some("draft".to_owned());
        let mut right = left.clone();
        right.status = Some("stable".to_owned());
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "status");
        assert_eq!(changes[0].severity, ChangeSeverity::Minor);
    }

    #[test]
    fn test_diff_headers_imports_added_returns_minor() {
        let left = base_header();
        let mut right = left.clone();
        right.imports = Some(vec![ImportEntry::new("shared.auth".to_owned(), None)]);
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "imports");
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].severity, ChangeSeverity::Minor);
    }

    #[test]
    fn test_diff_headers_imports_removed_returns_breaking() {
        let mut left = base_header();
        left.imports = Some(vec![ImportEntry::new("shared.auth".to_owned(), None)]);
        let right = base_header();
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "imports");
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].severity, ChangeSeverity::Breaking);
    }

    #[test]
    fn test_diff_headers_tags_changed_returns_info() {
        let mut left = base_header();
        left.tags = Some(vec!["auth".to_owned()]);
        let mut right = left.clone();
        right.tags = Some(vec!["auth".to_owned(), "security".to_owned()]);
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "tags");
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_headers_load_profiles_added_returns_info() {
        let left = base_header();
        let mut right = left.clone();
        let mut lp = BTreeMap::new();
        lp.insert(
            "minimal".to_owned(),
            LoadProfile {
                filter: "priority in [critical]".to_owned(),
                estimated_tokens: None,
            },
        );
        right.load_profiles = Some(lp);
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "load_profiles");
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].severity, ChangeSeverity::Info);
    }

    #[test]
    fn test_diff_headers_load_profiles_removed_returns_minor() {
        let mut left = base_header();
        let mut lp = BTreeMap::new();
        lp.insert(
            "minimal".to_owned(),
            LoadProfile {
                filter: "priority in [critical]".to_owned(),
                estimated_tokens: None,
            },
        );
        left.load_profiles = Some(lp);
        let right = base_header();
        let changes = diff_headers(&left, &right);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].field, "load_profiles");
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].severity, ChangeSeverity::Minor);
    }
}
