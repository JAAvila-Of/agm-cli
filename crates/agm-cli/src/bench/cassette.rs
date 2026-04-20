//! Cassette (recorded HTTP response) storage and replay.
//!
//! Layout on disk:
//! ```text
//! <dir>/v1.0.0/<provider>/<model>/<case_id>.json
//! ```
//!
//! Each cassette file is a JSON envelope containing the recorded response.
//! The lookup key is `SHA-256(provider:model:prompt)`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::provider::{Provider, ProviderError, ProviderRequestOpts, ProviderResponse};

// ---------------------------------------------------------------------------
// Cassette envelope
// ---------------------------------------------------------------------------

/// On-disk JSON envelope for a recorded provider response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CassetteEnvelope {
    /// SHA-256 of `"<provider>:<model>:<prompt>"`.
    pub request_hash: String,
    /// First 120 characters of the prompt, for human readability.
    pub prompt_preview: String,
    /// Extracted AGM text (or tool-use JSON) from the recorded response.
    pub response_text: String,
    /// Recorded input token count.
    pub tokens_in: u32,
    /// Recorded output token count.
    pub tokens_out: u32,
    /// Recorded round-trip latency in milliseconds.
    pub latency_ms: u64,
    /// ISO 8601 timestamp of when the cassette was recorded.
    pub recorded_at: String,
}

// ---------------------------------------------------------------------------
// CassetteMode
// ---------------------------------------------------------------------------

/// Controls whether cassette files are used and how.
#[derive(Debug, Clone)]
pub enum CassetteMode {
    /// Do not use cassettes; all requests go live.
    Disabled,
    /// Record responses into the given directory (overwrite existing).
    Record(PathBuf),
    /// Replay from directory; fail if cassette is missing.
    Replay(PathBuf),
    /// Replay if cassette exists; otherwise fall through to inner provider and record.
    RecordOrReplay(PathBuf),
}

// ---------------------------------------------------------------------------
// Hash helper
// ---------------------------------------------------------------------------

/// Compute the cassette lookup key: `SHA-256("<provider>:<model>:<prompt>")`.
#[must_use]
pub fn cassette_key(provider: &str, model: &str, prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(provider.as_bytes());
    hasher.update(b":");
    hasher.update(model.as_bytes());
    hasher.update(b":");
    hasher.update(prompt.as_bytes());
    hex::encode(hasher.finalize())
}

/// Build the cassette file path for a given case ID.
///
/// Path: `<dir>/v1.0.0/<provider>/<model>/<case_id>.json`
#[must_use]
pub fn cassette_path(dir: &Path, provider: &str, model: &str, case_id: &str) -> PathBuf {
    dir.join("v1.0.0")
        .join(provider)
        .join(model)
        .join(format!("{}.json", case_id.replace('/', "_")))
}

// ---------------------------------------------------------------------------
// Read / write helpers
// ---------------------------------------------------------------------------

/// Try to read a cassette for the given case and verify the request hash.
///
/// Returns `None` if the file does not exist or the hash does not match.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be parsed.
pub fn read_cassette(path: &Path, expected_hash: &str) -> anyhow::Result<Option<CassetteEnvelope>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read cassette {}: {e}", path.display()))?;
    let env: CassetteEnvelope = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("failed to parse cassette {}: {e}", path.display()))?;

    if env.request_hash != expected_hash {
        // Hash mismatch: cassette was recorded for a different prompt. Treat as miss.
        return Ok(None);
    }
    Ok(Some(env))
}

/// Write a cassette envelope to the given path, creating parent directories.
///
/// # Errors
///
/// Returns an error on I/O failure.
pub fn write_cassette(path: &Path, env: &CassetteEnvelope) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            anyhow::anyhow!("failed to create cassette dir {}: {e}", parent.display())
        })?;
    }
    let json = serde_json::to_string_pretty(env)
        .map_err(|e| anyhow::anyhow!("failed to serialize cassette: {e}"))?;
    std::fs::write(path, json)
        .map_err(|e| anyhow::anyhow!("failed to write cassette {}: {e}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// CassetteProvider — wraps another Provider
// ---------------------------------------------------------------------------

/// Wraps a `Provider` with cassette record/replay logic.
///
/// The `case_id` must be set before each `send` call via `set_case_id`.
pub struct CassetteProvider<'a> {
    inner: &'a dyn Provider,
    cassette_dir: &'a Path,
    mode: CassetteMode,
    /// Case ID used to construct the cassette path.
    case_id: Mutex<String>,
}

impl<'a> CassetteProvider<'a> {
    #[must_use]
    pub fn new(inner: &'a dyn Provider, cassette_dir: &'a Path, mode: CassetteMode) -> Self {
        Self {
            inner,
            cassette_dir,
            mode,
            case_id: Mutex::new(String::new()),
        }
    }

    pub fn set_case_id(&self, id: &str) {
        *self.case_id.lock().unwrap() = id.to_owned();
    }
}

impl Provider for CassetteProvider<'_> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn model(&self) -> &str {
        self.inner.model()
    }

    fn send(
        &self,
        prompt: &str,
        opts: &ProviderRequestOpts,
    ) -> Result<ProviderResponse, ProviderError> {
        let case_id = self.case_id.lock().unwrap().clone();
        let hash = cassette_key(self.inner.name(), self.inner.model(), prompt);
        let path = cassette_path(
            self.cassette_dir,
            self.inner.name(),
            self.inner.model(),
            &case_id,
        );

        match &self.mode {
            CassetteMode::Disabled => self.inner.send(prompt, opts),

            CassetteMode::Replay(_) => {
                let env = read_cassette(&path, &hash)
                    .map_err(|e| ProviderError::ParseResponse(e.to_string()))?;
                match env {
                    Some(e) => Ok(ProviderResponse {
                        raw_text: e.response_text,
                        tokens_in: e.tokens_in,
                        tokens_out: e.tokens_out,
                        latency_ms: e.latency_ms,
                    }),
                    None => Err(ProviderError::CassetteMiss),
                }
            }

            CassetteMode::Record(_) => {
                let resp = self.inner.send(prompt, opts)?;
                let env = CassetteEnvelope {
                    request_hash: hash,
                    prompt_preview: prompt.chars().take(120).collect(),
                    response_text: resp.raw_text.clone(),
                    tokens_in: resp.tokens_in,
                    tokens_out: resp.tokens_out,
                    latency_ms: resp.latency_ms,
                    recorded_at: chrono_now(),
                };
                write_cassette(&path, &env).map_err(|e| ProviderError::Network(e.to_string()))?;
                Ok(resp)
            }

            CassetteMode::RecordOrReplay(_) => {
                let env = read_cassette(&path, &hash)
                    .map_err(|e| ProviderError::ParseResponse(e.to_string()))?;
                if let Some(e) = env {
                    return Ok(ProviderResponse {
                        raw_text: e.response_text,
                        tokens_in: e.tokens_in,
                        tokens_out: e.tokens_out,
                        latency_ms: e.latency_ms,
                    });
                }
                let resp = self.inner.send(prompt, opts)?;
                let env = CassetteEnvelope {
                    request_hash: hash,
                    prompt_preview: prompt.chars().take(120).collect(),
                    response_text: resp.raw_text.clone(),
                    tokens_in: resp.tokens_in,
                    tokens_out: resp.tokens_out,
                    latency_ms: resp.latency_ms,
                    recorded_at: chrono_now(),
                };
                write_cassette(&path, &env).map_err(|e| ProviderError::Network(e.to_string()))?;
                Ok(resp)
            }
        }
    }
}

fn chrono_now() -> String {
    // Simple RFC 3339 timestamp without pulling in chrono
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as rough ISO 8601 — good enough for cassette metadata
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Approximate date (not exact calendar, but stable and readable)
    format!("2026-01-01T{days:05}+{h:02}:{m:02}:{s:02}Z")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_hash_deterministic() {
        let k1 = cassette_key("messages", "demo-m1", "hello world");
        let k2 = cassette_key("messages", "demo-m1", "hello world");
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_key_hash_differs_on_different_inputs() {
        let k1 = cassette_key("messages", "demo-m1", "prompt A");
        let k2 = cassette_key("chat", "demo-m1", "prompt A");
        let k3 = cassette_key("messages", "demo-m1", "prompt B");
        assert_ne!(k1, k2);
        assert_ne!(k1, k3);
    }

    #[test]
    fn test_record_then_replay() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        let env = CassetteEnvelope {
            request_hash: cassette_key("messages", "demo-m1", "test prompt"),
            prompt_preview: "test prompt".into(),
            response_text: "agm node content".into(),
            tokens_in: 100,
            tokens_out: 50,
            latency_ms: 1000,
            recorded_at: "2026-04-20T00:00:00Z".into(),
        };

        let path = cassette_path(dir, "messages", "demo-m1", "ticket/a");
        write_cassette(&path, &env).unwrap();

        let hash = cassette_key("messages", "demo-m1", "test prompt");
        let loaded = read_cassette(&path, &hash).unwrap().unwrap();
        assert_eq!(loaded.response_text, "agm node content");
        assert_eq!(loaded.tokens_in, 100);
    }

    #[test]
    fn test_replay_miss_returns_none() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let path = cassette_path(tmp.path(), "messages", "demo-m1", "ticket/z");
        let result = read_cassette(&path, "deadbeef").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_replay_hash_mismatch_returns_none() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();

        let env = CassetteEnvelope {
            request_hash: "abc123".into(),
            prompt_preview: "prompt".into(),
            response_text: "data".into(),
            tokens_in: 10,
            tokens_out: 10,
            latency_ms: 100,
            recorded_at: "2026-04-20T00:00:00Z".into(),
        };
        let path = cassette_path(tmp.path(), "messages", "demo-m1", "ticket/a");
        write_cassette(&path, &env).unwrap();

        // Different hash — should return None (miss)
        let result = read_cassette(&path, "different_hash").unwrap();
        assert!(result.is_none());
    }
}
