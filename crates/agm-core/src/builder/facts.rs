//! Fluent builder for `NodeType::Facts` nodes (spec §13.1).

use std::collections::HashSet;

use crate::model::fields::{NodeType, Stability};
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
// FactsBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for `NodeType::Facts` nodes (spec §13.1).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::FactsBuilder;
///
/// let node = FactsBuilder::new("auth.constraints")
///     .summary("authentication policy constraints")
///     .items(["sessions expire after 24h", "MFA required for admin"])
///     .build()
///     .expect("valid facts node");
///
/// assert_eq!(node.id, "auth.constraints");
/// ```
#[derive(Debug, Clone)]
pub struct FactsBuilder {
    node: Node,
}

impl FactsBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Starts a new builder with the given node ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints");
    /// ```
    pub fn new<S: Into<String>>(id: S) -> Self {
        let node = Node {
            id: id.into(),
            node_type: NodeType::Facts,
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").summary("auth policy facts");
    /// ```
    pub fn summary<S: Into<String>>(mut self, s: S) -> Self {
        self.node.summary = s.into();
        self
    }

    /// Sets the `items` list (the primary payload of a facts node).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints")
    ///     .items(["sessions expire after 24h", "MFA required for admin"]);
    /// ```
    pub fn items<I, S>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.node.items = Some(items.into_iter().map(Into::into).collect());
        self
    }

    /// Sets the `detail` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").detail("See RFC-42.");
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
    /// use agm_core::builder::FactsBuilder;
    /// use agm_core::model::fields::Stability;
    ///
    /// let b = FactsBuilder::new("auth.constraints").stability(Stability::High);
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").notes("Updated 2026-04.");
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").depends(["auth.session"]);
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").related_to(["auth.rules"]);
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").replaces(["auth.old-constraints"]);
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").conflicts(["auth.permissive"]);
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").see_also(["auth.glossary"]);
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let b = FactsBuilder::new("auth.constraints").tags(["security", "auth"]);
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let node = FactsBuilder::new("auth.constraints")
    ///     .summary("auth policy facts")
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "auth.constraints");
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
    /// use agm_core::builder::FactsBuilder;
    /// use agm_core::model::schema::EnforcementLevel;
    ///
    /// let node = FactsBuilder::new("auth.constraints")
    ///     .summary("auth policy facts")
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
    /// use agm_core::builder::FactsBuilder;
    ///
    /// let node = FactsBuilder::new("auth.draft").build_unchecked().unwrap();
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
    fn test_facts_minimal_build_succeeds() {
        let node = FactsBuilder::new("auth.constraints")
            .summary("auth constraints")
            .build()
            .unwrap();
        assert_eq!(node.id, "auth.constraints");
        assert_eq!(node.node_type, NodeType::Facts);
    }

    #[test]
    fn test_facts_items_stored() {
        let node = FactsBuilder::new("auth.constraints")
            .summary("facts")
            .items(["sessions expire", "MFA required"])
            .build()
            .unwrap();
        assert_eq!(node.items.as_deref().unwrap().len(), 2);
    }

    #[test]
    fn test_facts_tags_stored() {
        let node = FactsBuilder::new("auth.constraints")
            .summary("facts")
            .tags(["security"])
            .build()
            .unwrap();
        assert_eq!(node.tags.as_deref().unwrap(), ["security"]);
    }
}
