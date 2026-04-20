//! Token estimation for corpus output.
//!
//! Uses the `cl100k_base` tokenizer via `tiktoken-rs` for accurate counts.
//! Falls back to a character-based heuristic (`len / 4`) only if the encoder
//! cannot be initialized.

use std::sync::OnceLock;

use tiktoken_rs::CoreBPE;

static ENCODER: OnceLock<Option<CoreBPE>> = OnceLock::new();

/// Return a reference to the shared `cl100k_base` encoder.
///
/// Initializes on first call. Returns `None` if initialization fails; callers
/// should fall back to the character heuristic in that case.
fn encoder() -> Option<&'static CoreBPE> {
    ENCODER
        .get_or_init(|| tiktoken_rs::cl100k_base().ok())
        .as_ref()
}

/// Estimate the number of tokens in `s`.
///
/// Uses the `cl100k_base` BPE encoder when available. Falls back to
/// `s.len() / 4` if the encoder cannot be initialized.
pub fn estimate(s: &str) -> usize {
    match encoder() {
        Some(enc) => enc.encode_with_special_tokens(s).len(),
        None => s.len() / 4,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_known_input() {
        // "hello world" tokenizes to 2 tokens under cl100k_base.
        let count = estimate("hello world");
        assert_eq!(count, 2, "expected 2 tokens for 'hello world', got {count}");
    }

    #[test]
    fn test_estimate_empty() {
        assert_eq!(estimate(""), 0);
    }

    #[test]
    fn test_estimate_caches_encoder() {
        // Call twice; both should return consistent results without re-init.
        let first = estimate("the quick brown fox");
        let second = estimate("the quick brown fox");
        assert_eq!(first, second, "encoder must be deterministic across calls");
        assert!(first > 0, "non-empty string must have at least one token");
    }

    #[test]
    fn test_estimate_returns_nonzero_for_nonempty_string() {
        let count = estimate("AGM is a line-oriented format.");
        assert!(
            count > 0,
            "non-empty string must produce a positive token count"
        );
    }
}
