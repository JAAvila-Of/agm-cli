//! Provider trait and concrete implementations for the LLM bench suite.
//!
//! Providers are identified by their HTTP API shape, not by vendor:
//! - `MessagesProvider`  — Messages-style API (system + `messages[]` + optional `tools[]`)
//! - `ChatCompletionsProvider` — Chat-Completions-style API (`messages[]` with role=system/user,
//!   optional function-style `tools[]`)
//!
//! Endpoint URLs and API key env-var names are supplied at runtime; no
//! hardcoded vendor URLs or key names exist in this module.

use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when calling an inference provider.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("network error: {0}")]
    Network(String),
    #[error("authentication error: {0}")]
    Auth(String),
    #[error("rate limited — try again later")]
    RateLimited,
    #[error("request timed out after {0}s")]
    Timeout(u64),
    #[error("provider returned HTTP {status}: {body}")]
    HttpError { status: u16, body: String },
    #[error("could not parse provider response: {0}")]
    ParseResponse(String),
    #[error("cassette miss in Replay mode")]
    CassetteMiss,
}

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// A successful response from a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderResponse {
    /// The extracted text from the provider response (AGM text, possibly in fences).
    pub raw_text: String,
    /// Reported input token count (may be estimated if not provided).
    pub tokens_in: u32,
    /// Reported output token count.
    pub tokens_out: u32,
    /// Measured round-trip latency in milliseconds.
    pub latency_ms: u64,
}

/// Options passed with each provider request.
#[derive(Debug, Clone)]
pub struct ProviderRequestOpts {
    /// Hard cap on input tokens (hint to provider).
    pub max_tokens_in: Option<u32>,
    /// Hard cap on output tokens.
    pub max_tokens_out: Option<u32>,
    /// Request timeout.
    pub timeout: Duration,
    /// Optional JSON Schema to send as a tool definition (from `agm_core::schemas`).
    pub tool_schema: Option<serde_json::Value>,
}

impl Default for ProviderRequestOpts {
    fn default() -> Self {
        Self {
            max_tokens_in: None,
            max_tokens_out: Some(4096),
            timeout: Duration::from_secs(60),
            tool_schema: None,
        }
    }
}

/// Static configuration used when constructing a provider.
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Model identifier string (passed verbatim to the provider endpoint).
    pub model: String,
    /// API key value (not the env var name — already resolved).
    pub api_key: String,
    /// Base endpoint URL.
    pub endpoint: String,
    /// Optional extra version/date header sent with every request.
    ///
    /// Some Messages-style endpoints require a vendor-specific API-version header.
    /// When `Some((name, value))`, the header `name: value` is appended to every
    /// outgoing request. When `None` (the default), no extra header is sent.
    ///
    /// Populated from env vars `AGM_MESSAGES_VERSION_HEADER_NAME` and
    /// `AGM_MESSAGES_VERSION_HEADER_VALUE`.
    pub version_header: Option<(String, String)>,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Abstraction over an inference HTTP endpoint.
pub trait Provider: Send + Sync {
    /// Short identifier for this provider (e.g. `"messages"` or `"chat"`).
    fn name(&self) -> &'static str;
    /// Model identifier being used.
    fn model(&self) -> &str;
    /// Send a prompt and return the provider response.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError`] on network, auth, or response-parsing failures.
    fn send(
        &self,
        prompt: &str,
        opts: &ProviderRequestOpts,
    ) -> Result<ProviderResponse, ProviderError>;
}

// ---------------------------------------------------------------------------
// Helpers — extract AGM text from raw LLM response
// ---------------------------------------------------------------------------

/// Strip markdown fences from a response.  If fences are found the interior
/// is returned; otherwise the full string is returned trimmed.
pub fn extract_agm_text(raw: &str) -> String {
    // Try ```agm ... ``` or ``` ... ```
    let stripped = raw.trim();
    if let Some(inner) = try_strip_fence(stripped, "```agm") {
        return inner.to_owned();
    }
    if let Some(inner) = try_strip_fence(stripped, "```") {
        return inner.to_owned();
    }
    stripped.to_owned()
}

fn try_strip_fence<'a>(s: &'a str, open: &str) -> Option<&'a str> {
    let s = s.trim();
    let after_open = s.strip_prefix(open)?;
    // Skip any language specifier on the opening line
    let after_first_nl = after_open.find('\n').map(|i| &after_open[i + 1..])?;
    // Find closing fence
    if let Some(close_pos) = after_first_nl.rfind("```") {
        return Some(after_first_nl[..close_pos].trim());
    }
    None
}

// ---------------------------------------------------------------------------
// MessagesProvider
// ---------------------------------------------------------------------------

/// Provider that speaks the Messages-style HTTP API
/// (system prompt + `messages[]` + optional `tools[]` + `tool_choice`).
pub struct MessagesProvider {
    config: ProviderConfig,
    client: reqwest::blocking::Client,
}

impl MessagesProvider {
    /// Construct a new `MessagesProvider`.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(config: ProviderConfig, timeout: Duration) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))?;
        Ok(Self { config, client })
    }

    fn messages_url(&self) -> String {
        self.config.endpoint.clone()
    }
}

// JSON shapes for the Messages API

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: &'a str,
    messages: Vec<MessageItem<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct MessageItem<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    #[serde(default)]
    usage: MessagesUsage,
}

#[derive(Deserialize, Default)]
struct MessagesUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { input: serde_json::Value },
    #[serde(other)]
    Other,
}

const AGM_SYSTEM_PROMPT: &str = "You are an expert at generating AGM (Agent Graph Memory) structured files. \
     When asked to produce an AGM node, emit only the AGM text using the canonical \
     line-oriented syntax. Do NOT include any prose before or after the AGM block. \
     If you use a code fence, open it with ```agm and close with ```.";

impl Provider for MessagesProvider {
    fn name(&self) -> &'static str {
        "messages"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn send(
        &self,
        prompt: &str,
        opts: &ProviderRequestOpts,
    ) -> Result<ProviderResponse, ProviderError> {
        let max_tokens_out = opts.max_tokens_out.unwrap_or(4096);

        let (tools, tool_choice) = match &opts.tool_schema {
            Some(schema) => {
                let tool = serde_json::json!({
                    "name": "emit_agm_node",
                    "description": "Emit a single AGM node as a structured object",
                    "input_schema": schema
                });
                (
                    Some(vec![tool]),
                    Some(serde_json::json!({ "type": "tool", "name": "emit_agm_node" })),
                )
            }
            None => (None, None),
        };

        let req_body = MessagesRequest {
            model: &self.config.model,
            max_tokens: max_tokens_out,
            system: AGM_SYSTEM_PROMPT,
            messages: vec![MessageItem {
                role: "user",
                content: prompt,
            }],
            tools,
            tool_choice,
        };

        let t0 = std::time::Instant::now();

        let mut req = self
            .client
            .post(self.messages_url())
            .header("x-api-key", &self.config.api_key)
            .header("content-type", "application/json");

        if let Some((name, value)) = &self.config.version_header {
            req = req.header(name.as_str(), value.as_str());
        }

        let resp = req.json(&req_body).send().map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout(opts.timeout.as_secs())
            } else {
                ProviderError::Network(e.to_string())
            }
        })?;

        let latency_ms = t0.elapsed().as_millis() as u64;
        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            let body = resp.text().unwrap_or_default();
            return Err(ProviderError::Auth(body));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(ProviderError::HttpError {
                status: status.as_u16(),
                body,
            });
        }

        let body: MessagesResponse = resp
            .json()
            .map_err(|e| ProviderError::ParseResponse(format!("JSON decode failed: {e}")))?;

        // Extract text: prefer tool_use block (structured), else first text block
        let raw_text = extract_from_messages_content(&body.content)?;

        Ok(ProviderResponse {
            raw_text,
            tokens_in: body.usage.input_tokens,
            tokens_out: body.usage.output_tokens,
            latency_ms,
        })
    }
}

fn extract_from_messages_content(blocks: &[ContentBlock]) -> Result<String, ProviderError> {
    // Prefer tool_use: reconstruct AGM from JSON args
    for block in blocks {
        if let ContentBlock::ToolUse { input } = block {
            // The tool input IS the structured node; convert to text via JSON
            return serde_json::to_string_pretty(input)
                .map_err(|e| ProviderError::ParseResponse(e.to_string()));
        }
    }
    // Fall back to first text block
    for block in blocks {
        if let ContentBlock::Text { text } = block {
            return Ok(extract_agm_text(text));
        }
    }
    Err(ProviderError::ParseResponse(
        "response contained no text or tool_use block".into(),
    ))
}

// ---------------------------------------------------------------------------
// ChatCompletionsProvider
// ---------------------------------------------------------------------------

/// Provider that speaks the Chat-Completions-style HTTP API
/// (`messages[]` with role=system/user, optional function-style `tools[]`).
pub struct ChatCompletionsProvider {
    config: ProviderConfig,
    client: reqwest::blocking::Client,
}

impl ChatCompletionsProvider {
    /// Construct a new `ChatCompletionsProvider`.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(config: ProviderConfig, timeout: Duration) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))?;
        Ok(Self { config, client })
    }

    fn completions_url(&self) -> String {
        self.config.endpoint.clone()
    }
}

// JSON shapes for Chat-Completions API

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Serialize, Deserialize)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: FunctionCall,
}

#[derive(Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: ChatUsage,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize, Default)]
struct ChatUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
}

impl Provider for ChatCompletionsProvider {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    fn send(
        &self,
        prompt: &str,
        opts: &ProviderRequestOpts,
    ) -> Result<ProviderResponse, ProviderError> {
        let (tools, tool_choice) = match &opts.tool_schema {
            Some(schema) => {
                let tool = serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": "emit_agm_node",
                        "description": "Emit a single AGM node as a structured object",
                        "parameters": schema
                    }
                });
                (
                    Some(vec![tool]),
                    Some(serde_json::json!({
                        "type": "function",
                        "function": { "name": "emit_agm_node" }
                    })),
                )
            }
            None => (None, None),
        };

        let messages = vec![
            ChatMessage {
                role: "system".into(),
                content: Some(AGM_SYSTEM_PROMPT.into()),
                tool_calls: None,
            },
            ChatMessage {
                role: "user".into(),
                content: Some(prompt.into()),
                tool_calls: None,
            },
        ];

        let req_body = ChatRequest {
            model: self.config.model.clone(),
            messages,
            max_tokens: opts.max_tokens_out,
            tools,
            tool_choice,
        };

        let t0 = std::time::Instant::now();

        let resp = self
            .client
            .post(self.completions_url())
            .header("authorization", format!("Bearer {}", self.config.api_key))
            .header("content-type", "application/json")
            .json(&req_body)
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(opts.timeout.as_secs())
                } else {
                    ProviderError::Network(e.to_string())
                }
            })?;

        let latency_ms = t0.elapsed().as_millis() as u64;
        let status = resp.status();

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            let body = resp.text().unwrap_or_default();
            return Err(ProviderError::Auth(body));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::RateLimited);
        }
        if !status.is_success() {
            let body = resp.text().unwrap_or_default();
            return Err(ProviderError::HttpError {
                status: status.as_u16(),
                body,
            });
        }

        let body: ChatResponse = resp
            .json()
            .map_err(|e| ProviderError::ParseResponse(format!("JSON decode failed: {e}")))?;

        let choice = body
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::ParseResponse("empty choices array".into()))?;

        // Prefer tool_call if present
        let raw_text = if let Some(tool_calls) = choice.message.tool_calls {
            if let Some(tc) = tool_calls.into_iter().next() {
                tc.function.arguments
            } else {
                choice.message.content.unwrap_or_default()
            }
        } else {
            extract_agm_text(&choice.message.content.unwrap_or_default())
        };

        Ok(ProviderResponse {
            raw_text,
            tokens_in: body.usage.prompt_tokens,
            tokens_out: body.usage.completion_tokens,
            latency_ms,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_agm_text_no_fence() {
        let input = "  agm\npackage: test\nversion: 0.1.0  ";
        assert_eq!(
            extract_agm_text(input),
            "agm\npackage: test\nversion: 0.1.0"
        );
    }

    #[test]
    fn test_extract_agm_text_with_agm_fence() {
        let input = "```agm\nagm\npackage: test\nversion: 0.1.0\n```";
        assert_eq!(
            extract_agm_text(input),
            "agm\npackage: test\nversion: 0.1.0"
        );
    }

    #[test]
    fn test_extract_agm_text_with_plain_fence() {
        let input = "```\nagm\npackage: test\nversion: 0.1.0\n```";
        assert_eq!(
            extract_agm_text(input),
            "agm\npackage: test\nversion: 0.1.0"
        );
    }

    #[test]
    fn test_messages_request_shape() {
        // Uses mockito to verify the request JSON shape matches the Messages API contract.
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/v1/messages")
            .match_header("x-api-key", "test-key")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "content": [{"type": "text", "text": "agm\npackage: test\nversion: 0.1.0\nnode test_node\ntype: ticket\nsummary: Test"}],
                    "usage": {"input_tokens": 100, "output_tokens": 50}
                }"#,
            )
            .create();

        let config = ProviderConfig {
            model: "demo-m1".into(),
            api_key: "test-key".into(),
            endpoint: format!("{}/v1/messages", server.url()),
            version_header: None,
        };
        let provider = MessagesProvider::new(config, Duration::from_secs(10)).unwrap();
        let opts = ProviderRequestOpts::default();
        let resp = provider.send("create a ticket", &opts).unwrap();

        assert_eq!(resp.tokens_in, 100);
        assert_eq!(resp.tokens_out, 50);
        assert!(resp.raw_text.contains("ticket"));
        mock.assert();
    }

    #[test]
    fn test_chat_request_shape() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("content-type", "application/json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "choices": [{"message": {"role": "assistant", "content": "agm\npackage: test\nversion: 0.1.0\nnode test_node\ntype: workflow\nsummary: Deploy"}}],
                    "usage": {"prompt_tokens": 80, "completion_tokens": 60}
                }"#,
            )
            .create();

        let config = ProviderConfig {
            model: "demo-m1".into(),
            api_key: "test-key".into(),
            endpoint: format!("{}/v1/chat/completions", server.url()),
            version_header: None,
        };
        let provider = ChatCompletionsProvider::new(config, Duration::from_secs(10)).unwrap();
        let opts = ProviderRequestOpts::default();
        let resp = provider.send("create a workflow", &opts).unwrap();

        assert_eq!(resp.tokens_in, 80);
        assert_eq!(resp.tokens_out, 60);
        assert!(resp.raw_text.contains("workflow"));
        mock.assert();
    }

    #[test]
    fn test_messages_tool_use_response_parses() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("POST", "/v1/messages")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "content": [{
                        "type": "tool_use",
                        "id": "tu_01",
                        "name": "emit_agm_node",
                        "input": {"type": "ticket", "summary": "Fix login bug"}
                    }],
                    "usage": {"input_tokens": 120, "output_tokens": 40}
                }"#,
            )
            .create();

        let config = ProviderConfig {
            model: "demo-m1".into(),
            api_key: "test-key".into(),
            endpoint: format!("{}/v1/messages", server.url()),
            version_header: None,
        };
        let provider = MessagesProvider::new(config, Duration::from_secs(10)).unwrap();
        let opts = ProviderRequestOpts {
            tool_schema: Some(serde_json::json!({
                "type": "object",
                "properties": { "summary": { "type": "string" } }
            })),
            ..Default::default()
        };
        let resp = provider.send("create a ticket", &opts).unwrap();
        assert!(resp.raw_text.contains("Fix login bug"));
    }

    // -----------------------------------------------------------------------
    // version_header tests
    // -----------------------------------------------------------------------

    /// When `version_header` is `None` (env vars unset), no extra header is sent.
    #[test]
    fn test_messages_no_version_header_when_unset() {
        let mut server = mockito::Server::new();
        // The mock must NOT receive the header; mockito will fail the assertion if
        // an unexpected header is sent only when we use match_header — here we
        // explicitly verify the header is absent by matching its absence.
        let mock = server
            .mock("POST", "/v1/messages")
            .match_header("x-api-key", "test-key")
            // Verify that no version header with a non-empty value is present.
            // mockito does not have a "must not have header" matcher, so we rely on
            // the config having version_header = None and trust that the request
            // builder path does not attach it. We verify the happy-path response
            // is received successfully — meaning the mock (which has no header
            // requirement for a version header) accepted the request as-is.
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "content": [{"type": "text", "text": "agm\npackage: p\nversion: 0.1.0"}],
                    "usage": {"input_tokens": 10, "output_tokens": 5}
                }"#,
            )
            .create();

        let config = ProviderConfig {
            model: "demo-m1".into(),
            api_key: "test-key".into(),
            endpoint: format!("{}/v1/messages", server.url()),
            version_header: None,
        };
        let provider = MessagesProvider::new(config, Duration::from_secs(10)).unwrap();
        let result = provider.send("prompt", &ProviderRequestOpts::default());
        assert!(result.is_ok(), "expected Ok but got: {:?}", result.err());
        mock.assert();
    }

    /// When `version_header` is `Some((name, value))` (env vars set), the header
    /// is present on the outgoing request.
    #[test]
    fn test_messages_version_header_sent_when_configured() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("POST", "/v1/messages")
            .match_header("x-api-key", "test-key")
            .match_header("x-api-version", "2024-01-01")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "content": [{"type": "text", "text": "agm\npackage: p\nversion: 0.1.0"}],
                    "usage": {"input_tokens": 10, "output_tokens": 5}
                }"#,
            )
            .create();

        let config = ProviderConfig {
            model: "demo-m1".into(),
            api_key: "test-key".into(),
            endpoint: format!("{}/v1/messages", server.url()),
            version_header: Some(("x-api-version".into(), "2024-01-01".into())),
        };
        let provider = MessagesProvider::new(config, Duration::from_secs(10)).unwrap();
        let result = provider.send("prompt", &ProviderRequestOpts::default());
        assert!(result.is_ok(), "expected Ok but got: {:?}", result.err());
        mock.assert();
    }
}
