//! Fluent builder for `NodeType::Orchestration` nodes (spec §13.10).

use std::collections::HashSet;

use crate::model::fields::{FieldValue, NodeType, Stability};
use crate::model::node::Node;
use crate::model::orchestration::ParallelGroup;
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
// OrchestrationBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for `NodeType::Orchestration` nodes (spec §13.10).
///
/// At least one `ParallelGroup` is required (enforced by the validator).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::OrchestrationBuilder;
/// use agm_core::model::orchestration::{ParallelGroup, Strategy};
///
/// // Group nodes must reference IDs in the same file. In single-node
/// // validation, the orchestration node's own ID is the only valid reference.
/// let group = ParallelGroup {
///     group: "1-schema".to_owned(),
///     nodes: vec!["deploy.orchestration".to_owned()], // self-reference for doc-test
///     strategy: Strategy::Sequential,
///     requires: None,
///     max_concurrency: None,
/// };
///
/// let node = OrchestrationBuilder::new("deploy.orchestration")
///     .summary("orchestrate the full deployment")
///     .parallel_groups([group])
///     .build()
///     .expect("valid orchestration");
///
/// assert_eq!(node.id, "deploy.orchestration");
/// ```
#[derive(Debug, Clone)]
pub struct OrchestrationBuilder {
    node: Node,
}

impl OrchestrationBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Starts a new builder with the given node ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("deploy.orchestration");
    /// ```
    pub fn new<S: Into<String>>(id: S) -> Self {
        let node = Node {
            id: id.into(),
            node_type: NodeType::Orchestration,
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("deploy.orchestration")
    ///     .summary("orchestrate the deployment pipeline");
    /// ```
    pub fn summary<S: Into<String>>(mut self, s: S) -> Self {
        self.node.summary = s.into();
        self
    }

    /// Sets the `parallel_groups` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::OrchestrationBuilder;
    /// use agm_core::model::orchestration::{ParallelGroup, Strategy};
    ///
    /// let group = ParallelGroup {
    ///     group: "1-schema".to_owned(),
    ///     nodes: vec!["o.x".to_owned()], // self-reference so single-node validation passes
    ///     strategy: Strategy::Sequential,
    ///     requires: None,
    ///     max_concurrency: None,
    /// };
    /// let b = OrchestrationBuilder::new("o.x").parallel_groups([group]);
    /// ```
    pub fn parallel_groups<I>(mut self, groups: I) -> Self
    where
        I: IntoIterator<Item = ParallelGroup>,
    {
        self.node.parallel_groups = Some(groups.into_iter().collect());
        self
    }

    /// Sets the `stability` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::OrchestrationBuilder;
    /// use agm_core::model::fields::Stability;
    ///
    /// let b = OrchestrationBuilder::new("o.x").stability(Stability::High);
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").detail("Deployment notes.");
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").notes("Run in CI only.");
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").depends(["migration.schema"]);
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
    /// Conceptual or semantic relationships without strong dependency (spec §15.2).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").related_to(["deploy.plan"]);
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").replaces(["deploy.old-orch"]);
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").conflicts(["deploy.manual-orch"]);
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").see_also(["deploy.runbook"]);
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let b = OrchestrationBuilder::new("o.x").tags(["deploy", "ci"]);
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
    /// use agm_core::builder::OrchestrationBuilder;
    /// use agm_core::model::fields::FieldValue;
    ///
    /// let b = OrchestrationBuilder::new("o.x")
    ///     .extra("meta_key", FieldValue::Scalar("meta_val".to_owned()));
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
    /// The validator will error (V024) if `parallel_groups` is absent.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Validation`] if the validator finds any errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::OrchestrationBuilder;
    /// use agm_core::model::orchestration::{ParallelGroup, Strategy};
    ///
    /// let group = ParallelGroup {
    ///     group: "g1".to_owned(),
    ///     nodes: vec!["o.test".to_owned()], // self-reference so single-node validation passes
    ///     strategy: Strategy::Sequential,
    ///     requires: None,
    ///     max_concurrency: None,
    /// };
    ///
    /// let node = OrchestrationBuilder::new("o.test")
    ///     .summary("test orchestration")
    ///     .parallel_groups([group])
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "o.test");
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
    /// use agm_core::builder::OrchestrationBuilder;
    /// use agm_core::model::orchestration::{ParallelGroup, Strategy};
    /// use agm_core::model::schema::EnforcementLevel;
    ///
    /// let group = ParallelGroup {
    ///     group: "g1".to_owned(),
    ///     nodes: vec!["o.test".to_owned()], // self-reference so single-node validation passes
    ///     strategy: Strategy::Sequential,
    ///     requires: None,
    ///     max_concurrency: None,
    /// };
    ///
    /// let node = OrchestrationBuilder::new("o.test")
    ///     .summary("test orchestration")
    ///     .parallel_groups([group])
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
    /// use agm_core::builder::OrchestrationBuilder;
    ///
    /// let node = OrchestrationBuilder::new("o.draft")
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
    use crate::model::orchestration::{ParallelGroup, Strategy};

    /// Creates a sample group whose `nodes` list references the orchestration node's own ID
    /// so the single-node validation file's `all_ids` set contains the reference.
    fn sample_group(name: &str) -> ParallelGroup {
        ParallelGroup {
            group: name.to_owned(),
            nodes: vec!["o.test".to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        }
    }

    fn sample_group_for(name: &str, node_id: &str) -> ParallelGroup {
        ParallelGroup {
            group: name.to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        }
    }

    #[test]
    fn test_requires_parallel_groups() {
        // Without parallel_groups validation should fail (V024)
        let result = OrchestrationBuilder::new("o.test")
            .summary("orchestrate")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_missing_groups_validation_error() {
        // parallel_groups = None — V018 fires (orchestration node missing parallel_groups)
        let result = OrchestrationBuilder::new("o.test")
            .summary("orchestrate")
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().is_validation());
    }

    #[test]
    fn test_empty_groups_list_build_unchecked_succeeds() {
        // parallel_groups = Some([]) — validator does not fire for empty list (only for None)
        // Use build_unchecked for composition into a multi-node file.
        let node = OrchestrationBuilder::new("o.draft")
            .parallel_groups(std::iter::empty::<ParallelGroup>())
            .build_unchecked()
            .unwrap();
        assert_eq!(node.parallel_groups.as_ref().unwrap().len(), 0);
    }

    #[test]
    fn test_valid_with_groups_succeeds() {
        let node = OrchestrationBuilder::new("o.test")
            .summary("orchestrate deployment")
            .parallel_groups([sample_group("g1"), sample_group("g2")])
            .build()
            .unwrap();
        assert_eq!(node.parallel_groups.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_extra_field_stored_in_extra_fields_map() {
        use crate::model::fields::FieldValue;
        let node = OrchestrationBuilder::new("o.test")
            .summary("orchestrate")
            .parallel_groups([sample_group("g1")])
            .extra("meta_key", FieldValue::Scalar("meta_val".to_owned()))
            .build()
            .unwrap();
        assert_eq!(
            node.extra_fields.get("meta_key"),
            Some(&FieldValue::Scalar("meta_val".to_owned()))
        );
    }

    #[test]
    fn test_three_groups() {
        // Three groups, each referencing the orchestration node's own ID (so
        // single-node validation's all_ids contains the reference).
        let node = OrchestrationBuilder::new("o.three")
            .summary("three groups")
            .parallel_groups([
                sample_group_for("1-schema", "o.three"),
                sample_group_for("2-models", "o.three"),
                sample_group_for("3-backend", "o.three"),
            ])
            .build()
            .unwrap();
        assert_eq!(node.parallel_groups.as_ref().unwrap().len(), 3);
    }
}
