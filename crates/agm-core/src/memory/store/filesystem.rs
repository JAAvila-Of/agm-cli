//! Filesystem-backed `.agm.mem` store.

use std::path::{Path, PathBuf};

use crate::memory::schema::MAX_MEMORY_VALUE_BYTES;
use crate::model::mem_file::{MemFile, MemFileEntry};
use crate::parser::mem::parse_mem;
use crate::renderer::mem::render_mem;

use super::MemoryStore;
use super::error::MemoryStoreError;
use super::merge::{MergeOutcome, MergeStrategy};
use super::signing::{
    HmacKey, SignatureEnvelope, SigningMode, VerifyMode, append_trailing_signature, sign,
    strip_trailing_signature, verify_any, verify_signature,
};

// ---------------------------------------------------------------------------
// FilesystemConfig
// ---------------------------------------------------------------------------

/// Configuration for [`FilesystemMemoryStore`].
#[derive(Debug, Clone)]
pub struct FilesystemConfig {
    /// Package identifier written into newly created sidecars.
    pub package: String,
    /// Signing configuration.
    pub signing: SigningMode,
    /// How to treat signatures on load.
    pub verify_mode: VerifyMode,
    /// Format version for newly created sidecars (default `"1.0"`).
    pub format_version: String,
    /// Signature envelope strategy.
    pub envelope: SignatureEnvelope,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            package: String::new(),
            signing: SigningMode::Disabled,
            verify_mode: VerifyMode::Permissive,
            format_version: "1.0".to_owned(),
            envelope: SignatureEnvelope::TrailingComment,
        }
    }
}

// ---------------------------------------------------------------------------
// FilesystemMemoryStore
// ---------------------------------------------------------------------------

/// Filesystem-backed `.agm.mem` store.
///
/// Writes are atomic: content is written to a temporary file in the same
/// directory, then renamed over the target — avoiding partial writes.
pub struct FilesystemMemoryStore {
    path: PathBuf,
    config: FilesystemConfig,
    cache: MemFile,
}

impl std::fmt::Debug for FilesystemMemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilesystemMemoryStore")
            .field("path", &self.path)
            .field("config", &self.config)
            .finish()
    }
}

impl FilesystemMemoryStore {
    /// Open a store at `path`.
    ///
    /// If the file does not exist and `config.package` is non-empty, an empty
    /// `MemFile` is created and persisted immediately.
    /// If the file exists, it is read, parsed, and signature-verified per
    /// `config.verify_mode`.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError`] on IO, parse, or signature errors.
    pub fn open<P: AsRef<Path>>(
        path: P,
        config: FilesystemConfig,
    ) -> Result<Self, MemoryStoreError> {
        let path = path.as_ref().to_path_buf();

        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            let (body, provided_sig) = extract_body_and_sig(&raw, &config);
            let mem = parse_mem_body(&body)?;
            verify_on_load(&body, provided_sig.as_deref(), &config, &path)?;
            Ok(Self {
                path,
                config,
                cache: mem,
            })
        } else {
            // File missing — create empty store.
            let mem = empty_mem_file(&config.package, &config.format_version);
            let mut store = Self {
                path,
                config,
                cache: mem,
            };
            if !store.config.package.is_empty() {
                store.save(&store.cache.clone())?;
            }
            Ok(store)
        }
    }

    /// Returns the path to the sidecar.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the path to the external signature file, if envelope is `SidecarFile`.
    #[must_use]
    pub fn signature_path(&self) -> Option<PathBuf> {
        if self.config.envelope == SignatureEnvelope::SidecarFile {
            Some(sig_path(&self.path))
        } else {
            None
        }
    }

    /// Verify the on-disk signature without fully reloading.
    ///
    /// Reads the raw bytes, strips the body, then validates against the
    /// configured key(s). Returns `Ok(())` if valid, `Err(...)` on failure.
    /// No-op (returns `Ok(())`) for stores with `SigningMode::Disabled` and
    /// `VerifyMode::Permissive`.
    ///
    /// # Errors
    /// Returns [`MemoryStoreError::SignatureMismatch`] or [`MemoryStoreError::SignatureMissing`].
    pub fn check_signature_on_disk(&self) -> Result<(), MemoryStoreError> {
        if matches!(self.config.verify_mode, VerifyMode::Permissive)
            && matches!(self.config.signing, SigningMode::Disabled)
        {
            return Ok(());
        }

        if !self.path.exists() {
            if matches!(self.config.verify_mode, VerifyMode::Strict) {
                return Err(MemoryStoreError::SignatureMissing);
            }
            return Ok(());
        }

        let raw = std::fs::read_to_string(&self.path)?;
        let (body, provided_sig) = extract_body_and_sig(&raw, &self.config);
        verify_on_load(&body, provided_sig.as_deref(), &self.config, &self.path)
    }
}

// ---------------------------------------------------------------------------
// MemoryStore trait impl
// ---------------------------------------------------------------------------

impl MemoryStore for FilesystemMemoryStore {
    fn load(&self) -> Result<MemFile, MemoryStoreError> {
        if !self.path.exists() {
            return Ok(empty_mem_file(
                &self.config.package,
                &self.config.format_version,
            ));
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let (body, provided_sig) = extract_body_and_sig(&raw, &self.config);
        let mem = parse_mem_body(&body)?;
        verify_on_load(&body, provided_sig.as_deref(), &self.config, &self.path)?;
        Ok(mem)
    }

    fn save(&mut self, mem: &MemFile) -> Result<(), MemoryStoreError> {
        let body = render_mem(mem);
        let final_content = apply_signing(&body, &self.config, &self.path)?;
        atomic_write(&self.path, &final_content)?;
        self.cache = mem.clone();
        Ok(())
    }

    fn upsert(&mut self, key: &str, entry: MemFileEntry) -> Result<(), MemoryStoreError> {
        if entry.value.len() > MAX_MEMORY_VALUE_BYTES {
            return Err(MemoryStoreError::ValueTooLarge(key.to_owned()));
        }
        self.cache.entries.insert(key.to_owned(), entry);
        let snapshot = self.cache.clone();
        self.save(&snapshot)
    }

    fn delete(&mut self, key: &str) -> Result<bool, MemoryStoreError> {
        let present = self.cache.entries.remove(key).is_some();
        if present {
            let snapshot = self.cache.clone();
            self.save(&snapshot)?;
        }
        Ok(present)
    }

    fn get(&self, key: &str) -> Result<Option<MemFileEntry>, MemoryStoreError> {
        Ok(self.cache.entries.get(key).cloned())
    }

    fn list(&self, topic: Option<&str>) -> Result<Vec<(String, MemFileEntry)>, MemoryStoreError> {
        let entries = self
            .cache
            .entries
            .iter()
            .filter(|(_, e)| topic.is_none_or(|t| e.topic == t))
            .map(|(k, e)| (k.clone(), e.clone()))
            .collect();
        Ok(entries)
    }

    fn verify_signature(&self) -> Result<(), MemoryStoreError> {
        self.check_signature_on_disk()
    }

    fn merge(
        &mut self,
        other: &MemFile,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome, MemoryStoreError> {
        let mut outcome = MergeOutcome::default();

        match strategy {
            MergeStrategy::LatestWins => {
                for (key, entry) in &other.entries {
                    if self.cache.entries.contains_key(key) {
                        outcome.updated += 1;
                    } else {
                        outcome.inserted += 1;
                    }
                    if entry.value.len() > MAX_MEMORY_VALUE_BYTES {
                        return Err(MemoryStoreError::ValueTooLarge(key.clone()));
                    }
                    self.cache.entries.insert(key.clone(), entry.clone());
                }
            }
            MergeStrategy::Union => {
                for (key, entry) in &other.entries {
                    if self.cache.entries.contains_key(key) {
                        outcome.unchanged += 1;
                    } else {
                        if entry.value.len() > MAX_MEMORY_VALUE_BYTES {
                            return Err(MemoryStoreError::ValueTooLarge(key.clone()));
                        }
                        self.cache.entries.insert(key.clone(), entry.clone());
                        outcome.inserted += 1;
                    }
                }
            }
            MergeStrategy::Reject => {
                // Collect conflicts without applying any changes.
                for key in other.entries.keys() {
                    if self.cache.entries.contains_key(key) {
                        outcome.conflicts.push(key.clone());
                    }
                }
                if !outcome.conflicts.is_empty() {
                    return Err(MemoryStoreError::MergeConflict(
                        outcome.conflicts[0].clone(),
                    ));
                }
                // No conflicts — apply all.
                for (key, entry) in &other.entries {
                    if entry.value.len() > MAX_MEMORY_VALUE_BYTES {
                        return Err(MemoryStoreError::ValueTooLarge(key.clone()));
                    }
                    self.cache.entries.insert(key.clone(), entry.clone());
                    outcome.inserted += 1;
                }
            }
        }

        let snapshot = self.cache.clone();
        self.save(&snapshot)?;
        Ok(outcome)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn now_iso8601() -> String {
    // Use SystemTime for a simple RFC 3339 / ISO 8601 timestamp without external deps.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (y, mo, d, h, mi, s) = unix_secs_to_ymdhms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Minimal UTC date decomposition from Unix seconds (no external dep required).
fn unix_secs_to_ymdhms(mut secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let s = (secs % 60) as u32;
    secs /= 60;
    let mi = (secs % 60) as u32;
    secs /= 60;
    let h = (secs % 24) as u32;
    secs /= 24;
    // Days since 1970-01-01
    let (y, mo, d) = days_to_ymd(secs as u32);
    (y, mo, d, h, mi, s)
}

fn days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    // Proleptic Gregorian calendar.
    let mut y = 1970u32;
    loop {
        let leap = is_leap(y);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        y += 1;
    }
    let leap = is_leap(y);
    let months = [
        31u32,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mo = 1u32;
    for &dim in &months {
        if days < dim {
            break;
        }
        days -= dim;
        mo += 1;
    }
    (y, mo, days + 1)
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn empty_mem_file(package: &str, format_version: &str) -> MemFile {
    MemFile {
        format_version: format_version.to_owned(),
        package: package.to_owned(),
        updated_at: now_iso8601(),
        entries: std::collections::BTreeMap::new(),
    }
}

fn parse_mem_body(body: &str) -> Result<MemFile, MemoryStoreError> {
    parse_mem(body).map_err(|errors| {
        MemoryStoreError::Parse(
            errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        )
    })
}

/// Extract the signing body and provided signature from raw file contents,
/// according to the configured envelope.
fn extract_body_and_sig(raw: &str, config: &FilesystemConfig) -> (String, Option<String>) {
    match config.envelope {
        SignatureEnvelope::TrailingComment => strip_trailing_signature(raw),
        SignatureEnvelope::SidecarFile => {
            // Body is the full raw content (no inline signature).
            // We don't know the path here, so we return the sig as None;
            // the caller is responsible for reading the sidecar.
            (raw.to_owned(), None)
        }
    }
}

/// Read the external signature file for `SidecarFile` envelope.
fn read_sig_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
}

fn sig_path(mem_path: &Path) -> PathBuf {
    let mut s = mem_path.as_os_str().to_owned();
    s.push(".sig");
    PathBuf::from(s)
}

/// Verify the signature according to `config.verify_mode`.
fn verify_on_load(
    body: &str,
    provided_sig: Option<&str>,
    config: &FilesystemConfig,
    path: &Path,
) -> Result<(), MemoryStoreError> {
    // For SidecarFile envelope, the provided_sig from extract_body_and_sig will be None;
    // we need to read it from disk.
    let sig: Option<String> = match config.envelope {
        SignatureEnvelope::TrailingComment => provided_sig.map(|s| s.to_owned()),
        SignatureEnvelope::SidecarFile => read_sig_file(&sig_path(path)),
    };

    match config.verify_mode {
        VerifyMode::Permissive => Ok(()),
        VerifyMode::IfPresent => {
            if let Some(hex_sig) = &sig {
                check_sig(body, hex_sig, &config.signing)
            } else {
                Ok(())
            }
        }
        VerifyMode::Strict => {
            let hex_sig = sig.ok_or(MemoryStoreError::SignatureMissing)?;
            check_sig(body, &hex_sig, &config.signing)
        }
    }
}

fn check_sig(body: &str, hex_sig: &str, mode: &SigningMode) -> Result<(), MemoryStoreError> {
    let ok = match mode {
        SigningMode::Disabled => {
            // No key configured — we can't verify; treat as mismatch.
            false
        }
        SigningMode::Enabled { key } => verify_signature(body, hex_sig, key),
        SigningMode::EnabledWithRotation { active, accepted } => {
            let all: Vec<&HmacKey> = std::iter::once(active).chain(accepted.iter()).collect();
            verify_any(body, hex_sig, &all)
        }
    };

    if ok {
        Ok(())
    } else {
        Err(MemoryStoreError::SignatureMismatch)
    }
}

/// Sign `body` and return the final file content (with signature appended or
/// write sidecar).
fn apply_signing(
    body: &str,
    config: &FilesystemConfig,
    path: &Path,
) -> Result<String, MemoryStoreError> {
    let key = match &config.signing {
        SigningMode::Disabled => return Ok(body.to_owned()),
        SigningMode::Enabled { key } => key,
        SigningMode::EnabledWithRotation { active, .. } => active,
    };

    let hex_sig = sign(body, key);

    match config.envelope {
        SignatureEnvelope::TrailingComment => Ok(append_trailing_signature(body, &hex_sig)),
        SignatureEnvelope::SidecarFile => {
            // Write signature to sidecar file.
            let sp = sig_path(path);
            atomic_write_str(&sp, &format!("{hex_sig}\n"))?;
            Ok(body.to_owned())
        }
    }
}

/// Atomically write `content` to `path` via a temp file in the same directory.
fn atomic_write(path: &Path, content: &str) -> Result<(), MemoryStoreError> {
    atomic_write_str(path, content)
}

fn atomic_write_str(path: &Path, content: &str) -> Result<(), MemoryStoreError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    use std::io::Write as _;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    // On Windows, `persist` uses ReplaceFile semantics when possible.
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::memory::{MemoryScope, MemoryTtl};
    use tempfile::TempDir;

    fn test_config(package: &str) -> FilesystemConfig {
        FilesystemConfig {
            package: package.to_owned(),
            ..Default::default()
        }
    }

    fn make_entry(value: &str) -> MemFileEntry {
        MemFileEntry {
            topic: "test".to_owned(),
            scope: MemoryScope::Project,
            ttl: MemoryTtl::Permanent,
            value: value.to_owned(),
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    fn signed_config(package: &str) -> FilesystemConfig {
        FilesystemConfig {
            package: package.to_owned(),
            signing: SigningMode::Enabled {
                key: HmacKey::from_bytes(vec![0x42; 32]),
            },
            verify_mode: VerifyMode::Strict,
            ..Default::default()
        }
    }

    #[test]
    fn test_open_creates_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("new.agm.mem");
        assert!(!path.exists());
        let store = FilesystemMemoryStore::open(&path, test_config("test.pkg")).unwrap();
        assert!(path.exists());
        assert_eq!(store.cache.package, "test.pkg");
    }

    #[test]
    fn test_save_load_roundtrip_unsigned() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store.agm.mem");
        let mut store = FilesystemMemoryStore::open(&path, test_config("test.pkg")).unwrap();
        store.upsert("k.one", make_entry("hello")).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.entries["k.one"].value, "hello");
    }

    #[test]
    fn test_save_load_roundtrip_signed_trailing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("signed.agm.mem");
        let mut store = FilesystemMemoryStore::open(&path, signed_config("test.pkg")).unwrap();
        store.upsert("k.two", make_entry("world")).unwrap();

        // Re-open with same key; should pass strict verify.
        let store2 = FilesystemMemoryStore::open(&path, signed_config("test.pkg")).unwrap();
        assert_eq!(store2.cache.entries["k.two"].value, "world");
    }

    #[test]
    fn test_save_load_roundtrip_signed_sidecar() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sidecar.agm.mem");
        let cfg = FilesystemConfig {
            package: "test.pkg".to_owned(),
            signing: SigningMode::Enabled {
                key: HmacKey::from_bytes(vec![0x42; 32]),
            },
            verify_mode: VerifyMode::Strict,
            envelope: SignatureEnvelope::SidecarFile,
            ..Default::default()
        };
        let mut store = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();
        store
            .upsert("k.three", make_entry("sidecar-value"))
            .unwrap();

        assert!(
            path.with_extension("mem.sig").exists()
                || dir.path().join("sidecar.agm.mem.sig").exists()
        );

        let store2 = FilesystemMemoryStore::open(&path, cfg).unwrap();
        assert_eq!(store2.cache.entries["k.three"].value, "sidecar-value");
    }

    #[test]
    fn test_upsert_then_delete() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("crud.agm.mem");
        let mut store = FilesystemMemoryStore::open(&path, test_config("test.pkg")).unwrap();
        store.upsert("k.del", make_entry("gone")).unwrap();
        assert!(store.get("k.del").unwrap().is_some());
        let removed = store.delete("k.del").unwrap();
        assert!(removed);
        assert!(store.get("k.del").unwrap().is_none());
    }

    #[test]
    fn test_list_filters_by_topic() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("list.agm.mem");
        let mut store = FilesystemMemoryStore::open(&path, test_config("test.pkg")).unwrap();

        let mut e1 = make_entry("v1");
        e1.topic = "alpha".to_owned();
        let mut e2 = make_entry("v2");
        e2.topic = "beta".to_owned();

        store.upsert("k.alpha", e1).unwrap();
        store.upsert("k.beta", e2).unwrap();

        let all = store.list(None).unwrap();
        assert_eq!(all.len(), 2);

        let alpha_only = store.list(Some("alpha")).unwrap();
        assert_eq!(alpha_only.len(), 1);
        assert_eq!(alpha_only[0].0, "k.alpha");
    }

    #[test]
    fn test_value_too_large() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("large.agm.mem");
        let mut store = FilesystemMemoryStore::open(&path, test_config("test.pkg")).unwrap();
        let huge_value = "x".repeat(MAX_MEMORY_VALUE_BYTES + 1);
        let entry = make_entry(&huge_value);
        let result = store.upsert("k.large", entry);
        assert!(matches!(result, Err(MemoryStoreError::ValueTooLarge(_))));
    }
}
