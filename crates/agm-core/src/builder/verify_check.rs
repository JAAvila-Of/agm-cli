//! Builder for [`VerifyCheck`] values.

use crate::model::verify::VerifyCheck;

use super::error::BuildError;

// ---------------------------------------------------------------------------
// VerifyCheckBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for [`VerifyCheck`] values (spec §24).
///
/// # Examples
///
/// ```rust
/// use agm_core::builder::VerifyCheckBuilder;
///
/// let check = VerifyCheckBuilder::command("cargo test")
///     .expect("exit_code_0")
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct VerifyCheckBuilder {
    kind: VerifyKind,
}

#[derive(Debug, Clone)]
enum VerifyKind {
    Command { run: String, expect: Option<String> },
    FileExists { file: String },
    FileContains { file: String, pattern: String },
    FileNotContains { file: String, pattern: String },
    NodeStatus { node: String, status: String },
}

impl VerifyCheckBuilder {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Returns a builder pre-configured for a `Command` check.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::VerifyCheckBuilder;
    ///
    /// let check = VerifyCheckBuilder::command("cargo check").build().unwrap();
    /// ```
    pub fn command<S: Into<String>>(run: S) -> Self {
        Self {
            kind: VerifyKind::Command {
                run: run.into(),
                expect: None,
            },
        }
    }

    /// Returns a builder pre-configured for a `FileExists` check.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::VerifyCheckBuilder;
    ///
    /// let check = VerifyCheckBuilder::file_exists("src/lib.rs").build().unwrap();
    /// ```
    pub fn file_exists<S: Into<String>>(file: S) -> Self {
        Self {
            kind: VerifyKind::FileExists { file: file.into() },
        }
    }

    /// Returns a builder pre-configured for a `FileContains` check.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::VerifyCheckBuilder;
    ///
    /// let check = VerifyCheckBuilder::file_contains("src/lib.rs", "fn hello").build().unwrap();
    /// ```
    pub fn file_contains<S: Into<String>, P: Into<String>>(file: S, pattern: P) -> Self {
        Self {
            kind: VerifyKind::FileContains {
                file: file.into(),
                pattern: pattern.into(),
            },
        }
    }

    /// Returns a builder pre-configured for a `FileNotContains` check.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::VerifyCheckBuilder;
    ///
    /// let check = VerifyCheckBuilder::file_not_contains("src/lib.rs", "unsafe").build().unwrap();
    /// ```
    pub fn file_not_contains<S: Into<String>, P: Into<String>>(file: S, pattern: P) -> Self {
        Self {
            kind: VerifyKind::FileNotContains {
                file: file.into(),
                pattern: pattern.into(),
            },
        }
    }

    /// Returns a builder pre-configured for a `NodeStatus` check.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::VerifyCheckBuilder;
    ///
    /// let check = VerifyCheckBuilder::node_status("auth.login", "completed").build().unwrap();
    /// ```
    pub fn node_status<S: Into<String>, T: Into<String>>(node: S, status: T) -> Self {
        Self {
            kind: VerifyKind::NodeStatus {
                node: node.into(),
                status: status.into(),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Optional setters
    // -----------------------------------------------------------------------

    /// Sets the expected output for a `Command` check.
    ///
    /// Has no effect on non-`Command` builders.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::VerifyCheckBuilder;
    ///
    /// let check = VerifyCheckBuilder::command("cargo test")
    ///     .expect("exit_code_0")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn expect<S: Into<String>>(mut self, val: S) -> Self {
        if let VerifyKind::Command { ref mut expect, .. } = self.kind {
            *expect = Some(val.into());
        }
        self
    }

    // -----------------------------------------------------------------------
    // Terminal
    // -----------------------------------------------------------------------

    /// Builds the [`VerifyCheck`].
    ///
    /// Always succeeds — the factory constructors ensure a valid kind is set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::builder::VerifyCheckBuilder;
    ///
    /// let check = VerifyCheckBuilder::command("cargo fmt --check")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn build(self) -> Result<VerifyCheck, BuildError> {
        match self.kind {
            VerifyKind::Command { run, expect } => Ok(VerifyCheck::Command { run, expect }),
            VerifyKind::FileExists { file } => Ok(VerifyCheck::FileExists { file }),
            VerifyKind::FileContains { file, pattern } => {
                Ok(VerifyCheck::FileContains { file, pattern })
            }
            VerifyKind::FileNotContains { file, pattern } => {
                Ok(VerifyCheck::FileNotContains { file, pattern })
            }
            VerifyKind::NodeStatus { node, status } => Ok(VerifyCheck::NodeStatus { node, status }),
        }
    }
}

// ---------------------------------------------------------------------------
// Ergonomic shortcuts on VerifyCheck
// ---------------------------------------------------------------------------

impl VerifyCheck {
    /// Returns a [`VerifyCheckBuilder`] pre-configured for a `Command` check.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use agm_core::model::verify::VerifyCheck;
    ///
    /// let check = VerifyCheck::command_builder("cargo test")
    ///     .expect("exit_code_0")
    ///     .build()
    ///     .unwrap();
    /// ```
    pub fn command_builder<S: Into<String>>(run: S) -> VerifyCheckBuilder {
        VerifyCheckBuilder::command(run)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_builder_no_expect() {
        let check = VerifyCheckBuilder::command("cargo check").build().unwrap();
        assert!(matches!(check, VerifyCheck::Command { expect: None, .. }));
    }

    #[test]
    fn test_command_builder_with_expect() {
        let check = VerifyCheckBuilder::command("cargo test")
            .expect("exit_code_0")
            .build()
            .unwrap();
        match check {
            VerifyCheck::Command { run, expect } => {
                assert_eq!(run, "cargo test");
                assert_eq!(expect.as_deref(), Some("exit_code_0"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_file_exists_builder() {
        let check = VerifyCheckBuilder::file_exists("src/lib.rs")
            .build()
            .unwrap();
        assert!(matches!(check, VerifyCheck::FileExists { .. }));
    }

    #[test]
    fn test_file_contains_builder() {
        let check = VerifyCheckBuilder::file_contains("src/lib.rs", "fn main")
            .build()
            .unwrap();
        match check {
            VerifyCheck::FileContains { file, pattern } => {
                assert_eq!(file, "src/lib.rs");
                assert_eq!(pattern, "fn main");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_file_not_contains_builder() {
        let check = VerifyCheckBuilder::file_not_contains("src/lib.rs", "unsafe")
            .build()
            .unwrap();
        assert!(matches!(check, VerifyCheck::FileNotContains { .. }));
    }

    #[test]
    fn test_node_status_builder() {
        let check = VerifyCheckBuilder::node_status("auth.login", "completed")
            .build()
            .unwrap();
        match check {
            VerifyCheck::NodeStatus { node, status } => {
                assert_eq!(node, "auth.login");
                assert_eq!(status, "completed");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_verify_check_command_shortcut_on_model() {
        let check = VerifyCheck::command_builder("cargo fmt --check")
            .build()
            .unwrap();
        assert!(matches!(check, VerifyCheck::Command { .. }));
    }
}
