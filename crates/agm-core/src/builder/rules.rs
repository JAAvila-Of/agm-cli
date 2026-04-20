//! Fluent builder for `NodeType::Rules` nodes (spec §13.2).

use std::collections::HashSet;

use crate::model::fields::{FieldValue, NodeType, Stability};
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
// RulesBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for `NodeType::Rules` nodes (spec §13.2).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::RulesBuilder;
///
/// let node = RulesBuilder::new("auth.rules")
///     .summary("authentication rules")
///     .items(["require HTTPS", "rate-limit login to 5 attempts/min"])
///     .build()
///     .expect("valid rules node");
///
/// assert_eq!(node.id, "auth.rules");
/// ```
#[derive(Debug, Clone)]
pub struct RulesBuilder {
    node: Node,
}

impl RulesBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Starts a new builder with the given node ID.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules");
    /// ```
    pub fn new<S: Into<String>>(id: S) -> Self {
        let node = Node {
            id: id.into(),
            node_type: NodeType::Rules,
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").summary("auth rules");
    /// ```
    pub fn summary<S: Into<String>>(mut self, s: S) -> Self {
        self.node.summary = s.into();
        self
    }

    /// Sets the `items` list (the rules themselves).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules")
    ///     .items(["require HTTPS", "rate-limit login"]);
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").detail("Background context.");
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
    /// use agm_core::builder::RulesBuilder;
    /// use agm_core::model::fields::Stability;
    ///
    /// let b = RulesBuilder::new("auth.rules").stability(Stability::High);
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").notes("Review quarterly.");
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").depends(["auth.constraints"]);
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").related_to(["auth.constraints"]);
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").replaces(["auth.old-rules"]);
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").conflicts(["auth.permissive-rules"]);
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").see_also(["auth.glossary"]);
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let b = RulesBuilder::new("auth.rules").tags(["security", "auth"]);
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
    /// use agm_core::builder::RulesBuilder;
    /// use agm_core::model::fields::FieldValue;
    ///
    /// let b = RulesBuilder::new("auth.rules")
    ///     .extra("extra_key", FieldValue::Scalar("v".to_owned()));
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let node = RulesBuilder::new("auth.rules")
    ///     .summary("auth rules")
    ///     .items(["require HTTPS"])
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(node.id, "auth.rules");
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
    /// use agm_core::builder::RulesBuilder;
    /// use agm_core::model::schema::EnforcementLevel;
    ///
    /// let node = RulesBuilder::new("auth.rules")
    ///     .summary("auth rules")
    ///     .items(["require HTTPS"])
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
    /// use agm_core::builder::RulesBuilder;
    ///
    /// let node = RulesBuilder::new("auth.draft").build_unchecked().unwrap();
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
    fn test_rules_minimal_build_succeeds() {
        // `items` is required for rules nodes (V024).
        let node = RulesBuilder::new("auth.rules")
            .summary("auth rules")
            .items(["require HTTPS"])
            .build()
            .unwrap();
        assert_eq!(node.id, "auth.rules");
        assert_eq!(node.node_type, NodeType::Rules);
    }

    #[test]
    fn test_rules_items_stored() {
        let node = RulesBuilder::new("auth.rules")
            .summary("rules")
            .items(["require HTTPS", "rate-limit"])
            .build()
            .unwrap();
        assert_eq!(node.items.as_deref().unwrap().len(), 2);
    }

    #[test]
    fn test_extra_field_stored_in_extra_fields_map() {
        use crate::model::fields::FieldValue;
        let node = RulesBuilder::new("auth.rules")
            .summary("rules")
            .items(["require HTTPS"])
            .extra("extra_key", FieldValue::Scalar("extra_val".to_owned()))
            .build()
            .unwrap();
        assert_eq!(
            node.extra_fields.get("extra_key"),
            Some(&FieldValue::Scalar("extra_val".to_owned()))
        );
    }
}
