//! Error types for the memory storage SDK.

/// Errors that can occur in memory store operations.
#[derive(Debug, thiserror::Error)]
pub enum MemoryStoreError {
    /// An IO error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The sidecar file could not be parsed.
    #[error("invalid sidecar: {0}")]
    Parse(String),

    /// The HMAC signature does not match the file contents.
    #[error("signature mismatch: file has been tampered with or the key is wrong")]
    SignatureMismatch,

    /// A signature was required but not found.
    #[error("signature required but missing (store configured with VerifyMode::Strict)")]
    SignatureMissing,

    /// The HMAC key could not be obtained.
    #[error("HMAC key not available: {0}")]
    KeyUnavailable(String),

    /// A merge conflict occurred with the `Reject` strategy.
    #[error("merge conflict on key `{0}` (strategy: Reject)")]
    MergeConflict(String),

    /// A memory entry value exceeds the 32 KiB limit.
    #[error("entry size exceeds 32 KiB for key `{0}`")]
    ValueTooLarge(String),
}
