//! Builder for [`CodeBlock`] values.

use crate::model::code::{CodeAction, CodeBlock};

use super::error::BuildError;

// ---------------------------------------------------------------------------
// CodeBlockBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for [`CodeBlock`] values (spec §23).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::CodeBlockBuilder;
/// use agm_core::model::code::CodeAction;
///
/// let cb = CodeBlockBuilder::new(CodeAction::Create)
///     .lang("rust")
///     .target("src/lib.rs")
///     .body("pub fn hello() {}")
///     .build()
///     .unwrap();
///
/// assert_eq!(cb.action, CodeAction::Create);
/// assert_eq!(cb.lang.as_deref(), Some("rust"));
/// ```
#[derive(Debug, Clone)]
pub struct CodeBlockBuilder {
    action: CodeAction,
    lang: Option<String>,
    target: Option<String>,
    body: String,
    anchor: Option<String>,
    old: Option<String>,
}

impl CodeBlockBuilder {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Creates a new builder with the given action.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    /// use agm_core::model::code::CodeAction;
    ///
    /// let b = CodeBlockBuilder::new(CodeAction::Append);
    /// assert!(b.build().is_err()); // no target set yet for Append
    /// ```
    pub fn new(action: CodeAction) -> Self {
        Self {
            action,
            lang: None,
            target: None,
            body: String::new(),
            anchor: None,
            old: None,
        }
    }

    // -----------------------------------------------------------------------
    // Shortcut constructors
    // -----------------------------------------------------------------------

    /// Creates a builder with `action = Create`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let cb = CodeBlockBuilder::create()
    ///     .lang("rust")
    ///     .target("src/new_file.rs")
    ///     .body("// new file")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn create() -> Self {
        Self::new(CodeAction::Create)
    }

    /// Creates a builder with `action = Append`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let cb = CodeBlockBuilder::append()
    ///     .target("src/lib.rs")
    ///     .body("// appended")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn append() -> Self {
        Self::new(CodeAction::Append)
    }

    /// Creates a builder with `action = Prepend`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let cb = CodeBlockBuilder::prepend()
    ///     .target("src/lib.rs")
    ///     .body("// prepended")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn prepend() -> Self {
        Self::new(CodeAction::Prepend)
    }

    /// Creates a builder with `action = Replace`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let cb = CodeBlockBuilder::replace()
    ///     .target("src/lib.rs")
    ///     .old("fn old() {}")
    ///     .body("fn new_impl() {}")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn replace() -> Self {
        Self::new(CodeAction::Replace)
    }

    /// Creates a builder with `action = InsertBefore` and the given anchor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let cb = CodeBlockBuilder::insert_before("pub struct Foo {")
    ///     .target("src/lib.rs")
    ///     .body("// inserted before Foo")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn insert_before<S: Into<String>>(anchor: S) -> Self {
        Self::new(CodeAction::InsertBefore).anchor(anchor)
    }

    /// Creates a builder with `action = InsertAfter` and the given anchor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let cb = CodeBlockBuilder::insert_after("pub struct Foo {")
    ///     .target("src/lib.rs")
    ///     .body("    pub new_field: String,")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn insert_after<S: Into<String>>(anchor: S) -> Self {
        Self::new(CodeAction::InsertAfter).anchor(anchor)
    }

    /// Creates a builder with `action = Full`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let cb = CodeBlockBuilder::full()
    ///     .body("fn main() { println!(\"hello\"); }")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn full() -> Self {
        Self::new(CodeAction::Full)
    }

    // -----------------------------------------------------------------------
    // Setters
    // -----------------------------------------------------------------------

    /// Sets the language of the code block.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let b = CodeBlockBuilder::create().lang("rust");
    /// ```
    pub fn lang<S: Into<String>>(mut self, s: S) -> Self {
        self.lang = Some(s.into());
        self
    }

    /// Sets the target file path.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let b = CodeBlockBuilder::create().target("src/lib.rs");
    /// ```
    pub fn target<S: Into<String>>(mut self, s: S) -> Self {
        self.target = Some(s.into());
        self
    }

    /// Sets the code body.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let b = CodeBlockBuilder::full().body("fn main() {}");
    /// ```
    pub fn body<S: Into<String>>(mut self, s: S) -> Self {
        self.body = s.into();
        self
    }

    /// Sets the anchor text (used with `InsertBefore` and `InsertAfter` only).
    ///
    /// For `Replace` action, use [`old`](Self::old) instead. The spec (§23.4)
    /// requires `old` for Replace; `anchor` is semantically limited to insert
    /// actions and will cause `build()` to return a `BuildError::Precondition`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let b = CodeBlockBuilder::insert_before("pub struct Foo {")
    ///     .target("src/lib.rs")
    ///     .body("// inserted before Foo");
    /// ```
    pub fn anchor<S: Into<String>>(mut self, s: S) -> Self {
        self.anchor = Some(s.into());
        self
    }

    /// Sets the `old` text to match and replace (used with `Replace`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::CodeBlockBuilder;
    ///
    /// let b = CodeBlockBuilder::replace()
    ///     .target("src/lib.rs")
    ///     .old("fn old_fn() { todo!() }")
    ///     .body("fn old_fn() { /* done */ }");
    /// ```
    pub fn old<S: Into<String>>(mut self, s: S) -> Self {
        self.old = Some(s.into());
        self
    }

    // -----------------------------------------------------------------------
    // Terminal
    // -----------------------------------------------------------------------

    /// Builds the [`CodeBlock`], enforcing structural invariants (spec §23.4).
    ///
    /// Invariants checked:
    /// - Actions other than `Full` and `Create` require `target`.
    /// - `Replace` requires `old`; `anchor` is rejected (spec §23.4: Replace
    ///   matches by old text, not by anchor line).
    /// - `InsertBefore` / `InsertAfter` require `anchor`.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError::Precondition`] if an invariant is violated.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::{CodeBlockBuilder, BuildError};
    ///
    /// // Replace requires `old`, not `anchor`
    /// let err = CodeBlockBuilder::replace()
    ///     .target("src/lib.rs")
    ///     .anchor("fn x(")
    ///     .body("fn x() { /* new */ }")
    ///     .build()
    ///     .unwrap_err();
    /// assert!(err.is_precondition());
    /// ```
    pub fn build(self) -> Result<CodeBlock, BuildError> {
        // Actions that modify an existing file require a target.
        let requires_target = !matches!(self.action, CodeAction::Full | CodeAction::Create);
        if requires_target && self.target.is_none() {
            return Err(BuildError::Precondition(format!(
                "action `{}` requires a target file path",
                self.action
            )));
        }

        match self.action {
            CodeAction::Replace => {
                if self.anchor.is_some() {
                    return Err(BuildError::Precondition(
                        "Replace action does not accept `anchor`; use `old` to specify the text to replace (spec §23.4)".to_owned(),
                    ));
                }
                if self.old.is_none() {
                    return Err(BuildError::Precondition(
                        "Replace action requires `old` (spec §23.4)".to_owned(),
                    ));
                }
            }
            CodeAction::InsertBefore | CodeAction::InsertAfter => {
                if self.anchor.is_none() {
                    return Err(BuildError::Precondition(format!(
                        "action `{}` requires `anchor`",
                        self.action
                    )));
                }
            }
            _ => {}
        }

        Ok(CodeBlock {
            lang: self.lang,
            target: self.target,
            action: self.action,
            body: self.body,
            anchor: self.anchor,
            old: self.old,
        })
    }
}

// ---------------------------------------------------------------------------
// Ergonomic shortcuts on CodeBlock
// ---------------------------------------------------------------------------

impl CodeBlock {
    /// Returns a [`CodeBlockBuilder`] for the given action.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::model::code::{CodeBlock, CodeAction};
    ///
    /// let cb = CodeBlock::builder(CodeAction::Full)
    ///     .body("fn main() {}")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn builder(action: CodeAction) -> CodeBlockBuilder {
        CodeBlockBuilder::new(action)
    }

    /// Returns an [`CodeBlockBuilder`] pre-configured with `InsertBefore` and the given anchor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::model::code::CodeBlock;
    ///
    /// let cb = CodeBlock::insert_before("fn main(")
    ///     .target("src/main.rs")
    ///     .body("// inserted before main")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn insert_before<S: Into<String>>(anchor: S) -> CodeBlockBuilder {
        CodeBlockBuilder::insert_before(anchor)
    }

    /// Returns a [`CodeBlockBuilder`] pre-configured with `InsertAfter` and the given anchor.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::model::code::CodeBlock;
    ///
    /// let cb = CodeBlock::insert_after("pub struct Foo {")
    ///     .target("src/lib.rs")
    ///     .body("    pub extra: String,")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn insert_after<S: Into<String>>(anchor: S) -> CodeBlockBuilder {
        CodeBlockBuilder::insert_after(anchor)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::code::CodeAction;

    #[test]
    fn test_insert_before_sets_action_and_anchor() {
        let b = CodeBlockBuilder::insert_before("pub struct Foo {");
        let cb = b.target("src/lib.rs").body("// before").build().unwrap();
        assert_eq!(cb.action, CodeAction::InsertBefore);
        assert_eq!(cb.anchor.as_deref(), Some("pub struct Foo {"));
    }

    #[test]
    fn test_insert_after_sets_action_and_anchor() {
        let cb = CodeBlockBuilder::insert_after("fn start()")
            .target("src/lib.rs")
            .body("// after")
            .build()
            .unwrap();
        assert_eq!(cb.action, CodeAction::InsertAfter);
        assert_eq!(cb.anchor.as_deref(), Some("fn start()"));
    }

    #[test]
    fn test_replace_with_old_succeeds() {
        let cb = CodeBlockBuilder::replace()
            .target("src/lib.rs")
            .old("fn old() {}")
            .body("fn new() {}")
            .build()
            .unwrap();
        assert_eq!(cb.action, CodeAction::Replace);
        assert!(cb.old.is_some());
        assert!(cb.anchor.is_none());
    }

    #[test]
    fn test_replace_with_anchor_returns_precondition() {
        // Per spec §23.4, Replace uses `old` to match text; `anchor` is for
        // InsertBefore/InsertAfter only. Builder must enforce this.
        let err = CodeBlockBuilder::replace()
            .target("src/lib.rs")
            .anchor("fn old(")
            .body("fn new() {}")
            .build()
            .unwrap_err();
        assert!(
            err.is_precondition(),
            "Replace with anchor must return BuildError::Precondition"
        );
        assert!(
            err.to_string().contains("does not accept `anchor`"),
            "error message must mention 'does not accept `anchor`', got: {err}"
        );
    }

    #[test]
    fn test_replace_requires_old() {
        let err = CodeBlockBuilder::replace()
            .target("src/lib.rs")
            .body("fn new() {}")
            .build()
            .unwrap_err();
        assert!(err.is_precondition());
        assert!(
            err.to_string().contains("requires `old`"),
            "error must mention 'requires `old`', got: {err}"
        );
    }

    #[test]
    fn test_replace_with_both_old_and_anchor_errors() {
        // When both anchor and old are set, anchor is checked first (it's never
        // valid on Replace), so the message is "does not accept `anchor`".
        let err = CodeBlockBuilder::replace()
            .target("src/lib.rs")
            .anchor("fn x(")
            .old("fn x() {}")
            .body("fn x() { /* new */ }")
            .build()
            .unwrap_err();
        assert!(err.is_precondition());
        assert!(
            err.to_string().contains("does not accept `anchor`"),
            "error must mention 'does not accept `anchor`', got: {err}"
        );
    }

    #[test]
    fn test_append_without_target_errors() {
        let err = CodeBlockBuilder::append()
            .body("// extra")
            .build()
            .unwrap_err();
        assert!(err.is_precondition());
        assert!(err.to_string().contains("target"));
    }

    #[test]
    fn test_full_without_target_succeeds() {
        let cb = CodeBlockBuilder::full()
            .body("fn main() {}")
            .build()
            .unwrap();
        assert_eq!(cb.action, CodeAction::Full);
        assert!(cb.target.is_none());
    }

    #[test]
    fn test_create_without_target_succeeds() {
        let cb = CodeBlockBuilder::create()
            .body("// new file")
            .build()
            .unwrap();
        assert_eq!(cb.action, CodeAction::Create);
    }

    #[test]
    fn test_insert_before_without_anchor_errors() {
        // Using new() directly without an anchor set
        use crate::model::code::CodeAction;
        let err = CodeBlockBuilder::new(CodeAction::InsertBefore)
            .target("src/lib.rs")
            .body("// x")
            .build()
            .unwrap_err();
        assert!(err.is_precondition());
    }

    #[test]
    fn test_code_block_builder_shortcut_on_model() {
        let cb = CodeBlock::insert_before("fn main(")
            .target("src/main.rs")
            .body("// before main")
            .build()
            .unwrap();
        assert_eq!(cb.action, CodeAction::InsertBefore);
    }

    #[test]
    fn test_code_block_insert_after_shortcut_on_model() {
        let cb = CodeBlock::insert_after("fn start()")
            .target("src/lib.rs")
            .body("// after start")
            .build()
            .unwrap();
        assert_eq!(cb.action, CodeAction::InsertAfter);
    }

    #[test]
    fn test_code_block_builder_factory_on_model() {
        let cb = CodeBlock::builder(CodeAction::Full)
            .body("fn hello() {}")
            .build()
            .unwrap();
        assert_eq!(cb.action, CodeAction::Full);
    }
}
