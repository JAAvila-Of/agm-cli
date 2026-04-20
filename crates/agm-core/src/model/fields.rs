//! Primitive field types, enums, and spans used throughout the AGM model.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error returned when parsing a string into an enum variant fails.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("invalid {type_name} value: {value:?}")]
pub struct ParseEnumError {
    pub type_name: &'static str,
    pub value: String,
}

// ---------------------------------------------------------------------------
// FieldValue
// ---------------------------------------------------------------------------

/// A dynamically-typed AGM field value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Scalar(String),
    List(Vec<String>),
    Block(String),
}

// ---------------------------------------------------------------------------
// Span
// ---------------------------------------------------------------------------

/// Source location span for diagnostics. Line numbers are 1-indexed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Span {
    pub start_line: usize,
    pub end_line: usize,
}

impl Span {
    #[must_use]
    pub fn new(start_line: usize, end_line: usize) -> Self {
        Self {
            start_line,
            end_line,
        }
    }
}

// ---------------------------------------------------------------------------
// NODE_ID_PATTERN
// ---------------------------------------------------------------------------

/// Regex pattern for valid AGM node IDs.
///
/// A node ID consists of dot-or-hyphen-separated segments where each segment
/// starts with a lowercase letter followed by zero or more lowercase letters,
/// digits, or underscores.
///
/// Pattern: `^[a-z][a-z0-9_]*([.\-][a-z][a-z0-9_]*)*$`
///
/// Note: the spec (§11.2) documents `[a-z0-9]` per segment, but the reference
/// implementation allows `_` (underscore) as a non-breaking errata. See §11.2
/// errata note in `docs/spec/agm_spec_v1.2.0.md`.
///
/// Used by both the parser (`P002`) and the validator (`V021`) for defence-in-depth:
/// the parser rejects invalid IDs at parse time; the validator catches invalid IDs
/// on programmatically constructed nodes that bypass the parser (e.g. via
/// `build_unchecked`, `serde_json::from_str`, or direct `Node` struct construction).
pub const NODE_ID_PATTERN: &str = r"^[a-z][a-z0-9_]*([.\-][a-z][a-z0-9_]*)*$";

// ---------------------------------------------------------------------------
// NodeType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Facts,
    Rules,
    Workflow,
    Entity,
    Decision,
    Exception,
    Example,
    Glossary,
    AntiPattern,
    Orchestration,
    Ticket,
    Custom(String),
}

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Facts => write!(f, "facts"),
            Self::Rules => write!(f, "rules"),
            Self::Workflow => write!(f, "workflow"),
            Self::Entity => write!(f, "entity"),
            Self::Decision => write!(f, "decision"),
            Self::Exception => write!(f, "exception"),
            Self::Example => write!(f, "example"),
            Self::Glossary => write!(f, "glossary"),
            Self::AntiPattern => write!(f, "anti_pattern"),
            Self::Orchestration => write!(f, "orchestration"),
            Self::Ticket => write!(f, "ticket"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for NodeType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "facts" => Self::Facts,
            "rules" => Self::Rules,
            "workflow" => Self::Workflow,
            "entity" => Self::Entity,
            "decision" => Self::Decision,
            "exception" => Self::Exception,
            "example" => Self::Example,
            "glossary" => Self::Glossary,
            "anti_pattern" => Self::AntiPattern,
            "orchestration" => Self::Orchestration,
            "ticket" => Self::Ticket,
            other => Self::Custom(other.to_owned()),
        })
    }
}

// ---------------------------------------------------------------------------
// TicketAction
// ---------------------------------------------------------------------------

/// Declares the intent of a ticket emission (spec §14.4.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TicketAction {
    #[default]
    Create,
    Edit,
    Close,
    Archive,
    Split,
    Link,
}

impl fmt::Display for TicketAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create => write!(f, "create"),
            Self::Edit => write!(f, "edit"),
            Self::Close => write!(f, "close"),
            Self::Archive => write!(f, "archive"),
            Self::Split => write!(f, "split"),
            Self::Link => write!(f, "link"),
        }
    }
}

impl FromStr for TicketAction {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "create" => Ok(Self::Create),
            "edit" => Ok(Self::Edit),
            "close" => Ok(Self::Close),
            "archive" => Ok(Self::Archive),
            "split" => Ok(Self::Split),
            "link" => Ok(Self::Link),
            _ => Err(ParseEnumError {
                type_name: "TicketAction",
                value: s.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// SddPhase
// ---------------------------------------------------------------------------

/// Suggests the SDD pipeline phase the ticket belongs to (spec §14.4.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SddPhase {
    #[default]
    Backlog,
    Explore,
    Propose,
    Spec,
    Design,
    Tasks,
    Apply,
    Verify,
    Archive,
}

impl fmt::Display for SddPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Backlog => write!(f, "backlog"),
            Self::Explore => write!(f, "explore"),
            Self::Propose => write!(f, "propose"),
            Self::Spec => write!(f, "spec"),
            Self::Design => write!(f, "design"),
            Self::Tasks => write!(f, "tasks"),
            Self::Apply => write!(f, "apply"),
            Self::Verify => write!(f, "verify"),
            Self::Archive => write!(f, "archive"),
        }
    }
}

impl FromStr for SddPhase {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "backlog" => Ok(Self::Backlog),
            "explore" => Ok(Self::Explore),
            "propose" => Ok(Self::Propose),
            "spec" => Ok(Self::Spec),
            "design" => Ok(Self::Design),
            "tasks" => Ok(Self::Tasks),
            "apply" => Ok(Self::Apply),
            "verify" => Ok(Self::Verify),
            "archive" => Ok(Self::Archive),
            _ => Err(ParseEnumError {
                type_name: "SddPhase",
                value: s.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Priority
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Critical,
    High,
    Normal,
    Low,
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Critical => write!(f, "critical"),
            Self::High => write!(f, "high"),
            Self::Normal => write!(f, "normal"),
            Self::Low => write!(f, "low"),
        }
    }
}

impl FromStr for Priority {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "normal" => Ok(Self::Normal),
            "low" => Ok(Self::Low),
            _ => Err(ParseEnumError {
                type_name: "Priority",
                value: s.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Stability
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    High,
    Medium,
    Low,
    Volatile,
}

impl fmt::Display for Stability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Medium => write!(f, "medium"),
            Self::Low => write!(f, "low"),
            Self::Volatile => write!(f, "volatile"),
        }
    }
}

impl FromStr for Stability {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            "volatile" => Ok(Self::Volatile),
            _ => Err(ParseEnumError {
                type_name: "Stability",
                value: s.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Confidence
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
    Inferred,
    Tentative,
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Medium => write!(f, "medium"),
            Self::Low => write!(f, "low"),
            Self::Inferred => write!(f, "inferred"),
            Self::Tentative => write!(f, "tentative"),
        }
    }
}

impl FromStr for Confidence {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            "inferred" => Ok(Self::Inferred),
            "tentative" => Ok(Self::Tentative),
            _ => Err(ParseEnumError {
                type_name: "Confidence",
                value: s.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// NodeStatus
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Active,
    Draft,
    Deprecated,
    Superseded,
}

impl fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Draft => write!(f, "draft"),
            Self::Deprecated => write!(f, "deprecated"),
            Self::Superseded => write!(f, "superseded"),
        }
    }
}

impl FromStr for NodeStatus {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "draft" => Ok(Self::Draft),
            "deprecated" => Ok(Self::Deprecated),
            "superseded" => Ok(Self::Superseded),
            _ => Err(ParseEnumError {
                type_name: "NodeStatus",
                value: s.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_from_str_known_returns_variant() {
        assert_eq!("facts".parse::<NodeType>().unwrap(), NodeType::Facts);
        assert_eq!("rules".parse::<NodeType>().unwrap(), NodeType::Rules);
        assert_eq!("workflow".parse::<NodeType>().unwrap(), NodeType::Workflow);
        assert_eq!("entity".parse::<NodeType>().unwrap(), NodeType::Entity);
        assert_eq!("decision".parse::<NodeType>().unwrap(), NodeType::Decision);
        assert_eq!(
            "exception".parse::<NodeType>().unwrap(),
            NodeType::Exception
        );
        assert_eq!("example".parse::<NodeType>().unwrap(), NodeType::Example);
        assert_eq!("glossary".parse::<NodeType>().unwrap(), NodeType::Glossary);
        assert_eq!(
            "anti_pattern".parse::<NodeType>().unwrap(),
            NodeType::AntiPattern
        );
        assert_eq!(
            "orchestration".parse::<NodeType>().unwrap(),
            NodeType::Orchestration
        );
        assert_eq!("ticket".parse::<NodeType>().unwrap(), NodeType::Ticket);
    }

    #[test]
    fn test_node_type_from_str_unknown_returns_custom() {
        assert_eq!(
            "unknown_custom".parse::<NodeType>().unwrap(),
            NodeType::Custom("unknown_custom".to_owned())
        );
    }

    #[test]
    fn test_node_type_display_roundtrip() {
        let types = [
            NodeType::Facts,
            NodeType::Rules,
            NodeType::Workflow,
            NodeType::Entity,
            NodeType::Decision,
            NodeType::Exception,
            NodeType::Example,
            NodeType::Glossary,
            NodeType::AntiPattern,
            NodeType::Orchestration,
            NodeType::Ticket,
            NodeType::Custom("my_type".to_owned()),
        ];
        for t in &types {
            let s = t.to_string();
            let parsed: NodeType = s.parse().unwrap();
            assert_eq!(&parsed, t);
        }
    }

    #[test]
    fn test_priority_from_str_valid_returns_ok() {
        assert_eq!("critical".parse::<Priority>().unwrap(), Priority::Critical);
        assert_eq!("high".parse::<Priority>().unwrap(), Priority::High);
        assert_eq!("normal".parse::<Priority>().unwrap(), Priority::Normal);
        assert_eq!("low".parse::<Priority>().unwrap(), Priority::Low);
    }

    #[test]
    fn test_priority_from_str_invalid_returns_error() {
        let err = "invalid".parse::<Priority>().unwrap_err();
        assert_eq!(err.type_name, "Priority");
        assert_eq!(err.value, "invalid");
    }

    #[test]
    fn test_priority_display_roundtrip() {
        for p in [
            Priority::Critical,
            Priority::High,
            Priority::Normal,
            Priority::Low,
        ] {
            let s = p.to_string();
            assert_eq!(s.parse::<Priority>().unwrap(), p);
        }
    }

    #[test]
    fn test_stability_from_str_valid_returns_ok() {
        assert_eq!("high".parse::<Stability>().unwrap(), Stability::High);
        assert_eq!("medium".parse::<Stability>().unwrap(), Stability::Medium);
        assert_eq!("low".parse::<Stability>().unwrap(), Stability::Low);
        assert_eq!(
            "volatile".parse::<Stability>().unwrap(),
            Stability::Volatile
        );
    }

    #[test]
    fn test_stability_from_str_invalid_returns_error() {
        let err = "wrong".parse::<Stability>().unwrap_err();
        assert_eq!(err.type_name, "Stability");
    }

    #[test]
    fn test_stability_display_roundtrip() {
        for s in [
            Stability::High,
            Stability::Medium,
            Stability::Low,
            Stability::Volatile,
        ] {
            let text = s.to_string();
            assert_eq!(text.parse::<Stability>().unwrap(), s);
        }
    }

    #[test]
    fn test_confidence_from_str_valid_returns_ok() {
        assert_eq!("high".parse::<Confidence>().unwrap(), Confidence::High);
        assert_eq!("medium".parse::<Confidence>().unwrap(), Confidence::Medium);
        assert_eq!("low".parse::<Confidence>().unwrap(), Confidence::Low);
        assert_eq!(
            "inferred".parse::<Confidence>().unwrap(),
            Confidence::Inferred
        );
        assert_eq!(
            "tentative".parse::<Confidence>().unwrap(),
            Confidence::Tentative
        );
    }

    #[test]
    fn test_confidence_from_str_invalid_returns_error() {
        let err = "maybe".parse::<Confidence>().unwrap_err();
        assert_eq!(err.type_name, "Confidence");
    }

    #[test]
    fn test_confidence_display_roundtrip() {
        for c in [
            Confidence::High,
            Confidence::Medium,
            Confidence::Low,
            Confidence::Inferred,
            Confidence::Tentative,
        ] {
            let text = c.to_string();
            assert_eq!(text.parse::<Confidence>().unwrap(), c);
        }
    }

    #[test]
    fn test_node_status_from_str_valid_returns_ok() {
        assert_eq!("active".parse::<NodeStatus>().unwrap(), NodeStatus::Active);
        assert_eq!("draft".parse::<NodeStatus>().unwrap(), NodeStatus::Draft);
        assert_eq!(
            "deprecated".parse::<NodeStatus>().unwrap(),
            NodeStatus::Deprecated
        );
        assert_eq!(
            "superseded".parse::<NodeStatus>().unwrap(),
            NodeStatus::Superseded
        );
    }

    #[test]
    fn test_node_status_from_str_invalid_returns_error() {
        let err = "archived".parse::<NodeStatus>().unwrap_err();
        assert_eq!(err.type_name, "NodeStatus");
    }

    #[test]
    fn test_node_status_display_roundtrip() {
        for ns in [
            NodeStatus::Active,
            NodeStatus::Draft,
            NodeStatus::Deprecated,
            NodeStatus::Superseded,
        ] {
            let text = ns.to_string();
            assert_eq!(text.parse::<NodeStatus>().unwrap(), ns);
        }
    }

    #[test]
    fn test_field_value_scalar_debug() {
        let v = FieldValue::Scalar("hello".to_owned());
        assert!(format!("{v:?}").contains("Scalar"));
    }

    #[test]
    fn test_field_value_list_clone() {
        let v = FieldValue::List(vec!["a".to_owned(), "b".to_owned()]);
        assert_eq!(v.clone(), v);
    }

    #[test]
    fn test_field_value_serde_roundtrip_scalar() {
        let v = FieldValue::Scalar("test".to_owned());
        let json = serde_json::to_string(&v).unwrap();
        let back: FieldValue = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_field_value_serde_roundtrip_list() {
        let v = FieldValue::List(vec!["a".to_owned(), "b".to_owned()]);
        let json = serde_json::to_string(&v).unwrap();
        let back: FieldValue = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn test_span_new() {
        let s = Span::new(1, 10);
        assert_eq!(s.start_line, 1);
        assert_eq!(s.end_line, 10);
    }

    #[test]
    fn test_ticket_action_from_str_valid_returns_ok() {
        assert_eq!(
            "create".parse::<TicketAction>().unwrap(),
            TicketAction::Create
        );
        assert_eq!("edit".parse::<TicketAction>().unwrap(), TicketAction::Edit);
        assert_eq!(
            "close".parse::<TicketAction>().unwrap(),
            TicketAction::Close
        );
        assert_eq!(
            "archive".parse::<TicketAction>().unwrap(),
            TicketAction::Archive
        );
        assert_eq!(
            "split".parse::<TicketAction>().unwrap(),
            TicketAction::Split
        );
        assert_eq!("link".parse::<TicketAction>().unwrap(), TicketAction::Link);
    }

    #[test]
    fn test_ticket_action_from_str_invalid_returns_error() {
        let err = "delete".parse::<TicketAction>().unwrap_err();
        assert_eq!(err.type_name, "TicketAction");
        assert_eq!(err.value, "delete");
    }

    #[test]
    fn test_ticket_action_display_roundtrip() {
        for a in [
            TicketAction::Create,
            TicketAction::Edit,
            TicketAction::Close,
            TicketAction::Archive,
            TicketAction::Split,
            TicketAction::Link,
        ] {
            let s = a.to_string();
            assert_eq!(s.parse::<TicketAction>().unwrap(), a);
        }
    }

    #[test]
    fn test_ticket_action_default_is_create() {
        assert_eq!(TicketAction::default(), TicketAction::Create);
    }

    #[test]
    fn test_sdd_phase_from_str_valid_returns_ok() {
        assert_eq!("backlog".parse::<SddPhase>().unwrap(), SddPhase::Backlog);
        assert_eq!("explore".parse::<SddPhase>().unwrap(), SddPhase::Explore);
        assert_eq!("propose".parse::<SddPhase>().unwrap(), SddPhase::Propose);
        assert_eq!("spec".parse::<SddPhase>().unwrap(), SddPhase::Spec);
        assert_eq!("design".parse::<SddPhase>().unwrap(), SddPhase::Design);
        assert_eq!("tasks".parse::<SddPhase>().unwrap(), SddPhase::Tasks);
        assert_eq!("apply".parse::<SddPhase>().unwrap(), SddPhase::Apply);
        assert_eq!("verify".parse::<SddPhase>().unwrap(), SddPhase::Verify);
        assert_eq!("archive".parse::<SddPhase>().unwrap(), SddPhase::Archive);
    }

    #[test]
    fn test_sdd_phase_from_str_invalid_returns_error() {
        let err = "unknown".parse::<SddPhase>().unwrap_err();
        assert_eq!(err.type_name, "SddPhase");
        assert_eq!(err.value, "unknown");
    }

    #[test]
    fn test_sdd_phase_display_roundtrip() {
        for p in [
            SddPhase::Backlog,
            SddPhase::Explore,
            SddPhase::Propose,
            SddPhase::Spec,
            SddPhase::Design,
            SddPhase::Tasks,
            SddPhase::Apply,
            SddPhase::Verify,
            SddPhase::Archive,
        ] {
            let s = p.to_string();
            assert_eq!(s.parse::<SddPhase>().unwrap(), p);
        }
    }

    #[test]
    fn test_sdd_phase_default_is_backlog() {
        assert_eq!(SddPhase::default(), SddPhase::Backlog);
    }
}
