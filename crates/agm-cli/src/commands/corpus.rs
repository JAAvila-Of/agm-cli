//! `corpus` command: emit a cacheable, provider-aware AGM system-prompt corpus.
//!
//! Exit codes:
//!   0 — corpus rendered and written successfully.
//!   1 — min-tokens budget cannot be met (bank exhausted).
//!   2 — I/O error writing to --output.

use std::io::Write;
use std::path::Path;

use crate::corpora::{CorpusError, CorpusFlavor, CorpusOptions, CorpusTarget, render_corpus};
use serde::Serialize;

// ---------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------

const EXIT_OK: i32 = 0;
const EXIT_BANK_EXHAUSTED: i32 = 1;
const EXIT_IO_ERROR: i32 = 2;

// ---------------------------------------------------------------------------
// CLI arg enums (converted from clap ValueEnum variants in main.rs)
// ---------------------------------------------------------------------------

/// Corpus content flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusFlavorArg {
    Full,
    Standard,
    GrammarOnly,
}

impl CorpusFlavorArg {
    pub fn to_core(self) -> CorpusFlavor {
        match self {
            Self::Full => CorpusFlavor::Full,
            Self::Standard => CorpusFlavor::Standard,
            Self::GrammarOnly => CorpusFlavor::GrammarOnly,
        }
    }
}

/// Provider target for corpus wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusTargetArg {
    Anthropic,
    OpenAi,
    Vanilla,
}

impl CorpusTargetArg {
    pub fn to_core(self) -> CorpusTarget {
        match self {
            Self::Anthropic => CorpusTarget::Anthropic,
            Self::OpenAi => CorpusTarget::OpenAi,
            Self::Vanilla => CorpusTarget::Vanilla,
        }
    }
}

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusFormatArg {
    Text,
    Json,
}

// ---------------------------------------------------------------------------
// JSON output shape
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonOutput<'a> {
    body: &'a str,
    estimated_tokens: usize,
    flavor: &'a str,
    target: &'a str,
}

fn flavor_str(f: CorpusFlavor) -> &'static str {
    match f {
        CorpusFlavor::Full => "full",
        CorpusFlavor::Standard => "standard",
        CorpusFlavor::GrammarOnly => "grammar-only",
    }
}

fn target_str(t: CorpusTarget) -> &'static str {
    match t {
        CorpusTarget::Anthropic => "anthropic",
        CorpusTarget::OpenAi => "openai",
        CorpusTarget::Vanilla => "vanilla",
    }
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Entry point called from `main.rs`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    flavor: CorpusFlavorArg,
    for_target: CorpusTargetArg,
    min_tokens: Option<usize>,
    no_version: bool,
    output: Option<&Path>,
    format: CorpusFormatArg,
    count_only: bool,
) -> i32 {
    let opts = CorpusOptions {
        flavor: flavor.to_core(),
        target: for_target.to_core(),
        min_tokens,
        include_version: !no_version,
    };

    let corpus = match render_corpus(&opts) {
        Ok(c) => c,
        Err(CorpusError::BankExhausted { target, actual }) => {
            eprintln!(
                "error: cannot reach {target}-token target; bank exhausted at {actual} tokens. \
                 Lower --min-tokens or add examples."
            );
            return EXIT_BANK_EXHAUSTED;
        }
        Err(CorpusError::Io(e)) => {
            eprintln!("error: I/O error: {e}");
            return EXIT_IO_ERROR;
        }
    };

    // --count-only: print just the integer token count.
    if count_only {
        let line = format!("{}\n", corpus.estimated_tokens);
        return match write_output(output, line.as_bytes()) {
            Ok(()) => EXIT_OK,
            Err(e) => {
                eprintln!("error: write failed: {e}");
                EXIT_IO_ERROR
            }
        };
    }

    // Build output content.
    let content = match format {
        CorpusFormatArg::Text => corpus.body.clone(),
        CorpusFormatArg::Json => {
            let json_out = JsonOutput {
                body: &corpus.body,
                estimated_tokens: corpus.estimated_tokens,
                flavor: flavor_str(corpus.flavor),
                target: target_str(corpus.target),
            };
            match serde_json::to_string_pretty(&json_out) {
                Ok(s) => s + "\n",
                Err(e) => {
                    eprintln!("error: JSON serialization failed: {e}");
                    return EXIT_IO_ERROR;
                }
            }
        }
    };

    match write_output(output, content.as_bytes()) {
        Ok(()) => EXIT_OK,
        Err(e) => {
            eprintln!("error: write failed: {e}");
            EXIT_IO_ERROR
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_output(output: Option<&Path>, content: &[u8]) -> Result<(), std::io::Error> {
    match output {
        Some(path) => std::fs::write(path, content),
        None => std::io::stdout().write_all(content),
    }
}
