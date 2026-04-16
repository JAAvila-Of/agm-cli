//! Ticket node validation (spec §13.11, §14.4).
//!
//! Enforces ticket-specific cross-field rules:
//! - V031: non-create actions require `ticket_id`
//! - V032: ticket `title` must not exceed 200 characters

use crate::error::codes::ErrorCode;
use crate::error::diagnostic::{AgmError, ErrorLocation, Severity};
use crate::model::fields::{NodeType, TicketAction};
use crate::model::node::Node;

const TITLE_MAX_LEN: usize = 200;

/// Validates ticket-specific cross-field rules.
///
/// Returns an empty `Vec` for non-ticket nodes.
#[must_use]
pub fn validate_ticket(node: &Node, file_name: &str) -> Vec<AgmError> {
    if node.node_type != NodeType::Ticket {
        return Vec::new();
    }

    let mut errors = Vec::new();
    let loc = ErrorLocation::full(file_name, node.span.start_line, &node.id);

    // V031 — ticket_id required for non-create actions
    if let Some(action) = &node.action {
        let needs_id = !matches!(action, TicketAction::Create);
        if needs_id && node.ticket_id.is_none() {
            errors.push(AgmError::new(
                ErrorCode::V031,
                format!(
                    "Ticket `{}` with action `{}` requires `ticket_id`",
                    node.id, action
                ),
                loc.clone(),
            ));
        }
    }

    // V032 — title length cap (warning)
    if let Some(title) = &node.title {
        if title.chars().count() > TITLE_MAX_LEN {
            errors.push(AgmError::with_severity(
                ErrorCode::V032,
                Severity::Warning,
                format!("Ticket `{}` title exceeds 200 characters", node.id),
                loc,
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::{SddPhase, TicketAction};
    use crate::model::node::Node;

    fn ticket_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Ticket,
            summary: "a ticket".to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_ticket_create_without_id_returns_empty() {
        let mut node = ticket_node("test.ticket.create");
        node.action = Some(TicketAction::Create);
        let errors = validate_ticket(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_ticket_edit_without_id_returns_v031() {
        let mut node = ticket_node("test.ticket.edit");
        node.action = Some(TicketAction::Edit);
        let errors = validate_ticket(&node, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::V031);
    }

    #[test]
    fn test_validate_ticket_close_without_id_returns_v031() {
        let mut node = ticket_node("test.ticket.close");
        node.action = Some(TicketAction::Close);
        let errors = validate_ticket(&node, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::V031);
    }

    #[test]
    fn test_validate_ticket_archive_without_id_returns_v031() {
        let mut node = ticket_node("test.ticket.archive");
        node.action = Some(TicketAction::Archive);
        let errors = validate_ticket(&node, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::V031);
    }

    #[test]
    fn test_validate_ticket_split_without_id_returns_v031() {
        let mut node = ticket_node("test.ticket.split");
        node.action = Some(TicketAction::Split);
        let errors = validate_ticket(&node, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::V031);
    }

    #[test]
    fn test_validate_ticket_link_without_id_returns_v031() {
        let mut node = ticket_node("test.ticket.link");
        node.action = Some(TicketAction::Link);
        let errors = validate_ticket(&node, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::V031);
    }

    #[test]
    fn test_validate_ticket_edit_with_id_returns_empty() {
        let mut node = ticket_node("test.ticket.edit-with-id");
        node.action = Some(TicketAction::Edit);
        node.ticket_id = Some("some.ticket.id".to_owned());
        let errors = validate_ticket(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_ticket_title_200_returns_empty() {
        let mut node = ticket_node("test.ticket.title-ok");
        node.title = Some("a".repeat(200));
        let errors = validate_ticket(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_ticket_title_201_returns_v032_warning() {
        let mut node = ticket_node("test.ticket.title-long");
        node.title = Some("a".repeat(201));
        let errors = validate_ticket(&node, "test.agm");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, ErrorCode::V032);
        assert_eq!(errors[0].severity, Severity::Warning);
    }

    #[test]
    fn test_validate_ticket_title_unicode_200_chars_returns_empty() {
        // Each emoji is multiple bytes but counts as 1 char
        let mut node = ticket_node("test.ticket.unicode");
        node.title = Some("🎫".repeat(200)); // 200 chars, each 4 bytes
        let errors = validate_ticket(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_ticket_non_ticket_type_returns_empty() {
        let node = Node {
            id: "test.facts".to_owned(),
            node_type: NodeType::Facts,
            summary: "a facts node".to_owned(),
            action: Some(TicketAction::Edit), // would trigger V031 if ticket
            ..Default::default()
        };
        let errors = validate_ticket(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_ticket_no_action_returns_empty() {
        // No action = implicitly create, no ticket_id needed
        let node = ticket_node("test.ticket.no-action");
        let errors = validate_ticket(&node, "test.agm");
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_ticket_sdd_phase_field_ignored_in_ticket_validator() {
        let mut node = ticket_node("test.ticket.phases");
        node.sdd_phase = Some(SddPhase::Design);
        let errors = validate_ticket(&node, "test.agm");
        assert!(errors.is_empty());
    }
}
