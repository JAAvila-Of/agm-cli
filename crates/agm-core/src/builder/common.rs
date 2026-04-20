//! Shared helpers for the builder module.
//!
//! These are `pub(super)` — internal to the `builder` crate module.

use crate::error::diagnostic::DiagnosticCollection;
use crate::model::file::{AgmFile, Header};
use crate::model::node::Node;
use crate::model::schema::EnforcementLevel;
use crate::validator::{self, ValidateOptions, ValidationScope};

use super::error::BuildError;

// ---------------------------------------------------------------------------
// Scratch header
// ---------------------------------------------------------------------------

/// A minimal header used when wrapping a single node for validation or rendering.
///
/// The values are intentionally non-real so they're easy to identify and strip
/// when rendering node-only output.
pub(super) const SCRATCH_AGM: &str = "1.0";
pub(super) const SCRATCH_PACKAGE: &str = "scratch.builder";
pub(super) const SCRATCH_VERSION: &str = "0.1.0";

pub(super) fn scratch_header() -> Header {
    Header {
        agm: SCRATCH_AGM.to_owned(),
        package: SCRATCH_PACKAGE.to_owned(),
        version: SCRATCH_VERSION.to_owned(),
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

pub(super) fn wrap_node(node: Node) -> AgmFile {
    AgmFile {
        header: scratch_header(),
        nodes: vec![node],
    }
}

// ---------------------------------------------------------------------------
// validate_single_node
// ---------------------------------------------------------------------------

/// Wraps `node` in a scratch `AgmFile`, runs the validator at `level` with
/// `ValidationScope::SingleNode`, and returns the node if no errors are found.
///
/// `SingleNode` scope skips cross-node reference checks (Pass 5: cycle
/// detection, reference resolution) and cross-node reference checks within
/// Pass 3 (V004 for `agent_context.load_nodes`, `verify.node_status`,
/// and orchestration group-node refs). These checks require the full node-set
/// of a complete file and would produce spurious errors in isolation.
///
/// Schema/type/format checks (Passes 1–4) still run in full.
pub(crate) fn validate_single_node(
    node: Node,
    level: EnforcementLevel,
) -> Result<Node, BuildError> {
    let file = wrap_node(node);
    let opts = ValidateOptions {
        enforcement_level: level,
        import_resolver: None,
        scope: ValidationScope::SingleNode,
    };
    let diags: DiagnosticCollection = validator::validate(&file, "", "<builder>", &opts);

    if diags.has_errors() {
        return Err(BuildError::Validation(Box::new(diags)));
    }

    // Unwrap is safe: we just put exactly one node in.
    Ok(file.nodes.into_iter().next().unwrap())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::{NodeType, Priority};

    fn valid_ticket_node() -> Node {
        Node {
            id: "test.ticket.x".to_owned(),
            node_type: NodeType::Ticket,
            summary: "a valid ticket".to_owned(),
            title: Some("A valid ticket".to_owned()),
            description: Some("Detailed description here.".to_owned()),
            priority: Some(Priority::Normal),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_single_node_with_cross_node_load_nodes_no_v004() {
        // In SingleNode scope, load_nodes referencing a non-existent node must NOT
        // produce V004. The reference will be validated at file-level when assembled.
        use crate::model::context::AgentContext;
        let node = Node {
            id: "test.ticket.x".to_owned(),
            node_type: NodeType::Ticket,
            summary: "ticket with external load_nodes".to_owned(),
            title: Some("Ticket X".to_owned()),
            description: Some("desc".to_owned()),
            priority: Some(Priority::Normal),
            agent_context: Some(AgentContext {
                load_nodes: Some(vec!["other.file.node".to_owned()]),
                load_files: None,
                system_hint: None,
                max_tokens: None,
                load_memory: None,
            }),
            ..Default::default()
        };
        let result = validate_single_node(node, EnforcementLevel::Standard);
        assert!(
            result.is_ok(),
            "SingleNode scope must not fire V004 for cross-node load_nodes: {result:?}"
        );
    }

    #[test]
    fn test_validate_file_scope_with_cross_node_load_nodes_fires_v004() {
        // File scope must still fire V004 for an unresolved load_nodes reference.
        use crate::model::context::AgentContext;
        use crate::validator::{ValidateOptions, ValidationScope};
        let node = Node {
            id: "test.ticket.x".to_owned(),
            node_type: NodeType::Ticket,
            summary: "ticket".to_owned(),
            title: Some("T".to_owned()),
            description: Some("d".to_owned()),
            priority: Some(Priority::Normal),
            agent_context: Some(AgentContext {
                load_nodes: Some(vec!["missing.node".to_owned()]),
                load_files: None,
                system_hint: None,
                max_tokens: None,
                load_memory: None,
            }),
            ..Default::default()
        };
        let file = wrap_node(node);
        let opts = ValidateOptions {
            enforcement_level: EnforcementLevel::Standard,
            import_resolver: None,
            scope: ValidationScope::File,
        };
        let diags = crate::validator::validate(&file, "", "<test>", &opts);
        assert!(
            diags
                .diagnostics()
                .iter()
                .any(|d| d.code == crate::error::codes::ErrorCode::V004),
            "File scope must fire V004 for unresolved load_nodes"
        );
    }

    #[test]
    fn test_validate_single_node_valid_returns_ok() {
        let node = valid_ticket_node();
        let result = validate_single_node(node, EnforcementLevel::Standard);
        assert!(result.is_ok(), "valid ticket should pass: {result:?}");
    }

    #[test]
    fn test_validate_single_node_invalid_id_returns_error() {
        let node = Node {
            id: "BAD ID WITH SPACES".to_owned(),
            node_type: NodeType::Facts,
            summary: "test".to_owned(),
            ..Node::default()
        };
        let result = validate_single_node(node, EnforcementLevel::Standard);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_validation());
    }

    #[test]
    fn test_validate_single_node_wraps_and_unwraps() {
        // The returned node should be the same node we passed in.
        let node = valid_ticket_node();
        let id = node.id.clone();
        let returned = validate_single_node(node, EnforcementLevel::Standard).unwrap();
        assert_eq!(returned.id, id);
    }
}
