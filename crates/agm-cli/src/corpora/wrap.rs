//! Provider-specific wrapping of corpus bodies.
//!
//! Each provider target may prepend a short role preamble to the corpus body.
//! `Vanilla` is always the identity transformation.

use super::CorpusTarget;

/// Prepend from §6 Step 6 of news_5 for the Anthropic target.
const ANTHROPIC_PREAMBLE: &str = "\
You are an AGM-emitting assistant. When asked to produce knowledge, emit \
valid AGM v1.2.0 text inside a fenced ```agm block. Follow the grammar \
below strictly. Do not invent fields. When given a tool schema, prefer \
the tool call.

---

";

/// Prepend from §6 Step 6 of news_5 for the OpenAI target.
const OPENAI_PREAMBLE: &str = "\
You are an AGM-emitting assistant. When the host provides an `create_ticket`, \
`plan_execution`, or similar tool, always use the tool rather than free-form \
AGM text. Otherwise, emit AGM inside ```agm fences. Follow the grammar.

---

";

/// Wrap `body` with any provider-specific preamble.
///
/// - `Vanilla` — returns `body` unchanged.
/// - `Anthropic` — prepends the Anthropic role preamble and `---` separator.
/// - `OpenAi` — prepends the OpenAI tool-preference preamble and `---` separator.
pub fn wrap_for_target(body: String, target: CorpusTarget) -> String {
    match target {
        CorpusTarget::Vanilla => body,
        CorpusTarget::Anthropic => format!("{ANTHROPIC_PREAMBLE}{body}"),
        CorpusTarget::OpenAi => format!("{OPENAI_PREAMBLE}{body}"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vanilla_is_identity() {
        let body = "# AGM v1.2.0 — Compact Reference\n\nSome content.".to_owned();
        let result = wrap_for_target(body.clone(), CorpusTarget::Vanilla);
        assert_eq!(result, body);
    }

    #[test]
    fn test_anthropic_prepends_role() {
        let body = "# AGM v1.2.0 — Compact Reference\n".to_owned();
        let result = wrap_for_target(body.clone(), CorpusTarget::Anthropic);
        assert!(
            result.starts_with("You are an AGM-emitting assistant."),
            "Anthropic output must start with the role preamble"
        );
        assert!(
            result.contains(&body),
            "Anthropic output must contain the original body"
        );
    }

    #[test]
    fn test_openai_prepends_tool_preference_role() {
        let body = "# AGM v1.2.0 — Compact Reference\n".to_owned();
        let result = wrap_for_target(body.clone(), CorpusTarget::OpenAi);
        assert!(
            result.starts_with("You are an AGM-emitting assistant."),
            "OpenAI output must start with the role preamble"
        );
        assert!(
            result.contains("create_ticket"),
            "OpenAI preamble must mention the tool convention"
        );
        assert!(
            result.contains(&body),
            "OpenAI output must contain the original body"
        );
    }

    #[test]
    fn test_anthropic_separator_before_body() {
        let body = "BODY_CONTENT".to_owned();
        let result = wrap_for_target(body, CorpusTarget::Anthropic);
        // The separator "---" followed by blank line should appear before the body.
        assert!(result.contains("---\n\nBODY_CONTENT"));
    }

    #[test]
    fn test_openai_separator_before_body() {
        let body = "BODY_CONTENT".to_owned();
        let result = wrap_for_target(body, CorpusTarget::OpenAi);
        assert!(result.contains("---\n\nBODY_CONTENT"));
    }
}
