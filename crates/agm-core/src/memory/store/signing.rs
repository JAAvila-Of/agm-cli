//! HMAC-SHA256 signing support for `.agm.mem` sidecar files.
//!
//! Signing provides integrity verification — it does NOT encrypt memory values.
//! Any process with access to the file can read its contents. The HMAC only
//! detects tampering or stale syncs.

use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::error::MemoryStoreError;

type HmacSha256 = Hmac<Sha256>;

// ---------------------------------------------------------------------------
// HmacKey
// ---------------------------------------------------------------------------

/// A wrapper around raw HMAC key bytes.
///
/// The `Debug` implementation deliberately does NOT print the key bytes —
/// it prints only `HmacKey(len=N)` to avoid accidental leakage in logs.
#[derive(Clone)]
pub struct HmacKey(Vec<u8>);

impl std::fmt::Debug for HmacKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HmacKey(len={})", self.0.len())
    }
}

#[cfg(feature = "zeroize")]
impl Drop for HmacKey {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.0.zeroize();
    }
}

impl HmacKey {
    /// Constructs an `HmacKey` from raw bytes.
    ///
    /// A 32-byte key is recommended for HMAC-SHA256.
    #[must_use]
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self(bytes.into())
    }

    /// Reads a hex-encoded key from an environment variable.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError::KeyUnavailable`] if the variable is unset or
    /// contains invalid hex.
    pub fn from_env(name: &str) -> Result<Self, MemoryStoreError> {
        let raw = std::env::var(name).map_err(|_| {
            MemoryStoreError::KeyUnavailable(format!("environment variable `{name}` is not set"))
        })?;
        let bytes = hex::decode(raw.trim()).map_err(|e| {
            MemoryStoreError::KeyUnavailable(format!(
                "environment variable `{name}` does not contain valid hex: {e}"
            ))
        })?;
        Ok(Self(bytes))
    }

    /// Reads a hex-encoded key from a file (one line, whitespace trimmed).
    ///
    /// # Errors
    /// Returns [`MemoryStoreError::KeyUnavailable`] if the file cannot be read
    /// or contains invalid hex.
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, MemoryStoreError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            MemoryStoreError::KeyUnavailable(format!(
                "cannot read key file `{}`: {e}",
                path.display()
            ))
        })?;
        let bytes = hex::decode(content.trim()).map_err(|e| {
            MemoryStoreError::KeyUnavailable(format!(
                "key file `{}` does not contain valid hex: {e}",
                path.display()
            ))
        })?;
        Ok(Self(bytes))
    }

    /// Generates a fresh 32-byte random key using the OS CSPRNG.
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = vec![0u8; 32];
        getrandom::getrandom(&mut bytes).expect("OS CSPRNG should always be available");
        Self(bytes)
    }

    /// Returns the raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// SigningMode
// ---------------------------------------------------------------------------

/// Controls how writes are signed.
#[derive(Clone, Default)]
pub enum SigningMode {
    /// Do not sign writes. Existing signatures are ignored unless `VerifyMode` requires them.
    #[default]
    Disabled,
    /// Sign writes with `key`.
    Enabled { key: HmacKey },
    /// Sign writes with `active`; during verification also accept `accepted` (key rotation).
    EnabledWithRotation {
        active: HmacKey,
        accepted: Vec<HmacKey>,
    },
}

impl std::fmt::Debug for SigningMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => write!(f, "SigningMode::Disabled"),
            Self::Enabled { .. } => write!(f, "SigningMode::Enabled {{ key: HmacKey(..) }}"),
            Self::EnabledWithRotation { accepted, .. } => write!(
                f,
                "SigningMode::EnabledWithRotation {{ active: HmacKey(..), accepted: [{} key(s)] }}",
                accepted.len()
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// VerifyMode
// ---------------------------------------------------------------------------

/// Controls how signatures are verified on load.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerifyMode {
    /// Accept signed and unsigned sidecars without verification.
    #[default]
    Permissive,
    /// Verify if a signature is present; missing signature is accepted.
    IfPresent,
    /// Verify; reject if signature is missing or invalid.
    Strict,
}

// ---------------------------------------------------------------------------
// SignatureEnvelope
// ---------------------------------------------------------------------------

/// Determines where the HMAC signature is stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignatureEnvelope {
    /// Trailing comment line `# hmac-sha256: <hex>` at the end of `.agm.mem`.
    #[default]
    TrailingComment,
    /// Separate sidecar file `<path>.sig` containing the hex digest only.
    SidecarFile,
}

// ---------------------------------------------------------------------------
// Core signing helpers
// ---------------------------------------------------------------------------

/// Normalize newlines in `body` to `\n` before signing/verifying.
fn normalize_newlines(body: &str) -> String {
    body.replace("\r\n", "\n")
}

/// Compute HMAC-SHA256 of `body` (after newline normalization) with `key`.
/// Returns the lowercase hex digest.
#[must_use]
pub fn sign(body: &str, key: &HmacKey) -> String {
    let normalized = normalize_newlines(body);
    let mut mac =
        HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC can accept keys of any length");
    mac.update(normalized.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verify that `sig` (hex) matches `body` signed with `key`.
///
/// Uses constant-time comparison internally (`Mac::verify_slice`).
#[must_use]
pub fn verify_signature(body: &str, sig: &str, key: &HmacKey) -> bool {
    let Ok(expected_bytes) = hex::decode(sig) else {
        return false;
    };
    let normalized = normalize_newlines(body);
    let mut mac =
        HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC can accept keys of any length");
    mac.update(normalized.as_bytes());
    mac.verify_slice(&expected_bytes).is_ok()
}

/// Check whether `body` matches any of the provided `keys`.
///
/// Returns `true` if at least one key produces a matching HMAC.
#[must_use]
pub fn verify_any(body: &str, sig: &str, keys: &[&HmacKey]) -> bool {
    keys.iter().any(|k| verify_signature(body, sig, k))
}

// ---------------------------------------------------------------------------
// TrailingComment envelope helpers
// ---------------------------------------------------------------------------

const TRAILING_SIG_PREFIX: &str = "# hmac-sha256: ";

/// Strip the trailing `# hmac-sha256: <hex>` line from `raw`, if present.
///
/// Returns `(body_without_sig, Some(hex_sig))` or `(raw.to_owned(), None)`.
#[must_use]
pub fn strip_trailing_signature(raw: &str) -> (String, Option<String>) {
    // Normalize to LF for processing.
    let normalized = normalize_newlines(raw);
    let trimmed = normalized.trim_end_matches('\n');

    // Walk backwards through lines to find the signature line.
    if let Some(last_newline) = trimmed.rfind('\n') {
        let last_line = &trimmed[last_newline + 1..];
        if let Some(hex) = last_line.strip_prefix(TRAILING_SIG_PREFIX) {
            let body = trimmed[..last_newline + 1].to_owned();
            return (body, Some(hex.to_owned()));
        }
    } else {
        // Single-line file (unlikely but handle it).
        if let Some(hex) = trimmed.strip_prefix(TRAILING_SIG_PREFIX) {
            return (String::new(), Some(hex.to_owned()));
        }
    }

    // No signature line found — return the normalized body without trailing newline stripped.
    (normalized, None)
}

/// Append the trailing signature line to `body`.
#[must_use]
pub fn append_trailing_signature(body: &str, sig: &str) -> String {
    format!("{body}{TRAILING_SIG_PREFIX}{sig}\n")
}

// ---------------------------------------------------------------------------
// Key-spec resolver (Step 11)
// ---------------------------------------------------------------------------

/// Resolve a key specification string to an [`HmacKey`].
///
/// Supported forms:
/// - `env:VAR_NAME` — reads a hex-encoded key from the environment variable.
/// - `file:/path/to/key.hex` — reads a hex-encoded key from a file.
/// - `hex:<literal>` — parses the literal hex string directly (for testing only).
/// - `generate` — generates a new 32-byte key, prints the hex to **stderr** once
///   with a warning that it will be lost if not captured, then returns the key.
///
/// # Errors
/// Returns [`MemoryStoreError::KeyUnavailable`] if the key cannot be resolved.
pub fn resolve_key(spec: &str) -> Result<HmacKey, MemoryStoreError> {
    if let Some(var_name) = spec.strip_prefix("env:") {
        HmacKey::from_env(var_name)
    } else if let Some(path) = spec.strip_prefix("file:") {
        HmacKey::from_file(path)
    } else if let Some(literal) = spec.strip_prefix("hex:") {
        let bytes = hex::decode(literal.trim()).map_err(|e| {
            MemoryStoreError::KeyUnavailable(format!("hex key spec contains invalid hex: {e}"))
        })?;
        Ok(HmacKey::from_bytes(bytes))
    } else if spec == "generate" {
        let key = HmacKey::generate();
        let hex_key = hex::encode(key.as_bytes());
        eprintln!(
            "WARNING: generated HMAC key (hex) — this is your only chance to capture it:\n{hex_key}"
        );
        Ok(key)
    } else {
        Err(MemoryStoreError::KeyUnavailable(format!(
            "unknown key spec `{spec}`; expected `env:VAR`, `file:/path`, `hex:<literal>`, or `generate`"
        )))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- RFC 4231 test vectors (HMAC-SHA256) --

    /// RFC 4231 Test Case 1
    /// Key  = 0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b (20 bytes)
    /// Data = "Hi There"
    /// HMAC = b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7
    #[test]
    fn test_rfc4231_vector_1() {
        let key = HmacKey::from_bytes(vec![0x0b; 20]);
        let data = "Hi There";
        let mut mac = HmacSha256::new_from_slice(key.as_bytes()).unwrap();
        mac.update(data.as_bytes());
        let result = hex::encode(mac.finalize().into_bytes());
        assert_eq!(
            result,
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    /// RFC 4231 Test Case 2
    /// Key  = "Jefe" (0x4a656665, 4 bytes)
    /// Data = "what do ya want for nothing?" (0x7768617420646f2079612077616e7420666f72206e6f7468696e673f)
    /// HMAC-SHA256 (verified against the hmac crate's constant-time implementation)
    #[test]
    fn test_rfc4231_vector_2() {
        let key = HmacKey::from_bytes(b"Jefe".to_vec());
        let data = "what do ya want for nothing?";
        let mut mac = HmacSha256::new_from_slice(key.as_bytes()).unwrap();
        mac.update(data.as_bytes());
        let result = hex::encode(mac.finalize().into_bytes());
        // Verify the result is stable and matches what the hmac crate computes.
        // Cross-checked against RFC 4231 §4.2 using the published hex data.
        assert_eq!(
            result,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    /// RFC 4231 Test Case 4 (truncated to 32-byte variant, full output)
    /// Key  = 0102030405060708090a0b0c0d0e0f10111213141516171819 (25 bytes)
    /// Data = 0xcd repeated 50 times
    /// HMAC = 82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b
    #[test]
    fn test_rfc4231_vector_4() {
        let key: Vec<u8> = (1u8..=25).collect();
        let data: Vec<u8> = vec![0xcd; 50];
        let mut mac = HmacSha256::new_from_slice(&key).unwrap();
        mac.update(&data);
        let result = hex::encode(mac.finalize().into_bytes());
        assert_eq!(
            result,
            "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b"
        );
    }

    // -- sign / verify helpers --

    #[test]
    fn test_sign_verify_happy() {
        let key = HmacKey::from_bytes(vec![0x42; 32]);
        let body = "# agm.mem: 1.0\n# package: test\n# updated_at: 2026-01-01T00:00:00Z\n";
        let sig = sign(body, &key);
        assert!(verify_signature(body, &sig, &key));
    }

    #[test]
    fn test_sign_verify_tamper_fails() {
        let key = HmacKey::from_bytes(vec![0x42; 32]);
        let body = "# agm.mem: 1.0\n# package: test\n# updated_at: 2026-01-01T00:00:00Z\n";
        let sig = sign(body, &key);
        let tampered = format!("{body}extra");
        assert!(!verify_signature(&tampered, &sig, &key));
    }

    #[test]
    fn test_sign_verify_wrong_key_fails() {
        let key_a = HmacKey::from_bytes(vec![0x01; 32]);
        let key_b = HmacKey::from_bytes(vec![0x02; 32]);
        let body = "# agm.mem: 1.0\n# package: test\n";
        let sig = sign(body, &key_a);
        assert!(!verify_signature(body, &sig, &key_b));
    }

    #[test]
    fn test_rotation_accepts_old_key() {
        let key_a = HmacKey::from_bytes(vec![0x01; 32]);
        let key_b = HmacKey::from_bytes(vec![0x02; 32]);
        let body = "# agm.mem: 1.0\n# package: test\n";
        // Signed with key_a
        let sig = sign(body, &key_a);
        // verify_any should accept both key_a and key_b (rotation)
        assert!(verify_any(body, &sig, &[&key_a, &key_b]));
        // But not key_b alone
        assert!(!verify_any(body, &sig, &[&key_b]));
    }

    #[test]
    fn test_strip_trailing_signature_roundtrip() {
        let body = "# agm.mem: 1.0\n# package: test\n";
        let key = HmacKey::from_bytes(vec![0x42; 32]);
        let sig = sign(body, &key);
        let with_sig = append_trailing_signature(body, &sig);
        let (stripped, found_sig) = strip_trailing_signature(&with_sig);
        assert_eq!(stripped, body);
        assert_eq!(found_sig.as_deref(), Some(sig.as_str()));
    }

    #[test]
    fn test_strip_trailing_signature_no_sig() {
        let raw = "# agm.mem: 1.0\n# package: test\n";
        let (body, sig) = strip_trailing_signature(raw);
        assert_eq!(body, raw);
        assert!(sig.is_none());
    }

    #[test]
    fn test_constant_time_verify() {
        // Smoke test: a bad sig does not panic and returns false.
        let key = HmacKey::from_bytes(vec![0x42; 32]);
        let body = "body content\n";
        assert!(!verify_signature(body, "notvalidhex!!!", &key));
        assert!(!verify_signature(body, "deadbeef", &key)); // wrong length
    }

    #[test]
    fn test_hmackey_from_env_happy() {
        let hex = hex::encode(vec![0x42u8; 32]);
        // SAFETY: single-threaded test environment.
        unsafe { std::env::set_var("_AGM_TEST_KEY_OK", &hex) };
        let key = HmacKey::from_env("_AGM_TEST_KEY_OK").expect("should parse");
        assert_eq!(key.as_bytes(), &[0x42u8; 32]);
        // SAFETY: single-threaded test environment.
        unsafe { std::env::remove_var("_AGM_TEST_KEY_OK") };
    }

    #[test]
    fn test_hmackey_from_env_missing_returns_err() {
        // SAFETY: single-threaded test environment.
        unsafe { std::env::remove_var("_AGM_TEST_KEY_MISSING_XYZ") };
        let result = HmacKey::from_env("_AGM_TEST_KEY_MISSING_XYZ");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("_AGM_TEST_KEY_MISSING_XYZ"));
    }

    #[test]
    fn test_hmackey_generate_produces_32_bytes() {
        let key = HmacKey::generate();
        assert_eq!(key.as_bytes().len(), 32);
    }

    #[test]
    fn test_resolve_key_env() {
        let hex = hex::encode(vec![0x11u8; 32]);
        // SAFETY: single-threaded test environment.
        unsafe { std::env::set_var("_AGM_TEST_RESOLVE_ENV", &hex) };
        let key = resolve_key("env:_AGM_TEST_RESOLVE_ENV").expect("should resolve");
        assert_eq!(key.as_bytes(), &[0x11u8; 32]);
        // SAFETY: single-threaded test environment.
        unsafe { std::env::remove_var("_AGM_TEST_RESOLVE_ENV") };
    }

    #[test]
    fn test_resolve_key_hex() {
        let raw = vec![0xABu8; 16];
        let spec = format!("hex:{}", hex::encode(&raw));
        let key = resolve_key(&spec).expect("should resolve");
        assert_eq!(key.as_bytes(), raw.as_slice());
    }

    #[test]
    fn test_resolve_key_unknown_spec() {
        let result = resolve_key("unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_hmackey_debug_does_not_leak_bytes() {
        let key = HmacKey::from_bytes(vec![0x42; 32]);
        let dbg = format!("{key:?}");
        assert!(dbg.contains("HmacKey(len=32)"));
        assert!(!dbg.contains("42"));
    }

    #[test]
    fn test_signing_mode_debug_does_not_leak_key() {
        let mode = SigningMode::Enabled {
            key: HmacKey::from_bytes(vec![0x42; 32]),
        };
        let dbg = format!("{mode:?}");
        assert!(dbg.contains("Enabled"));
        assert!(!dbg.contains("42"));
    }

    #[test]
    fn test_crlf_normalized_before_verify() {
        let key = HmacKey::from_bytes(vec![0x42; 32]);
        // Sign with LF body.
        let lf_body = "# agm.mem: 1.0\n# package: test\n";
        let sig = sign(lf_body, &key);
        // Verify with CRLF body — should still pass.
        let crlf_body = "# agm.mem: 1.0\r\n# package: test\r\n";
        assert!(verify_signature(crlf_body, &sig, &key));
    }
}
