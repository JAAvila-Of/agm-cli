//! Fluent builder for `NodeType::Workflow` nodes (spec §13.3).

use std::collections::HashSet;

use crate::model::code::CodeBlock;
use crate::model::context::AgentContext;
use crate::model::fields::{FieldValue, NodeType, Priority, Stability};
use crate::model::node::Node;
use crate::model::schema::EnforcementLevel;
use crate::model::verify::VerifyCheck;

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
// WorkflowBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for `NodeType::Workflow` nodes (spec §13.3).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::WorkflowBuilder;
///
/// let node = WorkflowBuilder::new("auth.login")
///     .summary("resolve tenant → redirect → callback → create sid")
///     .steps(["resolve tenant", "redirect to provider", "handle callback"])
///     .input(["host", "return_url"])
///     .output(["redirect_url", "sid_cookie"])
///     .build()
///     .expect("valid workflow");
///
/// assert_eq!(node.id, "auth.login");
/// ```
#[derive(Debug, Clone)]
pub struct WorkflowBuilder {
    node: Node,
}

impl WorkflowBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Starts a new builder with the given node ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login");
    /// ```
    pub fn new<S: Into<String>>(id: S) -> Self {
        let node = Node {
            id: id.into(),
            node_type: NodeType::Workflow,
            ..Default::default()
        };
        Self { node }
    }

    // -----------------------------------------------------------------------
    // Setters
    // -----------------------------------------------------------------------

    /// Sets the one-line `summary` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").summary("authenticate user");
    /// ```
    pub fn summary<S: Into<String>>(mut self, s: S) -> Self {
        self.node.summary = s.into();
        self
    }

    /// Sets the `steps` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login")
    ///     .steps(["resolve tenant", "redirect to provider"]);
    /// ```
    pub fn steps<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        // Steps are ordered and may intentionally repeat; no dedup.
        self.node.steps = Some(items.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the `input` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").input(["host", "return_url"]);
    /// ```
    pub fn input<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.input = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the `output` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").output(["redirect_url", "sid_cookie"]);
    /// ```
    pub fn output<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.output = Some(dedup_preserve_order(items));
        self
    }

    /// Sets the single `code` field (primary code block).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    /// use agm_core::model::code::{CodeBlock, CodeAction};
    ///
    /// let cb = CodeBlock { action: CodeAction::Full, body: "fn run() {}".to_owned(),
    ///     lang: None, target: None, anchor: None, old: None };
    /// let b = WorkflowBuilder::new("auth.login").code(cb);
    /// ```
    pub fn code(mut self, cb: CodeBlock) -> Self {
        self.node.code = Some(cb);
        self
    }

    /// Sets the `code_blocks` list.
    ///
    /// Callers should call `.build()?` on each [`CodeBlockBuilder`] before
    /// passing the values here.
    ///
    /// [`CodeBlockBuilder`]: crate::builder::CodeBlockBuilder
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::{WorkflowBuilder, CodeBlockBuilder};
    ///
    /// let cb = CodeBlockBuilder::create()
    ///     .target("src/auth.rs")
    ///     .body("pub fn auth() {}")
    ///     .build()
    ///     .unwrap();
    ///
    /// let b = WorkflowBuilder::new("auth.login").code_blocks([cb]);
    /// ```
    pub fn code_blocks<I>(mut self, blocks: I) -> Self
    where
        I: IntoIterator<Item = CodeBlock>,
    {
        self.node.code_blocks = Some(blocks.into_iter().collect());
        self
    }

    /// Sets the `verify` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::{WorkflowBuilder, VerifyCheckBuilder};
    ///
    /// let check = VerifyCheckBuilder::command("cargo test").build().unwrap();
    /// let b = WorkflowBuilder::new("auth.login").verify([check]);
    /// ```
    pub fn verify<I>(mut self, checks: I) -> Self
    where
        I: IntoIterator<Item = VerifyCheck>,
    {
        self.node.verify = Some(checks.into_iter().collect());
        self
    }

    /// Sets the `agent_context` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    /// use agm_core::model::context::AgentContext;
    ///
    /// let ctx = AgentContext {
    ///     load_nodes: Some(vec!["auth.constraints".to_owned()]),
    ///     load_files: None,
    ///     system_hint: None,
    ///     max_tokens: None,
    ///     load_memory: None,
    /// };
    /// let b = WorkflowBuilder::new("auth.login").agent_context(ctx);
    /// ```
    pub fn agent_context(mut self, ctx: AgentContext) -> Self {
        self.node.agent_context = Some(ctx);
        self
    }

    /// Sets the `target` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").target("src/handlers/auth.rs");
    /// ```
    pub fn target<S: Into<String>>(mut self, s: S) -> Self {
        self.node.target = Some(s.into());
        self
    }

    /// Sets the `priority` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    /// use agm_core::model::fields::Priority;
    ///
    /// let b = WorkflowBuilder::new("auth.login").priority(Priority::High);
    /// ```
    pub fn priority(mut self, p: Priority) -> Self {
        self.node.priority = Some(p);
        self
    }

    /// Sets the `stability` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    /// use agm_core::model::fields::Stability;
    ///
    /// let b = WorkflowBuilder::new("auth.login").stability(Stability::Medium);
    /// ```
    pub fn stability(mut self, s: Stability) -> Self {
        self.node.stability = Some(s);
        self
    }

    /// Sets the `detail` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").detail("Extra implementation notes.");
    /// ```
    pub fn detail<S: Into<String>>(mut self, s: S) -> Self {
        self.node.detail = Some(s.into());
        self
    }

    /// Sets the `notes` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").notes("See RFC-42.");
    /// ```
    pub fn notes<S: Into<String>>(mut self, s: S) -> Self {
        self.node.notes = Some(s.into());
        self
    }

    /// Sets the `depends` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").depends(["auth.constraints"]);
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
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").related_to(["auth.session"]);
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
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").replaces(["auth.old-login"]);
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
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").conflicts(["auth.basic-login"]);
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
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").see_also(["auth.session"]);
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
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let b = WorkflowBuilder::new("auth.login").tags(["auth", "backend"]);
    /// ```
    pub fn tags<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.tags = Some(dedup_preserve_order(items));
        self
    }

    /// Inserts an arbitrary field into the node's `extra_fields` map.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    /// use agm_core::model::fields::FieldValue;
    ///
    /// let b = WorkflowBuilder::new("auth.login")
    ///     .extra("custom_key", FieldValue::Scalar("val".to_owned()));
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
    /// # Errors
    ///
    /// Returns [`BuildError::Validation`] if the validator finds any errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let node = WorkflowBuilder::new("auth.login")
    ///     .summary("authenticate and create session")
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "auth.login");
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
    /// use agm_core::builder::WorkflowBuilder;
    /// use agm_core::model::schema::EnforcementLevel;
    ///
    /// let node = WorkflowBuilder::new("auth.login")
    ///     .summary("authenticate")
    ///     .build_with(EnforcementLevel::Standard)
    ///     .unwrap();
    /// ```
    pub fn build_with(self, level: EnforcementLevel) -> Result<Node, BuildError> {
        validate_single_node(self.node, level)
    }

    /// Builds without running the validator.
    ///
    /// # Errors
    ///
    /// Returns `Err` only if the node ID is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::WorkflowBuilder;
    ///
    /// let node = WorkflowBuilder::new("auth.draft")
    ///     .build_unchecked()
    ///     .unwrap();
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
    use crate::builder::verify_check::VerifyCheckBuilder;
    use crate::model::code::{CodeAction, CodeBlock};

    fn minimal_valid() -> WorkflowBuilder {
        WorkflowBuilder::new("auth.login").summary("authenticate user")
    }

    #[test]
    fn test_minimal_build_produces_valid_node() {
        let node = minimal_valid().build().unwrap();
        assert_eq!(node.id, "auth.login");
        assert_eq!(node.node_type, NodeType::Workflow);
    }

    #[test]
    fn test_steps_list_preserved() {
        let node = minimal_valid()
            .steps(["resolve tenant", "redirect", "callback"])
            .build()
            .unwrap();
        assert_eq!(node.steps.as_deref().unwrap().len(), 3);
    }

    #[test]
    fn test_code_blocks_preserved() {
        let cb = CodeBlock {
            lang: Some("rust".to_owned()),
            target: Some("src/auth.rs".to_owned()),
            action: CodeAction::Create,
            body: "fn auth() {}".to_owned(),
            anchor: None,
            old: None,
        };
        let node = minimal_valid().code_blocks([cb]).build().unwrap();
        assert_eq!(node.code_blocks.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_verify_block_preserved() {
        let check = VerifyCheckBuilder::command("cargo test").build().unwrap();
        let node = minimal_valid().verify([check]).build().unwrap();
        assert_eq!(node.verify.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_input_output_stored() {
        let node = minimal_valid()
            .input(["host", "return_url"])
            .output(["redirect_url"])
            .build()
            .unwrap();
        assert_eq!(node.input.as_deref().unwrap().len(), 2);
        assert_eq!(node.output.as_deref().unwrap().len(), 1);
    }

    #[test]
    fn test_extra_field_stored_in_extra_fields_map() {
        use crate::model::fields::FieldValue;
        let node = minimal_valid()
            .extra("custom_key", FieldValue::Scalar("some_value".to_owned()))
            .build()
            .unwrap();
        assert_eq!(
            node.extra_fields.get("custom_key"),
            Some(&FieldValue::Scalar("some_value".to_owned()))
        );
    }
}
