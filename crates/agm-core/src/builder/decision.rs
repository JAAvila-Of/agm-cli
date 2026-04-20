//! Fluent builder for `NodeType::Decision` nodes (spec §13.5).

use std::collections::HashSet;

use crate::model::fields::{NodeType, Stability};
use crate::model::node::Node;
use crate::model::schema::EnforcementLevel;

use super::common::validate_single_node;
use super::error::BuildError;

/// Collects `items` into a `Vec<String>`, removing duplicates while preserving
/// first-appearance order. Applied to all list setters on this builder.
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
// DecisionBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for `NodeType::Decision` nodes (spec §13.5).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::DecisionBuilder;
///
/// let node = DecisionBuilder::new("arch.db-choice")
///     .summary("chose PostgreSQL over MongoDB")
///     .rationale(["ACID guarantees required", "existing team expertise"])
///     .build()
///     .expect("valid decision node");
///
/// assert_eq!(node.id, "arch.db-choice");
/// ```
#[derive(Debug, Clone)]
pub struct DecisionBuilder {
    node: Node,
}

impl DecisionBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Starts a new builder with the given node ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice");
    /// ```
    pub fn new<S: Into<String>>(id: S) -> Self {
        let node = Node {
            id: id.into(),
            node_type: NodeType::Decision,
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice").summary("chose PostgreSQL");
    /// ```
    pub fn summary<S: Into<String>>(mut self, s: S) -> Self {
        self.node.summary = s.into();
        self
    }

    /// Sets the `rationale` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice")
    ///     .rationale(["ACID guarantees required", "existing expertise"]);
    /// ```
    pub fn rationale<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.rationale = Some(items.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the `tradeoffs` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice")
    ///     .tradeoffs(["more complex scaling than NoSQL"]);
    /// ```
    pub fn tradeoffs<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.tradeoffs = Some(items.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the `resolution` list.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice")
    ///     .resolution(["use PostgreSQL 16", "enable pgvector extension"]);
    /// ```
    pub fn resolution<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.resolution = Some(items.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the `detail` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice").detail("Background context.");
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
    /// use agm_core::builder::DecisionBuilder;
    /// use agm_core::model::fields::Stability;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice").stability(Stability::High);
    /// ```
    pub fn stability(mut self, s: Stability) -> Self {
        self.node.stability = Some(s);
        self
    }

    /// Sets the `notes` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice").notes("Revisit in Q3.");
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice").depends(["arch.requirements"]);
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
    /// Duplicate entries are silently removed; cross-node references are validated
    /// at file level, not at single-node build time.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice")
    ///     .summary("chose PostgreSQL")
    ///     .rationale(["ACID required"])
    ///     .related_to(["arch.requirements", "arch.constraints"]);
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice")
    ///     .summary("chose PostgreSQL")
    ///     .rationale(["ACID required"])
    ///     .replaces(["arch.old-db-choice"]);
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice")
    ///     .summary("chose PostgreSQL")
    ///     .rationale(["ACID required"])
    ///     .conflicts(["arch.nosql-choice"]);
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice")
    ///     .summary("chose PostgreSQL")
    ///     .rationale(["ACID required"])
    ///     .see_also(["arch.data-model"]);
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let b = DecisionBuilder::new("arch.db-choice").tags(["architecture", "database"]);
    /// ```
    pub fn tags<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.tags = Some(dedup_preserve_order(items));
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let node = DecisionBuilder::new("arch.db-choice")
    ///     .summary("chose PostgreSQL over MongoDB")
    ///     .rationale(["ACID guarantees required"])
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "arch.db-choice");
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
    /// use agm_core::builder::DecisionBuilder;
    /// use agm_core::model::schema::EnforcementLevel;
    ///
    /// let node = DecisionBuilder::new("arch.db-choice")
    ///     .summary("chose PostgreSQL over MongoDB")
    ///     .rationale(["ACID guarantees required"])
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
    /// use agm_core::builder::DecisionBuilder;
    ///
    /// let node = DecisionBuilder::new("arch.draft").build_unchecked().unwrap();
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

    #[test]
    fn test_decision_minimal_build_succeeds() {
        // `rationale` is required for decision nodes (V024).
        let node = DecisionBuilder::new("arch.db-choice")
            .summary("chose PostgreSQL over MongoDB")
            .rationale(["ACID guarantees required"])
            .build()
            .unwrap();
        assert_eq!(node.id, "arch.db-choice");
        assert_eq!(node.node_type, NodeType::Decision);
    }

    #[test]
    fn test_decision_rationale_and_tradeoffs_stored() {
        let node = DecisionBuilder::new("arch.db-choice")
            .summary("chose PostgreSQL")
            .rationale(["ACID required"])
            .tradeoffs(["complex scaling"])
            .build()
            .unwrap();
        assert_eq!(node.rationale.as_deref().unwrap().len(), 1);
        assert_eq!(node.tradeoffs.as_deref().unwrap().len(), 1);
    }
}
