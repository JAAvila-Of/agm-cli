//! File-level validation (spec S8, S9).
//!
//! Pass 1: validates header required fields and ensures at least one node exists.

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation};
use crate::model::file::AgmFile;

/// Validates file-level invariants: required header fields and node presence.
///
/// Rules: P001 (missing header field), P008 (no nodes).
#[must_use]
pub fn validate_file(file: &AgmFile, file_name: &str) -> Vec<AgmError> {
    let mut errors = Vec::new();

    // P001 — required header fields must be non-empty
    if file.header.agm.trim().is_empty() {
        errors.push(AgmError::new(
            ErrorCode::P001,
            "Missing required header field: `agm`",
            ErrorLocation::file_line(file_name, 1),
        ));
    }

    if file.header.package.trim().is_empty() {
        errors.push(AgmError::new(
            ErrorCode::P001,
            "Missing required header field: `package`",
            ErrorLocation::file_line(file_name, 1),
        ));
    }

    if file.header.version.trim().is_empty() {
        errors.push(AgmError::new(
            ErrorCode::P001,
            "Missing required header field: `version`",
            ErrorLocation::file_line(file_name, 1),
        ));
    }

    // P008 — at least one node is required
    if file.nodes.is_empty() {
        errors.push(AgmError::new(
            ErrorCode::P008,
            "Empty file (no nodes)",
            ErrorLocation::file_line(file_name, 1),
        ));
    }

    errors
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::model::fields::{NodeType, Span};
    use crate::model::file::{AgmFile, Header};
    use crate::model::node::Node;

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

    fn minimal_node() -> Node {
        Node {
            id: "test.node".to_owned(),
            node_type: NodeType::Facts,
            summary: "a test node".to_owned(),
            span: Span::new(5, 7),
            ..Default::default()
        }
    }

    fn file_with_node() -> AgmFile {
        AgmFile {
            header: minimal_header(),
            nodes: vec![minimal_node()],
        }
    }

    #[test]
    fn test_validate_file_valid_returns_empty() {
        let file = file_with_node();
        let errors = validate_file(&file, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_file_empty_agm_returns_p001() {
        let mut file = file_with_node();
        file.header.agm = String::new();
        let errors = validate_file(&file, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::P001);
        assert!(errors[0].message.contains("`agm`"));
    }

    #[test]
    fn test_validate_file_empty_package_returns_p001() {
        let mut file = file_with_node();
        file.header.package = String::new();
        let errors = validate_file(&file, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::P001);
        assert!(errors[0].message.contains("`package`"));
    }

    #[test]
    fn test_validate_file_empty_version_returns_p001() {
        let mut file = file_with_node();
        file.header.version = String::new();
        let errors = validate_file(&file, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::P001);
        assert!(errors[0].message.contains("`version`"));
    }

    #[test]
    fn test_validate_file_no_nodes_returns_p008() {
        let mut file = file_with_node();
        file.nodes.clear();
        let errors = validate_file(&file, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::P008);
    }

    #[test]
    fn test_validate_file_all_headers_empty_returns_three_p001() {
        let mut file = file_with_node();
        file.header.agm = String::new();
        file.header.package = String::new();
        file.header.version = String::new();
        let errors = validate_file(&file, "test.agm");
        assert_eq!(errors.len(), 3);
        assert!(errors.iter().all(|e| e.code == ErrorCode::P001));
    }

    #[test]
    fn test_validate_file_whitespace_only_agm_returns_p001() {
        let mut file = file_with_node();
        file.header.agm = "   ".to_owned();
        let errors = validate_file(&file, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::P001);
    }
}
