//! Fluent builder for `NodeType::Ticket` nodes (spec §13.11).

use std::collections::HashSet;

use crate::model::code::CodeBlock;
use crate::model::context::AgentContext;
use crate::model::fields::{FieldValue, NodeType, Priority, SddPhase, Stability, TicketAction};
use crate::model::node::Node;
use crate::model::schema::EnforcementLevel;

use super::common::validate_single_node;
use super::error::BuildError;

/// Collects `items` into a `Vec<String>`, removing duplicates while preserving
/// first-appearance order.
fn dedup_preserve_order<I, S>(items: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut seen = HashSet::new();
    items
        .into_iter()
        .map(Into::into)
        .filter(|s: &String| seen.insert(s.clone()))
        .collect()
}

// ---------------------------------------------------------------------------
// TicketBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for `NodeType::Ticket` nodes (spec §13.11, §14).
///
/// Required fields (enforced by `.build()` via the standard validator):
/// `summary`, `title`, `description`, `priority`.
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::TicketBuilder;
/// use agm_core::model::fields::Priority;
///
/// let node = TicketBuilder::new("my.ticket.oauth")
///     .summary("add OAuth2 login")
///     .title("Add OAuth2 login flow")
///     .description("Add Google OAuth2 login to the dashboard.")
///     .priority(Priority::High)
///     .build()
///     .expect("valid ticket");
///
/// assert_eq!(node.id, "my.ticket.oauth");
/// ```
#[derive(Debug, Clone)]
pub struct TicketBuilder {
    node: Node,
}

impl TicketBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Starts a new builder with the given node ID.
    ///
    /// The ID is stored as-is; validation of the node-ID pattern
    /// (`[a-z][a-z0-9_.]*`) is deferred to `.build()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("my.ticket.foo");
    /// ```
    pub fn new<S: Into<String>>(id: S) -> Self {
        let node = Node {
            id: id.into(),
            node_type: NodeType::Ticket,
            ..Default::default()
        };
        Self { node }
    }

    // -----------------------------------------------------------------------
    // Required setters
    // -----------------------------------------------------------------------

    /// Sets the one-line `summary` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").summary("add login flow");
    /// ```
    pub fn summary<S: Into<String>>(mut self, s: S) -> Self {
        self.node.summary = s.into();
        self
    }

    /// Sets the human-readable `title`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").title("Add Login Flow");
    /// ```
    pub fn title<S: Into<String>>(mut self, s: S) -> Self {
        self.node.title = Some(s.into());
        self
    }

    /// Sets the multi-line `description`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").description("Detailed description.");
    /// ```
    pub fn description<S: Into<String>>(mut self, s: S) -> Self {
        self.node.description = Some(s.into());
        self
    }

    /// Sets the `priority`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::fields::Priority;
    ///
    /// let b = TicketBuilder::new("t.x").priority(Priority::High);
    /// ```
    pub fn priority(mut self, p: Priority) -> Self {
        self.node.priority = Some(p);
        self
    }

    // -----------------------------------------------------------------------
    // Recommended setters
    // -----------------------------------------------------------------------

    /// Sets the `action` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::fields::TicketAction;
    ///
    /// let b = TicketBuilder::new("t.x").action(TicketAction::Create);
    /// ```
    pub fn action(mut self, a: TicketAction) -> Self {
        self.node.action = Some(a);
        self
    }

    /// Sets the `sdd_phase` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::fields::SddPhase;
    ///
    /// let b = TicketBuilder::new("t.x").sdd_phase(SddPhase::Apply);
    /// ```
    pub fn sdd_phase(mut self, p: SddPhase) -> Self {
        self.node.sdd_phase = Some(p);
        self
    }

    /// Sets the `labels` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").labels(["auth", "security"]);
    /// ```
    pub fn labels<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.labels = Some(dedup_preserve_order(items));
        self
    }

    // -----------------------------------------------------------------------
    // Optional setters
    // -----------------------------------------------------------------------

    /// Sets the `prompt` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").prompt("Implement OAuth2 login.");
    /// ```
    pub fn prompt<S: Into<String>>(mut self, s: S) -> Self {
        self.node.prompt = Some(s.into());
        self
    }

    /// Sets the `assignee` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").assignee("agent-01");
    /// ```
    pub fn assignee<S: Into<String>>(mut self, s: S) -> Self {
        self.node.assignee = Some(s.into());
        self
    }

    /// Sets the `ticket_id` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").ticket_id("GH-42");
    /// ```
    pub fn ticket_id<S: Into<String>>(mut self, s: S) -> Self {
        self.node.ticket_id = Some(s.into());
        self
    }

    /// Sets the `detail` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").detail("Additional implementation notes.");
    /// ```
    pub fn detail<S: Into<String>>(mut self, s: S) -> Self {
        self.node.detail = Some(s.into());
        self
    }

    /// Sets the `stability` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::fields::Stability;
    ///
    /// let b = TicketBuilder::new("t.x").stability(Stability::High);
    /// ```
    pub fn stability(mut self, s: Stability) -> Self {
        self.node.stability = Some(s);
        self
    }

    /// Sets the `depends` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").depends(["auth.constraints"]);
    /// ```
    pub fn depends<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.depends = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the `related_to` list, deduplicating while preserving first-appearance order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").related_to(["auth.session"]);
    /// ```
    pub fn related_to<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.related_to = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the `replaces` list, deduplicating while preserving first-appearance order.
    ///
    /// Indicates semantic replacement or supersession (spec §15.3).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").replaces(["t.old"]);
    /// ```
    pub fn replaces<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.replaces = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the `conflicts` list, deduplicating while preserving first-appearance order.
    ///
    /// Indicates mutual incompatibility (spec §15.4).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").conflicts(["t.conflicting"]);
    /// ```
    pub fn conflicts<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.conflicts = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the `see_also` list, deduplicating while preserving first-appearance order.
    ///
    /// Soft related navigation (spec §15.5).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").see_also(["t.related"]);
    /// ```
    pub fn see_also<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.see_also = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the `tags` list, deduplicating while preserving first-appearance order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").tags(["backend", "rust"]);
    /// ```
    pub fn tags<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.tags = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the `notes` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// let b = TicketBuilder::new("t.x").notes("See RFC-42 for background.");
    /// ```
    pub fn notes<S: Into<String>>(mut self, s: S) -> Self {
        self.node.notes = Some(s.into());
        self
    }

    /// Sets the `agent_context` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::context::AgentContext;
    ///
    /// let ctx = AgentContext {
    ///     load_nodes: Some(vec!["auth.constraints".to_owned()]),
    ///     load_files: None,
    ///     system_hint: None,
    ///     max_tokens: None,
    ///     load_memory: None,
    /// };
    /// let b = TicketBuilder::new("t.x").agent_context(ctx);
    /// ```
    pub fn agent_context(mut self, ctx: AgentContext) -> Self {
        self.node.agent_context = Some(ctx);
        self
    }

    /// Sets the `code_blocks` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::{TicketBuilder, CodeBlockBuilder};
    ///
    /// let cb = CodeBlockBuilder::create()
    ///     .lang("rust")
    ///     .target("src/auth.rs")
    ///     .body("pub fn auth() {}")
    ///     .build()
    ///     .unwrap();
    ///
    /// let b = TicketBuilder::new("t.x").code_blocks([cb]);
    /// ```
    pub fn code_blocks<I>(mut self, blocks: I) -> Self
    where
        I: IntoIterator<Item = CodeBlock>,
    {
        self.node.code_blocks = Some(blocks.into_iter().collect());
        self
    }

    /// Inserts an arbitrary field into the node's `extra_fields` map.
    ///
    /// Use this escape hatch to preserve model-emitted fields that are not
    /// covered by a typed setter. Unknown fields are stored for audit and
    /// will be rendered in canonical output.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::fields::{FieldValue, Priority};
    ///
    /// let node = TicketBuilder::new("t.x")
    ///     .summary("s")
    ///     .title("T")
    ///     .description("d")
    ///     .priority(Priority::Normal)
    ///     .extra("custom_field", FieldValue::Scalar("value".to_owned()))
    ///     .build()
    ///     .unwrap();
    ///
    /// assert!(node.extra_fields.contains_key("custom_field"));
    /// ```
    pub fn extra(mut self, key: impl Into<String>, value: FieldValue) -> Self {
        self.node.extra_fields.insert(key.into(), value);
        self
    }

    // -----------------------------------------------------------------------
    // Terminal
    // -----------------------------------------------------------------------

    /// Validates and finalizes the node using `Standard` enforcement.
    ///
    /// Equivalent to `build_with(EnforcementLevel::Standard)`.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Validation`] if the validator finds any errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::fields::Priority;
    ///
    /// let node = TicketBuilder::new("my.ticket.foo")
    ///     .summary("do something")
    ///     .title("Do Something")
    ///     .description("Detailed description.")
    ///     .priority(Priority::Normal)
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "my.ticket.foo");
    /// ```
    pub fn build(self) -> Result<Node, BuildError> {
        self.build_with(EnforcementLevel::Standard)
    }

    /// Validates and finalizes the node at the specified enforcement level.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Validation`] if the validator finds any errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    /// use agm_core::model::fields::Priority;
    /// use agm_core::model::schema::EnforcementLevel;
    ///
    /// let node = TicketBuilder::new("my.ticket.foo")
    ///     .summary("do something")
    ///     .title("Do Something")
    ///     .description("Detailed description.")
    ///     .priority(Priority::Normal)
    ///     .build_with(EnforcementLevel::Standard)
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "my.ticket.foo");
    /// ```
    pub fn build_with(self, level: EnforcementLevel) -> Result<Node, BuildError> {
        validate_single_node(self.node, level)
    }

    /// Builds without running the validator.
    ///
    /// Use when you intend to compose the resulting `Node` into a multi-node
    /// `AgmFile` that will be validated as a whole. Cross-node checks (cycle
    /// detection, reference resolution) are skipped on single-node validation
    /// anyway, but this avoids even the per-node passes.
    ///
    /// # Errors
    ///
    /// Returns `Err` only if the node ID is empty (a hard precondition).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::TicketBuilder;
    ///
    /// // build_unchecked skips validation — useful for downstream composition.
    /// let node = TicketBuilder::new("t.draft")
    ///     .summary("wip")
    ///     .build_unchecked()
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "t.draft");
    /// ```
    pub fn build_unchecked(self) -> Result<Node, BuildError> {
        if self.node.id.is_empty() {
            return Err(BuildError::Precondition(
                "node ID must not be empty".to_owned(),
            ));
        }
        Ok(self.node)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::fields::{Priority, SddPhase, TicketAction};

    fn minimal_valid() -> TicketBuilder {
        TicketBuilder::new("test.ticket.login")
            .summary("add login")
            .title("Add Login")
            .description("Implement login flow.")
            .priority(Priority::Normal)
    }

    #[test]
    fn test_minimal_build_produces_valid_node() {
        let node = minimal_valid().build().unwrap();
        assert_eq!(node.id, "test.ticket.login");
        assert_eq!(node.node_type, NodeType::Ticket);
        assert_eq!(node.summary, "add login");
    }

    #[test]
    fn test_missing_title_returns_validation_error() {
        let result = TicketBuilder::new("test.ticket.x")
            .summary("s")
            .description("d")
            .priority(Priority::Normal)
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_missing_description_returns_validation_error() {
        let result = TicketBuilder::new("test.ticket.x")
            .summary("s")
            .title("T")
            .priority(Priority::Normal)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_priority_returns_validation_error() {
        let result = TicketBuilder::new("test.ticket.x")
            .summary("s")
            .title("T")
            .description("d")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_chain_returns_self() {
        // Fluent API sanity — each setter returns Self
        let node = TicketBuilder::new("test.ticket.chain")
            .summary("s")
            .title("T")
            .description("d")
            .priority(Priority::High)
            .action(TicketAction::Create)
            .sdd_phase(SddPhase::Apply)
            .labels(["auth"])
            .notes("n")
            .build()
            .unwrap();
        assert!(node.action.is_some());
        assert!(node.sdd_phase.is_some());
        assert!(node.labels.is_some());
    }

    #[test]
    fn test_labels_accepts_vec_and_iterator() {
        let v = vec!["auth".to_owned(), "security".to_owned()];
        let node = minimal_valid().labels(v).build().unwrap();
        assert_eq!(node.labels.as_deref().unwrap().len(), 2);
    }

    #[test]
    fn test_action_enum_typed() {
        // Edit action requires ticket_id (V031); use Create to avoid that error.
        let node = minimal_valid()
            .action(TicketAction::Create)
            .build()
            .unwrap();
        assert_eq!(node.action, Some(TicketAction::Create));
    }

    #[test]
    fn test_edit_without_ticket_id_strict_errors() {
        // V031: Edit without ticket_id is an error regardless of enforcement level.
        let result = minimal_valid().action(TicketAction::Edit).build();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_validation());
        let diags = err.diagnostics().unwrap();
        assert!(
            diags
                .diagnostics()
                .iter()
                .any(|d| d.code == crate::error::codes::ErrorCode::V031)
        );
    }

    #[test]
    fn test_sdd_phase_enum_typed() {
        let node = minimal_valid().sdd_phase(SddPhase::Verify).build().unwrap();
        assert_eq!(node.sdd_phase, Some(SddPhase::Verify));
    }

    #[test]
    fn test_build_unchecked_skips_validation() {
        // Missing required fields — but build_unchecked succeeds
        let node = TicketBuilder::new("test.ticket.draft")
            .summary("wip")
            .build_unchecked()
            .unwrap();
        assert!(node.title.is_none());
    }

    #[test]
    fn test_build_unchecked_empty_id_returns_error() {
        let result = TicketBuilder::new("").build_unchecked();
        assert!(result.is_err());
        assert!(result.unwrap_err().is_precondition());
    }

    #[test]
    fn test_depends_and_related_to_stored() {
        // External references (depends/related_to) cannot be validated in single-node
        // mode because the referenced nodes don't exist in the scratch file.
        // Use build_unchecked for such pre-composition cases.
        let node = minimal_valid()
            .depends(["auth.constraints"])
            .related_to(["auth.session"])
            .build_unchecked()
            .unwrap();
        assert_eq!(node.depends.as_deref().unwrap(), ["auth.constraints"]);
        assert_eq!(node.related_to.as_deref().unwrap(), ["auth.session"]);
    }

    #[test]
    fn test_ticket_id_stored() {
        let node = minimal_valid().ticket_id("GH-42").build().unwrap();
        assert_eq!(node.ticket_id.as_deref(), Some("GH-42"));
    }

    #[test]
    fn test_assignee_stored() {
        let node = minimal_valid().assignee("agent-01").build().unwrap();
        assert_eq!(node.assignee.as_deref(), Some("agent-01"));
    }

    #[test]
    fn test_stability_stored() {
        let node = minimal_valid()
            .stability(Stability::Medium)
            .build()
            .unwrap();
        assert_eq!(node.stability, Some(Stability::Medium));
    }

    #[test]
    fn test_tags_stored() {
        let node = minimal_valid().tags(["rust", "backend"]).build().unwrap();
        assert_eq!(node.tags.as_deref().unwrap().len(), 2);
    }

    #[test]
    fn test_code_blocks_stored() {
        use crate::builder::code_block::CodeBlockBuilder;
        let cb = CodeBlockBuilder::create()
            .lang("rust")
            .target("src/auth.rs")
            .body("pub fn auth() {}")
            .build()
            .unwrap();
        let node = minimal_valid().code_blocks([cb]).build().unwrap();
        assert_eq!(node.code_blocks.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_prompt_stored() {
        let node = minimal_valid()
            .prompt("Implement full OAuth2 flow.")
            .build()
            .unwrap();
        assert!(node.prompt.is_some());
    }

    #[test]
    fn test_extra_field_stored_in_extra_fields_map() {
        use crate::model::fields::FieldValue;
        let node = minimal_valid()
            .extra("custom_label", FieldValue::Scalar("beta".to_owned()))
            .build()
            .unwrap();
        assert_eq!(
            node.extra_fields.get("custom_label"),
            Some(&FieldValue::Scalar("beta".to_owned()))
        );
    }
}
