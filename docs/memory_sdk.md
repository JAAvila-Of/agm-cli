# Memory SDK — `.agm.mem` First-Class API

This document covers the `agm_core::memory::store` module: the `FilesystemMemoryStore` struct,
the `MemoryStore` trait, HMAC-SHA256 signing, merge strategies, and CLI commands.

---

## Overview

Every `.agm` project can carry a companion sidecar file `<project>.agm.mem` that stores structured
key/value memory entries. The memory SDK wraps raw sidecar files with:

- **Atomic writes** — no partial updates, even on Windows.
- **HMAC-SHA256 integrity** — optional, opt-in signing that prevents undetected tampering.
- **Flexible merge** — combine two stores with `LatestWins`, `Union`, or `Reject` strategies.
- **TTL-aware GC** — expired entries are pruned automatically on `gc()`.

---

## `MemoryStore` Trait

```rust
pub trait MemoryStore {
    fn get(&self, key: &str) -> Option<&MemFileEntry>;
    fn upsert(&mut self, key: &str, entry: MemFileEntry) -> Result<(), MemoryStoreError>;
    fn delete(&mut self, key: &str) -> bool;
    fn list(&self) -> Vec<(&str, &MemFileEntry)>;
    fn merge(
        &mut self,
        source: &MemFile,
        strategy: MergeStrategy,
    ) -> Result<MergeOutcome, MemoryStoreError>;
    fn save(&mut self, mem: &MemFile) -> Result<(), MemoryStoreError>;
    fn verify_signature(&self) -> Result<(), MemoryStoreError>;
}
```

---

## `FilesystemMemoryStore`

### Opening a Store

```rust
use agm_core::memory::store::{FilesystemConfig, FilesystemMemoryStore, SigningMode, VerifyMode};
use std::path::Path;

let cfg = FilesystemConfig {
    package: "my.package".to_string(),
    signing: SigningMode::Disabled,
    verify_mode: VerifyMode::Permissive,
    ..Default::default()
};
let store = FilesystemMemoryStore::open(Path::new("project.agm.mem"), cfg)?;
```

`open` creates the file if it does not exist. If a file already exists its content is parsed and
verified according to `verify_mode` (see below).

### `FilesystemConfig` Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `package` | `String` | `""` | Package name written to the `# package:` header |
| `signing` | `SigningMode` | `Disabled` | Whether and how to sign on save |
| `verify_mode` | `VerifyMode` | `Permissive` | What to do with signatures on open |
| `format_version` | `String` | `"1.0"` | Written to the `# agm.mem:` header |
| `envelope` | `SignatureEnvelope` | `TrailingComment` | Where the signature is stored |

---

## Signing Modes

### `SigningMode`

```rust
pub enum SigningMode {
    Disabled,
    Enabled { key: HmacKey },
    EnabledWithRotation { current: HmacKey, previous: HmacKey },
}
```

When `Enabled`, every `save()` call appends a trailing `# hmac-sha256: <hex>` comment to the
canonical `render_mem` output before writing.

When `EnabledWithRotation`, the `current` key is used for signing. Both `current` and `previous`
are tried during `verify_signature` — useful for zero-downtime key rotation.

### `VerifyMode`

```rust
pub enum VerifyMode {
    Permissive,   // no signature → OK, bad signature → error
    IfPresent,    // no signature → OK, bad signature → error (alias of Permissive)
    Strict,       // no signature → error (exit code 2), bad signature → error (exit code 1)
}
```

### `SignatureEnvelope`

```rust
pub enum SignatureEnvelope {
    TrailingComment,  // appends "# hmac-sha256: <hex>" at end of mem file (default)
    SidecarFile,      // writes signature to <mem_path>.sig beside the mem file
}
```

---

## Key Resolution

The CLI and SDK share a single `resolve_key(spec: &str) -> Result<HmacKey, MemoryStoreError>`
function. The `spec` string can be:

| Format | Description |
|--------|-------------|
| `env:VAR_NAME` | Read key bytes from environment variable (hex-encoded) |
| `file:/absolute/path` | Read hex-encoded key from a file |
| `hex:<64-char-hex>` | Inline literal 32-byte HMAC key |
| `generate` | Generate a random key; print it to stderr with a warning |

Example:

```bash
agm mem sign project.agm.mem --key env:AGM_SIGNING_KEY
agm mem sign project.agm.mem --key hex:4141...4141
agm mem sign project.agm.mem --key generate
```

---

## Merge Strategies

```rust
pub enum MergeStrategy {
    LatestWins,  // newer updated_at timestamp wins (default)
    Union,       // keep all entries; conflict → newer wins
    Reject,      // abort on any key conflict
}
```

`merge()` returns `MergeOutcome { inserted, updated, conflicts, unchanged }`.

Example:

```rust
use agm_core::memory::store::{FilesystemMemoryStore, MergeStrategy};

let outcome = dst_store.merge(&source_mem, MergeStrategy::Union)?;
println!("inserted={} updated={}", outcome.inserted, outcome.updated);
```

---

## Error Types

```rust
pub enum MemoryStoreError {
    Io(std::io::Error),
    Parse(Vec<agm_core::parser::Error>),
    SignatureMismatch,   // HMAC verification failed
    SignatureMissing,    // strict mode and no signature present
    KeyUnavailable(String),
    MergeConflict(String),
    ValueTooLarge { key: String, size: usize, limit: usize },
}
```

---

## CLI Commands

### `agm mem sign`

Sign an existing `.agm.mem` file in-place.

```
agm mem sign <MEM_FILE> --key <KEY_SPEC> [--envelope trailing-comment|sidecar-file]
```

Rewrites the file with an HMAC-SHA256 signature. Exits 0 on success.

### `agm mem verify`

Verify the integrity of a `.agm.mem` file.

```
agm mem verify <MEM_FILE> --key <KEY_SPEC>
    [--envelope trailing-comment|sidecar-file]
    [--verify-mode permissive|if-present|strict]
```

Exit codes:
- `0` — signature present and valid
- `1` — signature present but invalid (tampered)
- `2` — signature absent and `--verify-mode strict`
- `3` — other error (I/O, key resolution)

### `agm mem import` (updated)

```
agm mem import <AGM_FILE> --from <SOURCE.agm.mem>
    [--strategy latest-wins|union|reject]
    [--sign <KEY_SPEC>]
    [--envelope trailing-comment|sidecar-file]
```

Merges entries from the source sidecar into the project sidecar. Optionally signs the result.

### `agm mem export` (updated)

```
agm mem export <AGM_FILE> [--format json|agm] [--sign <KEY_SPEC>]
    [--envelope trailing-comment|sidecar-file]
```

Exports memory. When `--sign` is supplied and format is `agm`, writes the signed sidecar to disk
instead of printing to stdout.

---

## Atomic Writes

All file writes go through a `NamedTempFile` on the same directory as the target, followed by
`persist()`. On POSIX this is an atomic rename; on Windows it uses `MoveFileExW` with fallback.
This prevents partial writes even on power loss.

---

## Signature Algorithm

1. Render the `MemFile` to canonical text via `render_mem()`.
2. CRLF-normalize: replace every `\r\n` with `\n`.
3. Strip any existing trailing `# hmac-sha256: ...` line (re-signing is idempotent).
4. Compute `HMAC-SHA256(key, body_utf8_bytes)`.
5. Encode as lowercase hex.
6. Append `# hmac-sha256: <hex>` as a new final line.

Verification reverses steps 3–6 and uses constant-time comparison (`Mac::verify_slice`).

---

## Programmatic Example

```rust
use agm_core::memory::store::{
    FilesystemConfig, FilesystemMemoryStore, MemoryStore as _,
    MergeStrategy, SignatureEnvelope, SigningMode, VerifyMode,
};
use agm_core::memory::store::signing::resolve_key;
use std::path::Path;

// Build a signing key from environment.
let key = resolve_key("env:AGM_SIGNING_KEY")?;

// Open (or create) the sidecar.
let cfg = FilesystemConfig {
    package: "my.project".to_string(),
    signing: SigningMode::Enabled { key },
    verify_mode: VerifyMode::Strict,
    envelope: SignatureEnvelope::TrailingComment,
    ..Default::default()
};
let mut store = FilesystemMemoryStore::open(Path::new("project.agm.mem"), cfg)?;

// Upsert an entry.
use agm_core::model::mem_file::MemFileEntry;
use agm_core::model::memory::{MemoryScope, MemoryTtl};
store.upsert("my.key", MemFileEntry {
    value: "some important fact".to_string(),
    topic: "architecture".to_string(),
    scope: MemoryScope::Project,
    ttl: MemoryTtl::Permanent,
    created_at: "2026-01-01T00:00:00Z".to_string(),
    updated_at: "2026-01-01T00:00:00Z".to_string(),
})?;

// Flush to disk (signed).
let mem = store.snapshot(); // or build a MemFile from the store
store.save(&mem)?;

// Later: verify integrity.
store.verify_signature()?;
```

---

## Key Rotation

Use `SigningMode::EnabledWithRotation` to rotate keys without downtime:

1. Deploy with `EnabledWithRotation { current: new_key, previous: old_key }`.
2. All newly written files are signed with `new_key`.
3. Files signed with `old_key` still verify (both keys are tried).
4. Once all files have been re-signed, promote `new_key` to `Enabled { key: new_key }`.
