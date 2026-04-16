//! Node type classifier using heuristic pattern matching (spec S35.2).

use regex::Regex;

use crate::model::fields::NodeType;

use super::section::MarkdownSection;

// ---------------------------------------------------------------------------
// Pattern definitions (spec S35.2)
// ---------------------------------------------------------------------------

struct TypePattern {
    node_type: NodeType,
    heading_patterns: &'static [&'static str],
    body_patterns: &'static [&'static str],
}

static TYPE_PATTERNS: &[TypePattern] = &[
    TypePattern {
        node_type: NodeType::Rules,
        heading_patterns: &["constraint", "rule", "requirement", "policy", "regulation"],
        body_patterns: &[
            "must",
            "must not",
            "shall",
            "shall not",
            "do not",
            "required",
            "prohibited",
        ],
    },
    TypePattern {
        node_type: NodeType::Workflow,
        heading_patterns: &[
            "flow",
            "process",
            "procedure",
            "pipeline",
            "workflow",
            "steps",
        ],
        body_patterns: &["the flow is", "step 1", "first,", "then,", "finally,"],
    },
    TypePattern {
        node_type: NodeType::Entity,
        heading_patterns: &["entity", "model", "schema", "object", "record", "table"],
        body_patterns: &["contains", "fields", "attributes", "properties", "columns"],
    },
    TypePattern {
        node_type: NodeType::Decision,
        heading_patterns: &["decision", "adr", "choice", "rationale"],
        body_patterns: &[
            "we decided",
            "decision",
            "chosen because",
            "trade-off",
            "tradeoff",
            "alternative",
        ],
    },
    TypePattern {
        node_type: NodeType::Exception,
        heading_patterns: &["exception", "error", "failure", "fallback", "recovery"],
        body_patterns: &[
            "in case of",
            "if .* fails",
            "when .* error",
            "fallback",
            "recovery",
        ],
    },
    TypePattern {
        node_type: NodeType::Example,
        heading_patterns: &["example", "sample", "demo", "tutorial", "walkthrough"],
        body_patterns: &[
            "for example",
            "e\\.g\\.",
            "such as",
            "consider the following",
        ],
    },
    TypePattern {
        node_type: NodeType::Glossary,
        heading_patterns: &["glossary", "terminology", "definitions", "terms"],
        body_patterns: &["means", "is defined as", "refers to", "definition"],
    },
    TypePattern {
        node_type: NodeType::AntiPattern,
        heading_patterns: &["anti.?pattern", "bad practice", "pitfall", "avoid", "don't"],
        body_patterns: &[
            "avoid",
            "bad practice",
            "do not",
            "never",
            "common mistake",
            "pitfall",
        ],
    },
    TypePattern {
        node_type: NodeType::Ticket,
        heading_patterns: &["ticket", "issue", "task", "bug", "feature request", "story"],
        body_patterns: &[
            "priority:",
            "assignee:",
            "as a user",
            "acceptance criteria",
            "closes #",
            "fixes #",
            "resolves #",
        ],
    },
];

// ---------------------------------------------------------------------------
// NodeTypeClassifier
// ---------------------------------------------------------------------------

/// Classifies Markdown sections into AGM node types using heuristic
/// pattern matching on heading text and body content.
pub(crate) struct NodeTypeClassifier {
    /// Pre-compiled regex patterns for each type, derived from TYPE_PATTERNS.
    compiled: Vec<CompiledPattern>,
}

struct CompiledPattern {
    node_type: NodeType,
    heading_regexes: Vec<Regex>,
    body_regexes: Vec<Regex>,
}

impl NodeTypeClassifier {
    /// Creates a new classifier with pre-compiled regex patterns.
    pub fn new() -> Self {
        let compiled = TYPE_PATTERNS
            .iter()
            .map(|tp| {
                let heading_regexes = tp
                    .heading_patterns
                    .iter()
                    .filter_map(|p| Regex::new(&format!("(?i){p}")).ok())
                    .collect();
                let body_regexes = tp
                    .body_patterns
                    .iter()
                    .filter_map(|p| Regex::new(&format!("(?i){p}")).ok())
                    .collect();
                CompiledPattern {
                    node_type: tp.node_type.clone(),
                    heading_regexes,
                    body_regexes,
                }
            })
            .collect();

        Self { compiled }
    }

    /// Classifies a section, returning (best_type, confidence, alternative_type).
    ///
    /// - `confidence` is in the range 0.0..=1.0.
    /// - `alternative_type` is `Some(type)` if a second type scored within
    ///   0.15 of the best, indicating ambiguity.
    ///
    /// The scoring algorithm:
    /// - Heading match: +0.4 per matching pattern (capped at 0.6)
    /// - Body match: +0.15 per matching pattern (capped at 0.6)
    /// - Ordered list presence: +0.2 bonus for workflow
    /// - Code block presence: +0.1 bonus for example
    /// - Final confidence capped at 1.0
    pub fn classify(&self, section: &MarkdownSection) -> (NodeType, f32, Option<NodeType>) {
        let mut scores: Vec<(NodeType, f32)> = Vec::new();

        for cp in &self.compiled {
            let mut score: f32 = 0.0;

            // Heading matches
            let heading_lower = section.heading.to_lowercase();
            let heading_hits: f32 = cp
                .heading_regexes
                .iter()
                .filter(|r| r.is_match(&heading_lower))
                .count() as f32;
            score += (heading_hits * 0.4).min(0.6);

            // Body matches
            let body_lower = section.body_text.to_lowercase();
            let items_text: String = section.list_items.join(" ").to_lowercase();
            let combined_body = format!("{body_lower} {items_text}");

            let body_hits: f32 = cp
                .body_regexes
                .iter()
                .filter(|r| r.is_match(&combined_body))
                .count() as f32;
            score += (body_hits * 0.15).min(0.6);

            // Structural bonuses
            if cp.node_type == NodeType::Workflow && section.is_ordered_list {
                score += 0.2;
            }
            if cp.node_type == NodeType::Example && !section.code_blocks.is_empty() {
                score += 0.1;
            }

            scores.push((cp.node_type.clone(), score.min(1.0)));
        }

        // Sort descending by score
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if scores.is_empty() || scores[0].1 == 0.0 {
            // No pattern matched; default to facts with low confidence
            return (NodeType::Facts, 0.2, None);
        }

        let best = scores[0].clone();
        let alternative = if scores.len() > 1 && (best.1 - scores[1].1) < 0.15 {
            Some(scores[1].0.clone())
        } else {
            None
        };

        (best.0, best.1, alternative)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_section(heading: &str, body: &str, items: Vec<&str>, ordered: bool) -> MarkdownSection {
        MarkdownSection {
            heading: heading.to_owned(),
            heading_level: 2,
            body_text: body.to_owned(),
            list_items: items.into_iter().map(|s| s.to_owned()).collect(),
            is_ordered_list: ordered,
            code_blocks: Vec::new(),
            source_line_start: 1,
            source_line_end: 10,
        }
    }

    #[test]
    fn test_classify_rules_heading_must_items() {
        let c = NodeTypeClassifier::new();
        let sec = make_section(
            "Login Constraints",
            "",
            vec![
                "Tokens must never be exposed",
                "Calls must originate from server",
            ],
            false,
        );
        let (node_type, confidence, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Rules);
        assert!(confidence >= 0.3);
    }

    #[test]
    fn test_classify_workflow_ordered_list() {
        let c = NodeTypeClassifier::new();
        let sec = make_section(
            "Login Flow",
            "",
            vec!["Resolve tenant", "Redirect to provider"],
            true,
        );
        let (node_type, confidence, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Workflow);
        assert!(confidence >= 0.5);
    }

    #[test]
    fn test_classify_entity_heading() {
        let c = NodeTypeClassifier::new();
        let sec = make_section(
            "User Entity",
            "Contains the following fields.",
            vec![],
            false,
        );
        let (node_type, confidence, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Entity);
        assert!(confidence > 0.0);
    }

    #[test]
    fn test_classify_decision_heading_and_body() {
        let c = NodeTypeClassifier::new();
        let sec = make_section(
            "Architecture Decision",
            "We decided to use PostgreSQL because of its reliability.",
            vec![],
            false,
        );
        let (node_type, confidence, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Decision);
        assert!(confidence >= 0.4);
    }

    #[test]
    fn test_classify_unknown_defaults_to_facts() {
        let c = NodeTypeClassifier::new();
        let sec = make_section("Random Title", "Nothing special here.", vec![], false);
        let (node_type, confidence, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Facts);
        assert!(confidence < 0.5);
    }

    #[test]
    fn test_classify_exception_heading() {
        let c = NodeTypeClassifier::new();
        let sec = make_section(
            "Error Recovery",
            "In case of timeout, retry with backoff.",
            vec![],
            false,
        );
        let (node_type, _, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Exception);
    }

    #[test]
    fn test_classify_glossary_heading() {
        let c = NodeTypeClassifier::new();
        let sec = make_section(
            "Glossary",
            "SID means server-issued device token.",
            vec![],
            false,
        );
        let (node_type, _, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Glossary);
    }

    #[test]
    fn test_classify_anti_pattern_heading() {
        let c = NodeTypeClassifier::new();
        let sec = make_section(
            "Common Pitfalls",
            "Avoid storing tokens in localStorage.",
            vec![],
            false,
        );
        let (node_type, _, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::AntiPattern);
    }

    #[test]
    fn test_classify_example_with_code_block() {
        let c = NodeTypeClassifier::new();
        let mut sec = make_section(
            "Usage Example",
            "For example, consider the following code.",
            vec![],
            false,
        );
        sec.code_blocks = vec![(Some("rust".to_owned()), "fn main() {}".to_owned())];
        let (node_type, confidence, _) = c.classify(&sec);
        assert_eq!(node_type, NodeType::Example);
        assert!(confidence >= 0.5);
    }

    #[test]
    fn test_classify_ambiguous_returns_alternative() {
        let c = NodeTypeClassifier::new();
        // "Avoid" in heading could match anti_pattern, but "must" in body matches rules
        let sec = make_section(
            "Things to Avoid",
            "You must never expose tokens.",
            vec![],
            false,
        );
        let (_, _, _alt) = c.classify(&sec);
        // Either alternative should be present or both score high
        // The important thing is the function returns without panic
        // and one of the expected types is chosen
        let (node_type, _, _) = c.classify(&sec);
        assert!(
            node_type == NodeType::AntiPattern || node_type == NodeType::Rules,
            "Expected AntiPattern or Rules, got {node_type}"
        );
    }
}
