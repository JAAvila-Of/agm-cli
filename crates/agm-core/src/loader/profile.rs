//! Load profile resolution and filter expression evaluation.
//!
//! Implements the filter mini-language used in `load_profiles` entries and
//! the built-in `debug` profile.

use std::collections::HashSet;

use crate::graph::build::build_graph;
use crate::graph::query::transitive_deps;
use crate::model::execution::ExecutionStatus;
use crate::model::file::AgmFile;
use crate::model::node::Node;

use super::filter::filter_node;
use super::mode::LoadMode;

// ---------------------------------------------------------------------------
// LoadError
// ---------------------------------------------------------------------------

/// Errors that can occur during profile-based loading.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LoadError {
    #[error("unknown load profile: {name:?}")]
    UnknownProfile { name: String },
    #[error("no load profiles defined in file header")]
    NoProfilesDefined,
    #[error("default_load profile {name:?} not found in load_profiles")]
    DefaultProfileNotFound { name: String },
}

// ---------------------------------------------------------------------------
// FilterExpr
// ---------------------------------------------------------------------------

/// A single filter predicate in a profile filter expression.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FilterExpr {
    Wildcard,
    PriorityIn(Vec<String>),
    TypeIn(Vec<String>),
    ExecutionStatusIn(Vec<String>),
    TagsIn(Vec<String>),
    CodeIsPresent,
    Unrecognized(String),
}

// ---------------------------------------------------------------------------
// FilterSet
// ---------------------------------------------------------------------------

/// A parsed set of filter expressions along with any parse-time warnings.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FilterSet {
    pub exprs: Vec<FilterExpr>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// parse_filter
// ---------------------------------------------------------------------------

/// Parses a filter string into a [`FilterSet`].
///
/// Clauses are separated by " AND " (case-insensitive). Unrecognized clauses
/// are collected as warnings rather than errors.
pub(crate) fn parse_filter(filter: &str) -> FilterSet {
    let clauses = split_on_and(filter);
    let mut exprs = Vec::new();
    let mut warnings = Vec::new();

    for clause in clauses {
        let clause = clause.trim();
        if clause.eq_ignore_ascii_case("*") || clause.eq_ignore_ascii_case("wildcard") {
            exprs.push(FilterExpr::Wildcard);
        } else if clause.eq_ignore_ascii_case("code is present")
            || clause.eq_ignore_ascii_case("code_is_present")
        {
            exprs.push(FilterExpr::CodeIsPresent);
        } else if let Some(expr) = try_parse_field_in(clause) {
            exprs.push(expr);
        } else {
            warnings.push(format!("unrecognized filter clause: {clause:?}"));
            exprs.push(FilterExpr::Unrecognized(clause.to_owned()));
        }
    }

    FilterSet { exprs, warnings }
}

// ---------------------------------------------------------------------------
// split_on_and
// ---------------------------------------------------------------------------

/// Splits `s` on " AND " (case-insensitive) and returns the parts.
pub(crate) fn split_on_and(s: &str) -> Vec<&str> {
    // We cannot use str::split with a pattern for case-insensitive matching,
    // so we scan manually.
    let sep = " and ";
    let lower = s.to_lowercase();
    let mut parts = Vec::new();
    let mut start = 0usize;

    let bytes = lower.as_bytes();
    let sep_bytes = sep.as_bytes();
    let sep_len = sep_bytes.len();

    let mut i = 0usize;
    while i + sep_len <= bytes.len() {
        if bytes[i..i + sep_len].eq_ignore_ascii_case(sep_bytes) {
            parts.push(&s[start..i]);
            start = i + sep_len;
            i = start;
        } else {
            i += 1;
        }
    }
    parts.push(&s[start..]);
    parts
}

// ---------------------------------------------------------------------------
// try_parse_field_in
// ---------------------------------------------------------------------------

/// Attempts to parse a `field in [val1, val2, ...]` clause.
///
/// Returns `Some(FilterExpr)` for known fields (`priority`, `type`,
/// `execution_status`, `tags`). Returns `None` for unrecognized patterns.
pub(crate) fn try_parse_field_in(clause: &str) -> Option<FilterExpr> {
    // Expected format: `<field> in [<val1>, <val2>, ...]` (case-insensitive keyword)
    let lower = clause.to_lowercase();
    let in_pos = lower.find(" in [")?;
    let field = clause[..in_pos].trim().to_lowercase();

    let after_in = clause[in_pos + 5..].trim(); // skip " in ["
    let closing = after_in.rfind(']')?;
    let values_str = &after_in[..closing];

    let values: Vec<String> = values_str
        .split(',')
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .collect();

    match field.as_str() {
        "priority" => Some(FilterExpr::PriorityIn(values)),
        "type" => Some(FilterExpr::TypeIn(values)),
        "execution_status" => Some(FilterExpr::ExecutionStatusIn(values)),
        "tags" => Some(FilterExpr::TagsIn(values)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// matches_filter
// ---------------------------------------------------------------------------

/// Returns `true` if `node` matches all expressions in `filter_set`.
///
/// - [`FilterExpr::Wildcard`] always matches.
/// - [`FilterExpr::Unrecognized`] passes through (evaluates to `true`).
/// - [`FilterExpr::TagsIn`] matches if ANY listed tag is in `node.tags`.
/// - All other expressions must match for the node to be included.
pub(crate) fn matches_filter(node: &Node, filter_set: &FilterSet) -> bool {
    filter_set.exprs.iter().all(|expr| match expr {
        FilterExpr::Wildcard => true,
        FilterExpr::Unrecognized(_) => true,
        FilterExpr::CodeIsPresent => node.code.is_some() || node.code_blocks.is_some(),
        FilterExpr::PriorityIn(values) => {
            let Some(p) = &node.priority else {
                return false;
            };
            values.contains(&p.to_string().to_lowercase())
        }
        FilterExpr::TypeIn(values) => values.contains(&node.node_type.to_string().to_lowercase()),
        FilterExpr::ExecutionStatusIn(values) => {
            let Some(s) = &node.execution_status else {
                return false;
            };
            values.contains(&s.to_string().to_lowercase())
        }
        FilterExpr::TagsIn(values) => {
            let Some(tags) = &node.tags else { return false };
            values.iter().any(|v| tags.contains(v))
        }
    })
}

// ---------------------------------------------------------------------------
// resolve_and_apply  (Sub-task 4)
// ---------------------------------------------------------------------------

/// Resolves the named profile (or the file's `default_load`) and applies it.
///
/// Steps:
/// 1. If `profile_name` is `None`, use `header.default_load`. If there is no
///    `default_load`, return a clone of the full file unchanged.
/// 2. The built-in `"debug"` profile is handled by [`apply_debug_profile`].
/// 3. Otherwise, look up the profile in `header.load_profiles` and apply
///    its filter expression, keeping matched nodes in Operational mode.
pub fn resolve_and_apply(file: &AgmFile, profile_name: Option<&str>) -> Result<AgmFile, LoadError> {
    // Step 1: determine effective profile name.
    let effective_name: String = match profile_name {
        Some(name) => name.to_owned(),
        None => match &file.header.default_load {
            Some(dl) => dl.clone(),
            // No explicit request and no default → return file unchanged (Full).
            None => return Ok(file.clone()),
        },
    };

    // Step 2: built-in debug profile.
    if effective_name.eq_ignore_ascii_case("debug") {
        return Ok(apply_debug_profile(file));
    }

    // Step 3: look up in load_profiles.
    let profiles = file
        .header
        .load_profiles
        .as_ref()
        .ok_or(LoadError::NoProfilesDefined)?;

    let profile = profiles.get(&effective_name).ok_or_else(|| {
        // Distinguish between a bad default_load vs an explicitly unknown name.
        if profile_name.is_none() {
            // The effective name came from default_load.
            LoadError::DefaultProfileNotFound {
                name: effective_name.clone(),
            }
        } else {
            LoadError::UnknownProfile {
                name: effective_name.clone(),
            }
        }
    })?;

    // Parse and evaluate the filter.
    let filter_set = parse_filter(&profile.filter);
    if !filter_set.warnings.is_empty() {
        for w in &filter_set.warnings {
            eprintln!("agm loader warning: {w}");
        }
    }

    let nodes: Vec<Node> = file
        .nodes
        .iter()
        .filter(|n| matches_filter(n, &filter_set))
        .map(|n| filter_node(n, LoadMode::Operational))
        .collect();

    Ok(AgmFile {
        header: file.header.clone(),
        nodes,
    })
}

// ---------------------------------------------------------------------------
// apply_debug_profile  (Sub-task 4)
// ---------------------------------------------------------------------------

/// Applies the built-in `debug` profile.
///
/// Selects nodes with `execution_status` of `Failed` or `Blocked`, then
/// adds all of their transitive dependencies. All selected nodes are returned
/// in Executable mode.
#[must_use]
pub fn apply_debug_profile(file: &AgmFile) -> AgmFile {
    let graph = build_graph(file);

    // Collect failed/blocked node IDs.
    let primary: Vec<String> = file
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                &n.execution_status,
                Some(ExecutionStatus::Failed) | Some(ExecutionStatus::Blocked)
            )
        })
        .map(|n| n.id.clone())
        .collect();

    // Collect transitive deps of each primary node.
    let mut selected: HashSet<String> = primary.iter().cloned().collect();
    for id in &primary {
        let deps = transitive_deps(&graph, id);
        selected.extend(deps);
    }

    let nodes: Vec<Node> = file
        .nodes
        .iter()
        .filter(|n| selected.contains(&n.id))
        .map(|n| filter_node(n, LoadMode::Executable))
        .collect();

    AgmFile {
        header: file.header.clone(),
        nodes,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::model::code::{CodeAction, CodeBlock};
    use crate::model::execution::ExecutionStatus;
    use crate::model::fields::{NodeType, Priority, Span};
    use crate::model::file::{AgmFile, Header, LoadProfile};
    use crate::model::node::Node;

    use super::*;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn minimal_header() -> Header {
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

    fn make_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: format!("node {id}"),
            span: Span::new(1, 1),
            ..Default::default()
        }
    }

    fn make_file(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: minimal_header(),
            nodes,
        }
    }

    // -----------------------------------------------------------------------
    // parse_filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_filter_wildcard_returns_wildcard_expr() {
        let fs = parse_filter("*");
        assert_eq!(fs.exprs, vec![FilterExpr::Wildcard]);
        assert!(fs.warnings.is_empty());
    }

    #[test]
    fn test_parse_filter_priority_in_returns_priority_expr() {
        let fs = parse_filter("priority in [critical, high]");
        assert_eq!(
            fs.exprs,
            vec![FilterExpr::PriorityIn(vec![
                "critical".to_owned(),
                "high".to_owned()
            ])]
        );
        assert!(fs.warnings.is_empty());
    }

    #[test]
    fn test_parse_filter_type_in_returns_type_expr() {
        let fs = parse_filter("type in [workflow, rules]");
        assert_eq!(
            fs.exprs,
            vec![FilterExpr::TypeIn(vec![
                "workflow".to_owned(),
                "rules".to_owned()
            ])]
        );
    }

    #[test]
    fn test_parse_filter_execution_status_in_returns_status_expr() {
        let fs = parse_filter("execution_status in [failed, blocked]");
        assert_eq!(
            fs.exprs,
            vec![FilterExpr::ExecutionStatusIn(vec![
                "failed".to_owned(),
                "blocked".to_owned()
            ])]
        );
    }

    #[test]
    fn test_parse_filter_and_conjunction_parses_multiple_exprs() {
        let fs = parse_filter("priority in [critical] AND type in [workflow]");
        assert_eq!(fs.exprs.len(), 2);
        assert!(matches!(&fs.exprs[0], FilterExpr::PriorityIn(_)));
        assert!(matches!(&fs.exprs[1], FilterExpr::TypeIn(_)));
    }

    #[test]
    fn test_parse_filter_and_is_case_insensitive() {
        let fs = parse_filter("priority in [critical] and type in [workflow]");
        assert_eq!(fs.exprs.len(), 2);
    }

    #[test]
    fn test_parse_filter_unrecognized_clause_produces_warning() {
        let fs = parse_filter("is_experimental");
        assert_eq!(fs.exprs.len(), 1);
        assert!(matches!(&fs.exprs[0], FilterExpr::Unrecognized(_)));
        assert!(!fs.warnings.is_empty());
    }

    // -----------------------------------------------------------------------
    // matches_filter tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_matches_filter_wildcard_always_matches() {
        let node = make_node("n");
        let fs = parse_filter("*");
        assert!(matches_filter(&node, &fs));
    }

    #[test]
    fn test_matches_filter_priority_in_matches_node_with_matching_priority() {
        let mut node = make_node("n");
        node.priority = Some(Priority::Critical);
        let fs = parse_filter("priority in [critical]");
        assert!(matches_filter(&node, &fs));
    }

    #[test]
    fn test_matches_filter_priority_in_rejects_node_with_wrong_priority() {
        let mut node = make_node("n");
        node.priority = Some(Priority::Low);
        let fs = parse_filter("priority in [critical]");
        assert!(!matches_filter(&node, &fs));
    }

    #[test]
    fn test_matches_filter_type_in_matches_correct_type() {
        let mut node = make_node("n");
        node.node_type = NodeType::Workflow;
        let fs = parse_filter("type in [workflow]");
        assert!(matches_filter(&node, &fs));
    }

    #[test]
    fn test_matches_filter_code_is_present_matches_node_with_code() {
        let mut node = make_node("n");
        node.code = Some(CodeBlock {
            lang: None,
            target: None,
            action: CodeAction::Full,
            body: "echo hi".to_owned(),
            anchor: None,
            old: None,
        });
        let fs = parse_filter("code is present");
        assert!(matches_filter(&node, &fs));
    }

    #[test]
    fn test_matches_filter_tags_in_matches_if_any_tag_present() {
        let mut node = make_node("n");
        node.tags = Some(vec!["auth".to_owned(), "api".to_owned()]);
        let fs = parse_filter("tags in [auth]");
        assert!(matches_filter(&node, &fs));
    }

    #[test]
    fn test_matches_filter_conjunction_requires_all_exprs() {
        let mut node = make_node("n");
        node.priority = Some(Priority::Critical);
        node.node_type = NodeType::Rules; // doesn't match "workflow"
        let fs = parse_filter("priority in [critical] AND type in [workflow]");
        assert!(!matches_filter(&node, &fs));
    }

    #[test]
    fn test_matches_filter_unrecognized_passes_through() {
        let node = make_node("n");
        let fs = FilterSet {
            exprs: vec![FilterExpr::Unrecognized("whatever".to_owned())],
            warnings: vec![],
        };
        assert!(matches_filter(&node, &fs));
    }

    // -----------------------------------------------------------------------
    // resolve_and_apply tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_and_apply_unknown_profile_returns_error() {
        let file = make_file(vec![]);
        let result = resolve_and_apply(&file, Some("nonexistent"));
        assert!(matches!(result, Err(LoadError::NoProfilesDefined)));
    }

    #[test]
    fn test_resolve_and_apply_no_profiles_defined_returns_error() {
        let mut file = make_file(vec![]);
        // Has a default_load but no load_profiles map.
        file.header.default_load = Some("ops".to_owned());
        let result = resolve_and_apply(&file, None);
        assert!(matches!(result, Err(LoadError::NoProfilesDefined)));
    }

    #[test]
    fn test_resolve_and_apply_default_load_not_found_returns_error() {
        let mut file = make_file(vec![]);
        file.header.default_load = Some("missing_profile".to_owned());
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "other".to_owned(),
            LoadProfile {
                filter: "*".to_owned(),
                estimated_tokens: None,
            },
        );
        file.header.load_profiles = Some(profiles);
        let result = resolve_and_apply(&file, None);
        assert!(matches!(
            result,
            Err(LoadError::DefaultProfileNotFound { .. })
        ));
    }

    #[test]
    fn test_resolve_and_apply_no_default_returns_full_file() {
        let node = make_node("a");
        let file = make_file(vec![node]);
        let result = resolve_and_apply(&file, None).unwrap();
        assert_eq!(result.nodes.len(), 1);
    }

    #[test]
    fn test_resolve_and_apply_filter_by_priority_keeps_matching_nodes() {
        let mut crit = make_node("crit");
        crit.priority = Some(Priority::Critical);
        let low = make_node("low");
        // low.priority stays None

        let mut profiles = BTreeMap::new();
        profiles.insert(
            "critical_only".to_owned(),
            LoadProfile {
                filter: "priority in [critical]".to_owned(),
                estimated_tokens: None,
            },
        );
        let mut file = make_file(vec![crit, low]);
        file.header.load_profiles = Some(profiles);

        let result = resolve_and_apply(&file, Some("critical_only")).unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].id, "crit");
    }

    #[test]
    fn test_resolve_and_apply_filter_by_type_keeps_matching_nodes() {
        let mut wf = make_node("wf");
        wf.node_type = NodeType::Workflow;
        let facts = make_node("facts");

        let mut profiles = BTreeMap::new();
        profiles.insert(
            "workflows".to_owned(),
            LoadProfile {
                filter: "type in [workflow]".to_owned(),
                estimated_tokens: None,
            },
        );
        let mut file = make_file(vec![wf, facts]);
        file.header.load_profiles = Some(profiles);

        let result = resolve_and_apply(&file, Some("workflows")).unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].id, "wf");
    }

    #[test]
    fn test_resolve_and_apply_debug_selects_failed_and_blocked() {
        let mut failed = make_node("failed_node");
        failed.execution_status = Some(ExecutionStatus::Failed);
        let mut blocked = make_node("blocked_node");
        blocked.execution_status = Some(ExecutionStatus::Blocked);
        let ok = make_node("ok_node");

        let file = make_file(vec![failed, blocked, ok]);
        let result = resolve_and_apply(&file, Some("debug")).unwrap();
        let ids: Vec<&str> = result.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"failed_node"));
        assert!(ids.contains(&"blocked_node"));
        assert!(!ids.contains(&"ok_node"));
    }

    #[test]
    fn test_resolve_and_apply_debug_includes_transitive_deps() {
        let mut failed = make_node("task.failed");
        failed.execution_status = Some(ExecutionStatus::Failed);
        failed.depends = Some(vec!["task.dep".to_owned()]);
        let dep = make_node("task.dep");
        let unrelated = make_node("task.unrelated");

        let file = make_file(vec![failed, dep, unrelated]);
        let result = resolve_and_apply(&file, Some("debug")).unwrap();
        let ids: Vec<&str> = result.nodes.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains(&"task.failed"));
        assert!(ids.contains(&"task.dep"));
        assert!(!ids.contains(&"task.unrelated"));
    }

    #[test]
    fn test_resolve_and_apply_debug_excludes_unrelated_nodes() {
        let mut failed = make_node("a");
        failed.execution_status = Some(ExecutionStatus::Failed);
        let unrelated = make_node("b"); // no execution status, not a dep

        let file = make_file(vec![failed, unrelated]);
        let result = resolve_and_apply(&file, Some("debug")).unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].id, "a");
    }

    #[test]
    fn test_resolve_and_apply_debug_uses_executable_mode() {
        let mut failed = make_node("task.a");
        failed.execution_status = Some(ExecutionStatus::Failed);
        failed.execution_log = Some("some log".to_owned());
        // `detail` is Full-only and should be absent.
        failed.detail = Some("detail text".to_owned());

        let file = make_file(vec![failed]);
        let result = resolve_and_apply(&file, Some("debug")).unwrap();
        assert_eq!(result.nodes.len(), 1);
        // Executable field included
        assert!(result.nodes[0].execution_log.is_some());
        // Full-only field excluded
        assert!(result.nodes[0].detail.is_none());
    }
}
