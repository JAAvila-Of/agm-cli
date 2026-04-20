//! Built-in corpora for AGM system prompts.
//!
//! Provides three flavors of corpus template (`Full`, `Standard`,
//! `GrammarOnly`) and three provider targets (`Anthropic`, `OpenAi`,
//! `Vanilla`). Templates and example bank files are embedded at compile time
//! via `include_str!`.
//!
//! # Usage
//!
//! ```rust,ignore
//! use agm_cli::corpora::{CorpusOptions, CorpusFlavor, CorpusTarget, render_corpus};
//!
//! let opts = CorpusOptions {
//!     flavor: CorpusFlavor::Full,
//!     target: CorpusTarget::Anthropic,
//!     min_tokens: Some(2048),
//!     include_version: true,
//! };
//! let output = render_corpus(&opts)?;
//! println!("{}", output.body);
//! ```

pub mod tokens;
pub mod wrap;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Corpus content flavor controlling template depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusFlavor {
    /// Full grammar summary, semantic notes, and 6 varied examples.
    Full,
    /// Abridged grammar and 3 examples.
    Standard,
    /// Grammar summary only with 1 minimal example.
    GrammarOnly,
}

/// Provider target controlling preamble wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusTarget {
    /// Anthropic system prompt format — role preamble prepended.
    Anthropic,
    /// OpenAI system prompt format — tool-preference preamble prepended.
    OpenAi,
    /// No wrapping — emit the raw corpus body.
    Vanilla,
}

/// Options passed to [`render_corpus`].
#[derive(Debug, Clone)]
pub struct CorpusOptions {
    /// Corpus content flavor.
    pub flavor: CorpusFlavor,
    /// Provider target for wrapping.
    pub target: CorpusTarget,
    /// Minimum token budget. If `Some(n)`, the corpus is padded with bank
    /// examples until the estimated token count meets or exceeds `n`.
    pub min_tokens: Option<usize>,
    /// When `true`, prepend `# AGM spec version: 1.2.0` to the body before
    /// provider wrapping.
    pub include_version: bool,
}

/// The result of [`render_corpus`].
#[derive(Debug, Clone)]
pub struct CorpusOutput {
    /// Final corpus body (preamble + template + any padding).
    pub body: String,
    /// Estimated token count of [`body`] after provider wrapping.
    pub estimated_tokens: usize,
    /// Flavor that was used.
    pub flavor: CorpusFlavor,
    /// Target that was used.
    pub target: CorpusTarget,
}

/// Errors that can occur while rendering a corpus.
#[derive(Debug, Error)]
pub enum CorpusError {
    /// The token budget cannot be met because the example bank is exhausted.
    #[error(
        "token budget {target} unreachable; bank exhausted at {actual} tokens. \
         Lower --min-tokens or add examples."
    )]
    BankExhausted {
        /// Requested minimum token count.
        target: usize,
        /// Actual token count when the bank ran out.
        actual: usize,
    },

    /// An I/O error occurred while writing output.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Embedded templates
// ---------------------------------------------------------------------------

const FULL_TMPL: &str = include_str!("full.md.tmpl");
const STANDARD_TMPL: &str = include_str!("standard.md.tmpl");
const GRAMMAR_ONLY_TMPL: &str = include_str!("grammar_only.md.tmpl");

/// Return the built-in template source for the given flavor.
///
/// The returned `&'static str` is the raw Markdown template embedded at
/// compile time.
#[must_use]
pub fn builtin_template(flavor: CorpusFlavor) -> &'static str {
    match flavor {
        CorpusFlavor::Full => FULL_TMPL,
        CorpusFlavor::Standard => STANDARD_TMPL,
        CorpusFlavor::GrammarOnly => GRAMMAR_ONLY_TMPL,
    }
}

// ---------------------------------------------------------------------------
// Embedded example bank
// ---------------------------------------------------------------------------

static EXAMPLE_BANK: &[(&str, &str)] = &[
    ("ex1", include_str!("examples/ex1.agm")),
    ("ex2", include_str!("examples/ex2.agm")),
    ("ex3", include_str!("examples/ex3.agm")),
    ("ex4", include_str!("examples/ex4.agm")),
    ("ex5", include_str!("examples/ex5.agm")),
    ("ex6", include_str!("examples/ex6.agm")),
    ("ex7", include_str!("examples/ex7.agm")),
    ("ex8", include_str!("examples/ex8.agm")),
    ("ex9", include_str!("examples/ex9.agm")),
    ("ex10", include_str!("examples/ex10.agm")),
    ("ex11", include_str!("examples/ex11.agm")),
    ("ex12", include_str!("examples/ex12.agm")),
];

/// Return the full example bank as a slice of `(name, content)` pairs.
///
/// Both `name` and `content` are `&'static str` — embedded at compile time.
#[must_use]
#[allow(dead_code)]
pub fn example_bank() -> &'static [(&'static str, &'static str)] {
    EXAMPLE_BANK
}

// ---------------------------------------------------------------------------
// render_corpus
// ---------------------------------------------------------------------------

/// Render a corpus according to `opts`.
///
/// Steps:
/// 1. Start from the built-in template for the requested flavor.
/// 2. Optionally prepend the AGM spec version header.
/// 3. Optionally pad with bank examples until `min_tokens` is met.
/// 4. Wrap for the provider target.
/// 5. Return [`CorpusOutput`] with the final body and estimated token count.
///
/// # Errors
///
/// Returns [`CorpusError::BankExhausted`] when `min_tokens` is set but the
/// entire example bank is consumed before the token budget is met.
#[must_use = "result must be used"]
pub fn render_corpus(opts: &CorpusOptions) -> Result<CorpusOutput, CorpusError> {
    // 1. Start from the template.
    let mut body = builtin_template(opts.flavor).to_owned();

    // 2. Optionally prepend the spec version header.
    if opts.include_version {
        body = format!("# AGM spec version: 1.2.0\n\n{body}");
    }

    // 3. Pad to meet the min_tokens budget.
    if let Some(min) = opts.min_tokens {
        let mut bank_iter = EXAMPLE_BANK.iter();

        loop {
            let current_tokens = tokens::estimate(&body);
            if current_tokens >= min {
                break;
            }

            match bank_iter.next() {
                Some((name, content)) => {
                    body.push_str(&format!(
                        "\n\n---\n\n# Example: {name}\n\n```agm\n{content}\n```"
                    ));
                }
                None => {
                    let actual = tokens::estimate(&body);
                    return Err(CorpusError::BankExhausted {
                        target: min,
                        actual,
                    });
                }
            }
        }
    }

    // 4. Wrap for provider target.
    body = wrap::wrap_for_target(body, opts.target);

    // 5. Compute final token estimate on the wrapped body.
    let estimated_tokens = tokens::estimate(&body);

    Ok(CorpusOutput {
        body,
        estimated_tokens,
        flavor: opts.flavor,
        target: opts.target,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_template_non_empty_for_each_flavor() {
        for flavor in [
            CorpusFlavor::Full,
            CorpusFlavor::Standard,
            CorpusFlavor::GrammarOnly,
        ] {
            let tmpl = builtin_template(flavor);
            assert!(!tmpl.is_empty(), "{flavor:?} template must not be empty");
        }
    }

    #[test]
    fn test_builtin_template_contains_agm_header() {
        for flavor in [
            CorpusFlavor::Full,
            CorpusFlavor::Standard,
            CorpusFlavor::GrammarOnly,
        ] {
            let tmpl = builtin_template(flavor);
            assert!(
                tmpl.contains("# AGM v1.2.0"),
                "{flavor:?} template must contain the AGM v1.2.0 header"
            );
        }
    }

    #[test]
    fn test_example_bank_has_at_least_12_entries() {
        assert!(
            example_bank().len() >= 12,
            "example bank must have at least 12 entries"
        );
    }

    #[test]
    fn test_render_corpus_no_padding_for_no_min_tokens() {
        let opts = CorpusOptions {
            flavor: CorpusFlavor::GrammarOnly,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: false,
        };
        let out = render_corpus(&opts).expect("render must succeed");
        assert!(out.body.contains("# AGM v1.2.0"));
        // Without padding, the body should not contain "# Example: ex1"
        assert!(!out.body.contains("# Example: ex1"));
    }

    #[test]
    fn test_render_corpus_meets_min_tokens_when_possible() {
        // Set a low min_tokens target that can be met by a few examples.
        let opts = CorpusOptions {
            flavor: CorpusFlavor::GrammarOnly,
            target: CorpusTarget::Vanilla,
            min_tokens: Some(1000),
            include_version: false,
        };
        let out = render_corpus(&opts).expect("render must succeed when bank can cover budget");
        assert!(
            out.estimated_tokens >= 1000,
            "estimated_tokens ({}) must be >= min_tokens (1000)",
            out.estimated_tokens
        );
    }

    #[test]
    fn test_render_corpus_returns_err_on_bank_exhaustion() {
        // An unreachably large min_tokens value forces exhaustion.
        let opts = CorpusOptions {
            flavor: CorpusFlavor::Full,
            target: CorpusTarget::Vanilla,
            min_tokens: Some(999_999),
            include_version: false,
        };
        let result = render_corpus(&opts);
        assert!(
            matches!(result, Err(CorpusError::BankExhausted { .. })),
            "must return BankExhausted when the bank cannot reach the budget"
        );
    }

    #[test]
    fn test_render_corpus_full_exceeds_standard_in_tokens() {
        let full = render_corpus(&CorpusOptions {
            flavor: CorpusFlavor::Full,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: false,
        })
        .expect("full render must succeed");

        let standard = render_corpus(&CorpusOptions {
            flavor: CorpusFlavor::Standard,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: false,
        })
        .expect("standard render must succeed");

        assert!(
            full.estimated_tokens > standard.estimated_tokens,
            "full ({}) must have more tokens than standard ({})",
            full.estimated_tokens,
            standard.estimated_tokens
        );
    }

    #[test]
    fn test_render_corpus_include_version_prepends_header() {
        let opts = CorpusOptions {
            flavor: CorpusFlavor::GrammarOnly,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: true,
        };
        let out = render_corpus(&opts).expect("render must succeed");
        assert!(
            out.body.contains("# AGM spec version: 1.2.0"),
            "body must contain the spec version header"
        );
    }

    #[test]
    fn test_render_corpus_estimated_tokens_computed_on_wrapped_body() {
        let opts = CorpusOptions {
            flavor: CorpusFlavor::GrammarOnly,
            target: CorpusTarget::Anthropic,
            min_tokens: None,
            include_version: false,
        };
        let out = render_corpus(&opts).expect("render must succeed");
        // The Anthropic preamble adds tokens; estimated_tokens is on the
        // final wrapped body and should equal what estimate() returns on out.body.
        let recomputed = tokens::estimate(&out.body);
        assert_eq!(
            out.estimated_tokens, recomputed,
            "estimated_tokens must equal estimate() of the final body"
        );
    }

    // --- Token-count regression bounds ---

    #[test]
    fn test_full_tokens_at_least_1800() {
        let opts = CorpusOptions {
            flavor: CorpusFlavor::Full,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: false,
        };
        let out = render_corpus(&opts).expect("render must succeed");
        assert!(
            out.estimated_tokens >= 1800,
            "full corpus must have at least 1800 tokens, got {}",
            out.estimated_tokens
        );
    }

    #[test]
    fn test_standard_tokens_at_most_1500() {
        let opts = CorpusOptions {
            flavor: CorpusFlavor::Standard,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: false,
        };
        let out = render_corpus(&opts).expect("render must succeed");
        assert!(
            out.estimated_tokens <= 1500,
            "standard corpus must have at most 1500 tokens, got {}",
            out.estimated_tokens
        );
    }

    #[test]
    fn test_grammar_only_tokens_at_most_600() {
        let opts = CorpusOptions {
            flavor: CorpusFlavor::GrammarOnly,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: false,
        };
        let out = render_corpus(&opts).expect("render must succeed");
        assert!(
            out.estimated_tokens <= 600,
            "grammar-only corpus must have at most 600 tokens, got {}",
            out.estimated_tokens
        );
    }

    #[test]
    fn test_anthropic_preamble_tokens_at_most_100() {
        let vanilla = render_corpus(&CorpusOptions {
            flavor: CorpusFlavor::GrammarOnly,
            target: CorpusTarget::Vanilla,
            min_tokens: None,
            include_version: false,
        })
        .expect("render must succeed");

        let anthropic = render_corpus(&CorpusOptions {
            flavor: CorpusFlavor::GrammarOnly,
            target: CorpusTarget::Anthropic,
            min_tokens: None,
            include_version: false,
        })
        .expect("render must succeed");

        let preamble_tokens = anthropic
            .estimated_tokens
            .saturating_sub(vanilla.estimated_tokens);
        assert!(
            preamble_tokens <= 100,
            "Anthropic preamble must add at most 100 tokens, added {preamble_tokens}"
        );
    }
}
