//! Relation inference: detects cross-references and dependencies between nodes.

use regex::Regex;
use std::collections::HashMap;

use crate::model::node::Node;

// ---------------------------------------------------------------------------
// RelationInferrer
// ---------------------------------------------------------------------------

/// Infers relationships between compiled nodes by scanning content for
/// cross-references and dependency phrases.
pub(crate) struct RelationInferrer;

impl RelationInferrer {
    /// Scans all nodes and populates relationship fields where patterns
    /// are detected.
    ///
    /// Detection strategies:
    /// 1. **Explicit dependency phrases**: "depends on X", "requires X",
    ///    "after X" where X matches another node's ID or heading-derived ID.
    /// 2. **Heading cross-references**: If node A's content mentions the
    ///    heading text that was used to generate node B's ID, add B to A's
    ///    `related_to`.
    /// 3. **"See also" phrases**: "see X", "refer to X" -> `see_also`.
    pub fn infer(nodes: &mut [Node]) {
        if nodes.len() < 2 {
            return;
        }

        // Build lookup: node_id -> index, and summary words -> node_id
        let id_set: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.clone(), i))
            .collect();

        // Build heading-word lookup from node IDs
        // e.g., "login_flow" -> words ["login", "flow"]
        let id_words: Vec<(String, Vec<String>)> = nodes
            .iter()
            .map(|n| {
                let words: Vec<String> =
                    n.id.split('.')
                        .flat_map(|seg| seg.split('_'))
                        .map(|w| w.to_lowercase())
                        .filter(|w| w.len() > 2) // skip very short words
                        .collect();
                (n.id.clone(), words)
            })
            .collect();

        // Patterns for explicit dependency phrases
        let dep_re = Regex::new(r"(?i)\b(?:depends?\s+on|requires?|after)\s+(\S+)").unwrap();
        let see_re = Regex::new(r"(?i)\b(?:see\s+also|see|refer\s+to)\s+(\S+)").unwrap();

        // Collect inferred relations (avoid borrowing issues)
        let mut depends_map: HashMap<usize, Vec<String>> = HashMap::new();
        let mut related_map: HashMap<usize, Vec<String>> = HashMap::new();
        let mut see_also_map: HashMap<usize, Vec<String>> = HashMap::new();

        for (i, node) in nodes.iter().enumerate() {
            let content = gather_content(node);
            let content_lower = content.to_lowercase();

            // 1. Explicit dependency phrases
            for cap in dep_re.captures_iter(&content) {
                let ref_text = cap[1]
                    .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
                    .to_lowercase();
                // Check if ref_text matches any node ID
                for other_id in id_set.keys() {
                    if other_id != &node.id && fuzzy_match_id(other_id, &ref_text) {
                        depends_map.entry(i).or_default().push(other_id.clone());
                    }
                }
            }

            // 2. "See also" phrases
            for cap in see_re.captures_iter(&content) {
                let ref_text = cap[1]
                    .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
                    .to_lowercase();
                for other_id in id_set.keys() {
                    if other_id != &node.id && fuzzy_match_id(other_id, &ref_text) {
                        see_also_map.entry(i).or_default().push(other_id.clone());
                    }
                }
            }

            // 3. Heading cross-references (word overlap)
            for (other_id, words) in &id_words {
                if other_id == &node.id || words.is_empty() {
                    continue;
                }
                // If more than half the words from another node's ID appear
                // in this node's content, infer a related_to relationship
                let matches = words
                    .iter()
                    .filter(|w| content_lower.contains(w.as_str()))
                    .count();
                if words.len() >= 2 && matches > words.len() / 2 {
                    related_map.entry(i).or_default().push(other_id.clone());
                }
            }
        }

        // Apply inferred relations to nodes
        for (i, deps) in depends_map {
            let existing = nodes[i].depends.get_or_insert_with(Vec::new);
            for dep in deps {
                if !existing.contains(&dep) {
                    existing.push(dep);
                }
            }
        }

        for (i, rels) in related_map {
            let existing = nodes[i].related_to.get_or_insert_with(Vec::new);
            for rel in rels {
                if !existing.contains(&rel) {
                    existing.push(rel);
                }
            }
        }

        for (i, sees) in see_also_map {
            let existing = nodes[i].see_also.get_or_insert_with(Vec::new);
            for s in sees {
                if !existing.contains(&s) {
                    existing.push(s);
                }
            }
        }
    }
}

/// Gathers all textual content from a node for scanning.
fn gather_content(node: &Node) -> String {
    let mut content = String::new();

    content.push_str(&node.summary);
    content.push(' ');

    if let Some(ref detail) = node.detail {
        content.push_str(detail);
        content.push(' ');
    }

    if let Some(ref items) = node.items {
        for item in items {
            content.push_str(item);
            content.push(' ');
        }
    }

    if let Some(ref steps) = node.steps {
        for step in steps {
            content.push_str(step);
            content.push(' ');
        }
    }

    content
}

/// Fuzzy-matches a reference text against a node ID.
/// Returns true if the reference text contains the last segment of the ID
/// or matches the full ID.
fn fuzzy_match_id(node_id: &str, ref_text: &str) -> bool {
    let id_lower = node_id.to_lowercase();
    let ref_lower = ref_text.to_lowercase();

    // Exact match
    if ref_lower == id_lower {
        return true;
    }

    // Last segment match (e.g., "constraints" matches "auth.constraints")
    if let Some(last_seg) = id_lower.rsplit('.').next() {
        if ref_lower == last_seg || ref_lower.replace('_', " ") == last_seg.replace('_', " ") {
            return true;
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::{NodeType, Span};

    fn make_node(id: &str, summary: &str, detail: Option<&str>) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: summary.to_owned(),
            detail: detail.map(|s| s.to_owned()),
            span: Span::new(1, 5),
            ..Default::default()
        }
    }

    #[test]
    fn test_infer_explicit_depends_on() {
        let mut nodes = vec![
            make_node("auth.constraints", "Security constraints", None),
            make_node(
                "auth.login",
                "Login flow",
                Some("This depends on constraints to work."),
            ),
        ];
        RelationInferrer::infer(&mut nodes);
        assert!(
            nodes[1]
                .depends
                .as_ref()
                .is_some_and(|d| d.contains(&"auth.constraints".to_owned()))
        );
    }

    #[test]
    fn test_infer_no_self_reference() {
        let mut nodes = vec![make_node("auth.login", "Login depends on login", None)];
        RelationInferrer::infer(&mut nodes);
        // Single node: no inference possible
        assert!(nodes[0].depends.is_none());
    }

    #[test]
    fn test_infer_see_also() {
        let mut nodes = vec![
            make_node("auth.login", "Login flow", None),
            make_node("auth.session", "Session management. See also login.", None),
        ];
        RelationInferrer::infer(&mut nodes);
        // "See also login" should match auth.login via last-segment fuzzy match
        let see_also = nodes[1].see_also.as_ref();
        assert!(see_also.is_some_and(|s| s.contains(&"auth.login".to_owned())));
    }

    #[test]
    fn test_fuzzy_match_id_exact() {
        assert!(fuzzy_match_id("auth.login", "auth.login"));
    }

    #[test]
    fn test_fuzzy_match_id_last_segment() {
        assert!(fuzzy_match_id("auth.constraints", "constraints"));
    }

    #[test]
    fn test_fuzzy_match_id_no_match() {
        assert!(!fuzzy_match_id("auth.login", "billing"));
    }
}
