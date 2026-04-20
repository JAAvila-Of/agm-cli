//! Storage SDK for `.agm.mem` sidecar files.
//!
//! This module provides a `MemoryStore` trait and a `FilesystemMemoryStore`
//! implementation backing the existing `.agm.mem` text format.
//!
//! ## Quickstart
//!
//! ```rust,no_run
//! use agm_core::memory::store::{FilesystemMemoryStore, FilesystemConfig, MemoryStore};
//!
//! let mut store = FilesystemMemoryStore::open("project.agm.mem", FilesystemConfig::default())
//!     .expect("failed to open store");
//! let entries = store.list(None).expect("failed to list");
//! println!("{} entries", entries.len());
//! ```

pub mod error;
pub mod filesystem;
pub mod merge;
pub mod signing;

pub use error::MemoryStoreError;
pub use filesystem::{FilesystemConfig, FilesystemMemoryStore};
pub use merge::{MergeOutcome, MergeStrategy};
pub use signing::{HmacKey, SignatureEnvelope, SigningMode, VerifyMode};

use crate::model::mem_file::{MemFile, MemFileEntry};

/// Trait for memory storage backends.
///
/// Implementors provide a typed SDK over `.agm.mem`-compatible storage.
pub trait MemoryStore {
    /// Load the current state from the backing store.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError`] on IO, parse, or signature errors.
    fn load(&self) -> Result<MemFile, MemoryStoreError>;

    /// Persist `mem` atomically, replacing the current contents.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError`] on IO or signing errors.
    fn save(&mut self, mem: &MemFile) -> Result<(), MemoryStoreError>;

    /// Insert or update a single entry.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError::ValueTooLarge`] if the value exceeds 32 KiB.
    fn upsert(&mut self, key: &str, entry: MemFileEntry) -> Result<(), MemoryStoreError>;

    /// Remove a single entry. Returns `true` if the key was present.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError`] on IO errors.
    fn delete(&mut self, key: &str) -> Result<bool, MemoryStoreError>;

    /// Fetch a single entry by key.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError`] on IO errors.
    fn get(&self, key: &str) -> Result<Option<MemFileEntry>, MemoryStoreError>;

    /// List entries, optionally filtered by topic.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError`] on IO errors.
    fn list(&self, topic: Option<&str>) -> Result<Vec<(String, MemFileEntry)>, MemoryStoreError>;

    /// Verify the on-disk signature (if signing is configured) without fully reloading.
    ///
    /// Returns `Ok(())` for stores without signing configuration and `VerifyMode::Permissive`.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError::SignatureMismatch`] or [`MemoryStoreError::SignatureMissing`].
    fn verify_signature(&self) -> Result<(), MemoryStoreError>;

    /// Merge `other` into the store according to `strategy`.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError::MergeConflict`] if strategy is `Reject` and conflicts exist.
    fn merge(
        &mut self,
        other: &MemFile,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome, MemoryStoreError>;
}
