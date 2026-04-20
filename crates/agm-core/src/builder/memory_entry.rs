//! Fluent builder for nodes that carry `memory` entries (spec §13, §28).
//!
//! `MemoryEntryBuilder` builds a single [`MemoryEntry`], not a full node.
//! To attach memory entries to a node, use the node builder's `.memory()` setter
//! (e.g. on `WorkflowBuilder`).

use crate::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};

use super::error::BuildError;

// ---------------------------------------------------------------------------
// MemoryEntryBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for [`MemoryEntry`] values (spec §28).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::MemoryEntryBuilder;
/// use agm_core::model::memory::{MemoryAction, MemoryScope, MemoryTtl};
///
/// let entry = MemoryEntryBuilder::new("repo.pattern", "rust.repository", MemoryAction::Upsert)
///     .value("row_to_column uses get()")
///     .scope(MemoryScope::Project)
///     .ttl(MemoryTtl::Permanent)
///     .build()
///     .unwrap();
///
/// assert_eq!(entry.key, "repo.pattern");
/// assert_eq!(entry.action, MemoryAction::Upsert);
/// ```
#[derive(Debug, Clone)]
pub struct MemoryEntryBuilder {
    key: String,
    topic: String,
    action: MemoryAction,
    value: Option<String>,
    scope: Option<MemoryScope>,
    ttl: Option<MemoryTtl>,
    query: Option<String>,
    max_results: Option<u32>,
}

impl MemoryEntryBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Creates a new builder with the required fields: `key`, `topic`, and `action`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::MemoryEntryBuilder;
    /// use agm_core::model::memory::MemoryAction;
    ///
    /// let b = MemoryEntryBuilder::new("key", "topic", MemoryAction::Get);
    /// ```
    pub fn new<K: Into<String>, T: Into<String>>(key: K, topic: T, action: MemoryAction) -> Self {
        Self {
            key: key.into(),
            topic: topic.into(),
            action,
            value: None,
            scope: None,
            ttl: None,
            query: None,
            max_results: None,
        }
    }

    // -----------------------------------------------------------------------
    // Setters
    // -----------------------------------------------------------------------

    /// Sets the `value` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::MemoryEntryBuilder;
    /// use agm_core::model::memory::MemoryAction;
    ///
    /// let b = MemoryEntryBuilder::new("k", "t", MemoryAction::Upsert)
    ///     .value("some stored value");
    /// ```
    pub fn value<S: Into<String>>(mut self, s: S) -> Self {
        self.value = Some(s.into());
        self
    }

    /// Sets the `scope` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::MemoryEntryBuilder;
    /// use agm_core::model::memory::{MemoryAction, MemoryScope};
    ///
    /// let b = MemoryEntryBuilder::new("k", "t", MemoryAction::Upsert)
    ///     .scope(MemoryScope::Project);
    /// ```
    pub fn scope(mut self, s: MemoryScope) -> Self {
        self.scope = Some(s);
        self
    }

    /// Sets the `ttl` field.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::MemoryEntryBuilder;
    /// use agm_core::model::memory::{MemoryAction, MemoryTtl};
    ///
    /// let b = MemoryEntryBuilder::new("k", "t", MemoryAction::Upsert)
    ///     .ttl(MemoryTtl::Permanent);
    /// ```
    pub fn ttl(mut self, t: MemoryTtl) -> Self {
        self.ttl = Some(t);
        self
    }

    /// Sets the `query` field (used with `Search` action).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::MemoryEntryBuilder;
    /// use agm_core::model::memory::MemoryAction;
    ///
    /// let b = MemoryEntryBuilder::new("k", "t", MemoryAction::Search)
    ///     .query("how are optional fields handled");
    /// ```
    pub fn query<S: Into<String>>(mut self, s: S) -> Self {
        self.query = Some(s.into());
        self
    }

    /// Sets the `max_results` field (used with `Search` action).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::MemoryEntryBuilder;
    /// use agm_core::model::memory::MemoryAction;
    ///
    /// let b = MemoryEntryBuilder::new("k", "t", MemoryAction::Search)
    ///     .max_results(5);
    /// ```
    pub fn max_results(mut self, n: u32) -> Self {
        self.max_results = Some(n);
        self
    }

    // -----------------------------------------------------------------------
    // Terminal
    // -----------------------------------------------------------------------

    /// Builds the [`MemoryEntry`].
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Precondition`] if `key` or `topic` is empty.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::MemoryEntryBuilder;
    /// use agm_core::model::memory::MemoryAction;
    ///
    /// let entry = MemoryEntryBuilder::new("repo.pattern", "rust.repo", MemoryAction::Upsert)
    ///     .value("use get() for optionals")
    ///     .build()
    ///     .unwrap();
    ///
    /// assert_eq!(entry.key, "repo.pattern");
    /// ```
    pub fn build(self) -> Result<MemoryEntry, BuildError> {
        if self.key.is_empty() {
            return Err(BuildError::Precondition(
                "memory entry key must not be empty".to_owned(),
            ));
        }
        if self.topic.is_empty() {
            return Err(BuildError::Precondition(
                "memory entry topic must not be empty".to_owned(),
            ));
        }
        Ok(MemoryEntry {
            key: self.key,
            topic: self.topic,
            action: self.action,
            value: self.value,
            scope: self.scope,
            ttl: self.ttl,
            query: self.query,
            max_results: self.max_results,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::memory::{MemoryAction, MemoryScope, MemoryTtl};

    #[test]
    fn test_memory_entry_upsert_build_succeeds() {
        let entry =
            MemoryEntryBuilder::new("repo.pattern", "rust.repository", MemoryAction::Upsert)
                .value("row_to_column uses get()")
                .scope(MemoryScope::Project)
                .ttl(MemoryTtl::Permanent)
                .build()
                .unwrap();
        assert_eq!(entry.key, "repo.pattern");
        assert_eq!(entry.action, MemoryAction::Upsert);
        assert!(entry.value.is_some());
        assert!(entry.scope.is_some());
        assert!(entry.ttl.is_some());
    }

    #[test]
    fn test_memory_entry_search_build_succeeds() {
        let entry = MemoryEntryBuilder::new("search.key", "rust.repository", MemoryAction::Search)
            .query("how are optionals handled")
            .max_results(5)
            .build()
            .unwrap();
        assert_eq!(entry.query.as_deref(), Some("how are optionals handled"));
        assert_eq!(entry.max_results, Some(5));
    }

    #[test]
    fn test_empty_key_returns_precondition_error() {
        let result = MemoryEntryBuilder::new("", "topic", MemoryAction::Get).build();
        assert!(result.is_err());
        assert!(result.unwrap_err().is_precondition());
    }

    #[test]
    fn test_empty_topic_returns_precondition_error() {
        let result = MemoryEntryBuilder::new("key", "", MemoryAction::Get).build();
        assert!(result.is_err());
        assert!(result.unwrap_err().is_precondition());
    }
}
