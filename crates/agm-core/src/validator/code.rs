//! Code block validation (spec S23, S23.6).
//!
//! Pass 3 (structural): validates code blocks for required fields, path safety,
//! and absence of secrets.

use std::sync::OnceLock;

use regex::Regex;

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::code::{CodeAction, CodeBlock};
use crate::model::node::Node;

// ---------------------------------------------------------------------------
// Secret detection regexes
// ---------------------------------------------------------------------------

static SECRET_KEYWORD: OnceLock<Regex> = OnceLock::new();
static AWS_KEY: OnceLock<Regex> = OnceLock::new();
static TOKEN_PREFIX: OnceLock<Regex> = OnceLock::new();

fn secret_keyword_regex() -> &'static Regex {
    SECRET_KEYWORD.get_or_init(|| {
        Regex::new(
            r#"(?i)(password|secret|api_key|api_secret|token|private_key)\s*[:=]\s*["'][^"']{8,}["']"#,
        )
        .unwrap()
    })
}

fn aws_key_regex() -> &'static Regex {
    AWS_KEY.get_or_init(|| Regex::new(r"(AKIA|ASIA)[A-Z0-9]{16}").unwrap())
}

fn token_prefix_regex() -> &'static Regex {
    TOKEN_PREFIX.get_or_init(|| {
        Regex::new(r"(?i)(sk-|pk_live_|pk_test_|ghp_|gho_|glpat-)[a-zA-Z0-9]{20,}").unwrap()
    })
}

/// Returns true if the body appears to contain a secret credential.
fn contains_secret(body: &str) -> bool {
    secret_keyword_regex().is_match(body)
        || aws_key_regex().is_match(body)
        || token_prefix_regex().is_match(body)
}

/// Returns true if the target path is unsafe (absolute or traversal).
fn is_unsafe_path(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || path.contains("..")
}

/// Validates a single code block, returning any errors.
fn validate_block(
    block: &CodeBlock,
    node_id: &str,
    line: usize,
    file_name: &str,
    errors: &mut Vec<AgmError>,
) {
    let loc = ErrorLocation::full(file_name, line, node_id);

    // V008 — lang must be present
    if block.lang.is_none() {
        errors.push(AgmError::new(
            ErrorCode::V008,
            "Code block missing required field: `lang`",
            loc.clone(),
        ));
    }

    // V008 — body must not be empty
    if block.body.trim().is_empty() {
        errors.push(AgmError::new(
            ErrorCode::V008,
            "Code block missing required field: `body` (empty)",
            loc.clone(),
        ));
    }

    // V008 — replace action requires `old`
    if block.action == CodeAction::Replace && block.old.is_none() {
        errors.push(AgmError::new(
            ErrorCode::V008,
            "Code block with `action: replace` missing required field: `old`",
            loc.clone(),
        ));
    }

    // V008 — insert_before / insert_after require `anchor`
    if matches!(
        block.action,
        CodeAction::InsertBefore | CodeAction::InsertAfter
    ) && block.anchor.is_none()
    {
        errors.push(AgmError::new(
            ErrorCode::V008,
            format!(
                "Code block with `action: {}` missing required field: `anchor`",
                block.action
            ),
            loc.clone(),
        ));
    }

    // V015 — target path must be relative and traversal-free
    if let Some(ref target) = block.target {
        if is_unsafe_path(target) {
            errors.push(AgmError::new(
                ErrorCode::V015,
                format!("`target` path is absolute or contains traversal: `{target}`"),
                loc.clone(),
            ));
        }
    }

    // V008 — body must not contain secrets (security heuristic)
    if contains_secret(&block.body) {
        errors.push(AgmError::new(
            ErrorCode::V008,
            "Code block appears to contain a secret or credential",
            loc,
        ));
    }
}

/// Validates all code blocks on a node (both `code` and `code_blocks`).
///
/// Rules: V008 (missing lang, empty body, replace without old,
/// insert without anchor, secret detection), V015 (unsafe target path).
#[must_use]
pub fn validate_code(node: &Node, file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();
    let line = node.span.start_line;
    let id = node.id.as_str();

    if let Some(ref block) = node.code {
        validate_block(block, id, line, file_name, &mut errors);
    }

    if let Some(ref blocks) = node.code_blocks {
        for block in blocks {
            validate_block(block, id, line, file_name, &mut errors);
        }
    }

    errors
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::model::code::{CodeAction, CodeBlock};
    use crate::model::fields::{NodeType, Span};
    use crate::model::node::Node;

    fn minimal_node() -> Node {
        Node {
            id: "test.node".to_owned(),
            node_type: NodeType::Facts,
            summary: "a test node".to_owned(),
            span: Span::new(5, 7),
            ..Default::default()
        }
    }

    fn valid_block() -> CodeBlock {
        CodeBlock {
            lang: Some("rust".to_owned()),
            target: Some("src/main.rs".to_owned()),
            action: CodeAction::Append,
            body: "fn hello() {}".to_owned(),
            anchor: None,
            old: None,
        }
    }

    #[test]
    fn test_validate_code_no_code_returns_empty() {
        let node = minimal_node();
        let errors = validate_code(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_code_valid_block_returns_empty() {
        let mut node = minimal_node();
        node.code = Some(valid_block());
        let errors = validate_code(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_code_missing_lang_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.lang = None;
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V008 && e.message.contains("lang"))
        );
    }

    #[test]
    fn test_validate_code_empty_body_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.body = "   ".to_owned();
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V008 && e.message.contains("body"))
        );
    }

    #[test]
    fn test_validate_code_replace_no_old_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.action = CodeAction::Replace;
        block.old = None;
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V008 && e.message.contains("`old`"))
        );
    }

    #[test]
    fn test_validate_code_replace_with_old_returns_empty() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.action = CodeAction::Replace;
        block.old = Some("old code".to_owned());
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(!errors.iter().any(|e| e.message.contains("`old`")));
    }

    #[test]
    fn test_validate_code_insert_before_no_anchor_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.action = CodeAction::InsertBefore;
        block.anchor = None;
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V008 && e.message.contains("`anchor`"))
        );
    }

    #[test]
    fn test_validate_code_insert_after_no_anchor_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.action = CodeAction::InsertAfter;
        block.anchor = None;
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V008 && e.message.contains("`anchor`"))
        );
    }

    #[test]
    fn test_validate_code_absolute_target_returns_v015() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.target = Some("/etc/passwd".to_owned());
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V015));
    }

    #[test]
    fn test_validate_code_traversal_target_returns_v015() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.target = Some("src/../etc/secret".to_owned());
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V015));
    }

    #[test]
    fn test_validate_code_windows_absolute_target_returns_v015() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.target = Some("\\Windows\\System32".to_owned());
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V015));
    }

    #[test]
    fn test_validate_code_secret_password_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.body = r#"password = "super_secret_pass123""#.to_owned();
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V008 && e.message.contains("secret"))
        );
    }

    #[test]
    fn test_validate_code_secret_aws_key_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.body = "AKIAIOSFODNN7EXAMPLE".to_owned();
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V008));
    }

    #[test]
    fn test_validate_code_secret_github_token_returns_v008() {
        let mut node = minimal_node();
        let mut block = valid_block();
        block.body = "ghp_abcdefghijklmnopqrstuvwxyz1234".to_owned();
        node.code = Some(block);
        let errors = validate_code(&node, "test.agm");
        assert!(errors.iter().any(|e| e.code == ErrorCode::V008));
    }

    #[test]
    fn test_validate_code_validates_code_blocks_vec() {
        let mut node = minimal_node();
        let mut bad_block = valid_block();
        bad_block.lang = None;
        node.code_blocks = Some(vec![valid_block(), bad_block]);
        let errors = validate_code(&node, "test.agm");
        assert!(
            errors
                .iter()
                .any(|e| e.code == ErrorCode::V008 && e.message.contains("lang"))
        );
    }
}
