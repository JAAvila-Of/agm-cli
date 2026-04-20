//! Integration tests for `agm_core::memory::store`.
//!
//! Run with `REGEN_FIXTURES=1 cargo test -p agm-core --test memory_store gen_fixtures -- --nocapture`
//! to regenerate the fixture files in `tests/fixtures/mem/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agm_core::memory::store::signing::{HmacKey, sign};
use agm_core::memory::store::{
    FilesystemConfig, FilesystemMemoryStore, MemoryStore, MemoryStoreError, MergeStrategy,
    SignatureEnvelope, SigningMode, VerifyMode,
};
use agm_core::model::mem_file::{MemFile, MemFileEntry};
use agm_core::model::memory::{MemoryScope, MemoryTtl};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mem")
}

fn key_a() -> HmacKey {
    HmacKey::from_bytes(vec![0x41u8; 32])
}

fn key_b() -> HmacKey {
    HmacKey::from_bytes(vec![0x42u8; 32])
}

fn make_entry(value: &str) -> MemFileEntry {
    MemFileEntry {
        topic: "test".to_owned(),
        scope: MemoryScope::Project,
        ttl: MemoryTtl::Permanent,
        value: value.to_owned(),
        created_at: "2026-04-20T00:00:00Z".to_owned(),
        updated_at: "2026-04-20T00:00:00Z".to_owned(),
    }
}

fn unsigned_config(tmp: &TempDir) -> (PathBuf, FilesystemConfig) {
    let path = tmp.path().join("store.agm.mem");
    let cfg = FilesystemConfig {
        package: "test.store".to_owned(),
        ..Default::default()
    };
    (path, cfg)
}

fn signed_config(tmp: &TempDir) -> (PathBuf, FilesystemConfig) {
    let path = tmp.path().join("signed.agm.mem");
    let cfg = FilesystemConfig {
        package: "test.store".to_owned(),
        signing: SigningMode::Enabled { key: key_a() },
        verify_mode: VerifyMode::Strict,
        ..Default::default()
    };
    (path, cfg)
}

fn sidecar_signed_config(tmp: &TempDir) -> (PathBuf, FilesystemConfig) {
    let path = tmp.path().join("sidecar.agm.mem");
    let cfg = FilesystemConfig {
        package: "test.store".to_owned(),
        signing: SigningMode::Enabled { key: key_a() },
        verify_mode: VerifyMode::Strict,
        envelope: SignatureEnvelope::SidecarFile,
        ..Default::default()
    };
    (path, cfg)
}

// ---------------------------------------------------------------------------
// Fixture generation (run with REGEN_FIXTURES=1)
// ---------------------------------------------------------------------------

/// Regenerate test fixture files. Run with:
/// `REGEN_FIXTURES=1 cargo test -p agm-core --test memory_store gen_fixtures -- --nocapture`
#[test]
fn gen_fixtures() {
    if std::env::var("REGEN_FIXTURES").is_err() {
        return; // Skip unless explicitly requested.
    }

    let dir = fixtures_dir();
    std::fs::create_dir_all(dir.join("keys")).unwrap();

    // Write key files.
    std::fs::write(
        dir.join("keys/test_key_a.hex"),
        format!("{}\n", hex::encode(key_a().as_bytes())),
    )
    .unwrap();
    std::fs::write(
        dir.join("keys/test_key_b.hex"),
        format!("{}\n", hex::encode(key_b().as_bytes())),
    )
    .unwrap();

    // Unsigned fixture.
    let unsigned_body = "# agm.mem: 1.0\n# package: test.fixtures\n# updated_at: 2026-04-20T00:00:00Z\n\nentry fixture.key\ntopic: test\nscope: project\nttl: permanent\nvalue: unsigned fixture value\ncreated_at: 2026-04-20T00:00:00Z\nupdated_at: 2026-04-20T00:00:00Z\n";
    std::fs::write(dir.join("unsigned.agm.mem"), unsigned_body).unwrap();

    // Signed valid fixture (trailing comment).
    let sig_a = sign(unsigned_body, &key_a());
    let signed_body = format!("{unsigned_body}# hmac-sha256: {sig_a}\n");
    std::fs::write(dir.join("signed_valid.agm.mem"), &signed_body).unwrap();

    // Tampered fixture: change one byte in the body.
    let tampered_body = signed_body.replace("unsigned fixture value", "TAMPERED fixture value");
    std::fs::write(dir.join("signed_tampered.agm.mem"), tampered_body).unwrap();

    println!("Fixtures written to {}", dir.display());
    println!("key_a sig: {sig_a}");
}

// ---------------------------------------------------------------------------
// Integration test: unsigned roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_unsigned_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let (path, cfg) = unsigned_config(&tmp);
    let mut store = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();
    store.upsert("k.one", make_entry("hello")).unwrap();
    store.upsert("k.two", make_entry("world")).unwrap();

    let loaded = FilesystemMemoryStore::open(&path, cfg).unwrap();
    let entries = loaded.list(None).unwrap();
    assert_eq!(entries.len(), 2);
    let keys: Vec<_> = entries.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"k.one"));
    assert!(keys.contains(&"k.two"));
}

// ---------------------------------------------------------------------------
// Integration test: signed roundtrip (TrailingComment)
// ---------------------------------------------------------------------------

#[test]
fn test_signed_roundtrip_trailing_comment() {
    let tmp = TempDir::new().unwrap();
    let (path, cfg) = signed_config(&tmp);
    let mut store = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();
    store
        .upsert("k.signed", make_entry("signed-value"))
        .unwrap();

    // Re-open — must verify successfully.
    let store2 = FilesystemMemoryStore::open(&path, cfg).unwrap();
    let v = store2.get("k.signed").unwrap().unwrap();
    assert_eq!(v.value, "signed-value");
}

// ---------------------------------------------------------------------------
// Integration test: signed roundtrip (SidecarFile)
// ---------------------------------------------------------------------------

#[test]
fn test_signed_roundtrip_sidecar_file() {
    let tmp = TempDir::new().unwrap();
    let (path, cfg) = sidecar_signed_config(&tmp);
    let mut store = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();
    store
        .upsert("k.sidecar", make_entry("sidecar-value"))
        .unwrap();

    let sig_path = {
        let mut p = path.as_os_str().to_owned();
        p.push(".sig");
        PathBuf::from(p)
    };
    assert!(sig_path.exists(), "sig file should exist");

    let store2 = FilesystemMemoryStore::open(&path, cfg).unwrap();
    let v = store2.get("k.sidecar").unwrap().unwrap();
    assert_eq!(v.value, "sidecar-value");
}

// ---------------------------------------------------------------------------
// Integration test: tamper detection
// ---------------------------------------------------------------------------

#[test]
fn test_tampering_detected() {
    let tmp = TempDir::new().unwrap();
    let (path, cfg) = signed_config(&tmp);
    let mut store = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();
    store.upsert("k.tamper", make_entry("original")).unwrap();

    // Tamper the file by replacing one byte.
    let raw = std::fs::read_to_string(&path).unwrap();
    let tampered = raw.replace("original", "TAMPERED");
    std::fs::write(&path, tampered).unwrap();

    let result = FilesystemMemoryStore::open(&path, cfg);
    assert!(
        matches!(result, Err(MemoryStoreError::SignatureMismatch)),
        "expected SignatureMismatch, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Integration test: missing signature + Strict → error
// ---------------------------------------------------------------------------

#[test]
fn test_missing_signature_strict_fails() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("unsigned.agm.mem");

    // Write an unsigned file.
    let unsigned_cfg = FilesystemConfig {
        package: "test".to_owned(),
        ..Default::default()
    };
    let mut unsigned_store = FilesystemMemoryStore::open(&path, unsigned_cfg).unwrap();
    unsigned_store.upsert("k.u", make_entry("val")).unwrap();

    // Re-open with Strict verify — should fail with SignatureMissing.
    let strict_cfg = FilesystemConfig {
        package: "test".to_owned(),
        signing: SigningMode::Enabled { key: key_a() },
        verify_mode: VerifyMode::Strict,
        ..Default::default()
    };
    let result = FilesystemMemoryStore::open(&path, strict_cfg);
    assert!(
        matches!(result, Err(MemoryStoreError::SignatureMissing)),
        "expected SignatureMissing, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Integration test: missing signature + Permissive → ok
// ---------------------------------------------------------------------------

#[test]
fn test_missing_signature_permissive_ok() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("unsigned.agm.mem");

    let cfg = FilesystemConfig {
        package: "test".to_owned(),
        ..Default::default()
    };
    let mut store = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();
    store.upsert("k.u", make_entry("val")).unwrap();

    // Re-open with Permissive — should be fine.
    let store2 = FilesystemMemoryStore::open(&path, cfg).unwrap();
    assert!(store2.get("k.u").unwrap().is_some());
}

// ---------------------------------------------------------------------------
// Integration test: key rotation
// ---------------------------------------------------------------------------

#[test]
fn test_rotation_accepts_old_key() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("rotation.agm.mem");

    // Sign with key_a.
    let cfg_a = FilesystemConfig {
        package: "test".to_owned(),
        signing: SigningMode::Enabled { key: key_a() },
        verify_mode: VerifyMode::Strict,
        ..Default::default()
    };
    let mut store = FilesystemMemoryStore::open(&path, cfg_a).unwrap();
    store.upsert("k.r", make_entry("rotating")).unwrap();

    // Verify with rotation config (active=key_b, accepted=[key_a]).
    let cfg_rotation = FilesystemConfig {
        package: "test".to_owned(),
        signing: SigningMode::EnabledWithRotation {
            active: key_b(),
            accepted: vec![key_a()],
        },
        verify_mode: VerifyMode::Strict,
        ..Default::default()
    };
    let store2 = FilesystemMemoryStore::open(&path, cfg_rotation).unwrap();
    assert_eq!(store2.get("k.r").unwrap().unwrap().value, "rotating");

    // Verify with key_b alone — should fail (file is signed with key_a).
    let cfg_b_only = FilesystemConfig {
        package: "test".to_owned(),
        signing: SigningMode::Enabled { key: key_b() },
        verify_mode: VerifyMode::Strict,
        ..Default::default()
    };
    let result = FilesystemMemoryStore::open(&path, cfg_b_only);
    assert!(matches!(result, Err(MemoryStoreError::SignatureMismatch)));
}

// ---------------------------------------------------------------------------
// Integration test: merge strategies
// ---------------------------------------------------------------------------

fn make_mem_with_entries(entries: &[(&str, &str)]) -> MemFile {
    let mut map = BTreeMap::new();
    for (k, v) in entries {
        map.insert((*k).to_owned(), make_entry(v));
    }
    MemFile {
        format_version: "1.0".to_owned(),
        package: "test.merge".to_owned(),
        updated_at: "2026-04-20T00:00:00Z".to_owned(),
        entries: map,
    }
}

#[test]
fn test_merge_latest_wins_overwrites() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("merge.agm.mem");
    let cfg = FilesystemConfig {
        package: "test".to_owned(),
        ..Default::default()
    };

    let mut store = FilesystemMemoryStore::open(&path, cfg).unwrap();
    store.upsert("k.a", make_entry("original")).unwrap();

    let other = make_mem_with_entries(&[("k.a", "updated"), ("k.b", "new")]);
    let outcome = store.merge(&other, MergeStrategy::LatestWins).unwrap();

    assert_eq!(outcome.inserted, 1);
    assert_eq!(outcome.updated, 1);
    assert_eq!(store.get("k.a").unwrap().unwrap().value, "updated");
    assert_eq!(store.get("k.b").unwrap().unwrap().value, "new");
}

#[test]
fn test_merge_union_keeps_existing() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("merge_union.agm.mem");
    let cfg = FilesystemConfig {
        package: "test".to_owned(),
        ..Default::default()
    };

    let mut store = FilesystemMemoryStore::open(&path, cfg).unwrap();
    store.upsert("k.a", make_entry("keep-me")).unwrap();

    let other = make_mem_with_entries(&[("k.a", "ignored"), ("k.b", "inserted")]);
    let outcome = store.merge(&other, MergeStrategy::Union).unwrap();

    assert_eq!(outcome.inserted, 1);
    assert_eq!(outcome.unchanged, 1);
    // Existing value is preserved.
    assert_eq!(store.get("k.a").unwrap().unwrap().value, "keep-me");
    assert_eq!(store.get("k.b").unwrap().unwrap().value, "inserted");
}

#[test]
fn test_merge_reject_on_conflict() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("merge_reject.agm.mem");
    let cfg = FilesystemConfig {
        package: "test".to_owned(),
        ..Default::default()
    };

    let mut store = FilesystemMemoryStore::open(&path, cfg).unwrap();
    store.upsert("k.conflict", make_entry("existing")).unwrap();

    let other = make_mem_with_entries(&[("k.conflict", "new"), ("k.safe", "ok")]);
    let result = store.merge(&other, MergeStrategy::Reject);

    assert!(matches!(result, Err(MemoryStoreError::MergeConflict(_))));
    // Existing entry must be unchanged.
    assert_eq!(store.get("k.conflict").unwrap().unwrap().value, "existing");
}

// ---------------------------------------------------------------------------
// Integration test: large value → ValueTooLarge
// ---------------------------------------------------------------------------

#[test]
fn test_value_too_large_error() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("large.agm.mem");
    let cfg = FilesystemConfig {
        package: "test".to_owned(),
        ..Default::default()
    };
    let mut store = FilesystemMemoryStore::open(&path, cfg).unwrap();

    let huge = "x".repeat(32_769); // exceeds 32 KiB
    let result = store.upsert("k.large", make_entry(&huge));
    assert!(matches!(result, Err(MemoryStoreError::ValueTooLarge(_))));
}

// ---------------------------------------------------------------------------
// Integration test: concurrent-ish last-writer-wins, no corruption
// ---------------------------------------------------------------------------

#[test]
fn test_two_stores_no_corruption() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("concurrent.agm.mem");
    let cfg = FilesystemConfig {
        package: "test".to_owned(),
        ..Default::default()
    };

    let mut store_a = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();
    let mut store_b = FilesystemMemoryStore::open(&path, cfg.clone()).unwrap();

    // Both write different keys alternately.
    store_a.upsert("k.a1", make_entry("a1")).unwrap();
    store_b.upsert("k.b1", make_entry("b1")).unwrap();
    store_a.upsert("k.a2", make_entry("a2")).unwrap();
    store_b.upsert("k.b2", make_entry("b2")).unwrap();

    // Final reader should see a consistent file (no corruption / partial write).
    let reader = FilesystemMemoryStore::open(&path, cfg).unwrap();
    let all = reader.list(None).unwrap();
    // At minimum the last-writer's writes should be present.
    assert!(!all.is_empty(), "store should not be empty");
    // No panics or IO errors is the key contract.
}

// ---------------------------------------------------------------------------
// Integration test: fixture files (signed_valid, signed_tampered, unsigned)
// ---------------------------------------------------------------------------

#[test]
fn test_fixture_unsigned_loads_ok() {
    let fixture = fixtures_dir().join("unsigned.agm.mem");
    if !fixture.exists() {
        eprintln!("SKIP: unsigned fixture not found (run with REGEN_FIXTURES=1 first)");
        return;
    }
    let cfg = FilesystemConfig {
        package: "test.fixtures".to_owned(),
        ..Default::default()
    };
    let store = FilesystemMemoryStore::open(&fixture, cfg).unwrap();
    assert!(store.get("fixture.key").unwrap().is_some());
}

#[test]
fn test_fixture_signed_valid_verifies() {
    let fixture = fixtures_dir().join("signed_valid.agm.mem");
    if !fixture.exists() {
        eprintln!("SKIP: signed_valid fixture not found (run with REGEN_FIXTURES=1 first)");
        return;
    }
    let cfg = FilesystemConfig {
        package: "test.fixtures".to_owned(),
        signing: SigningMode::Enabled { key: key_a() },
        verify_mode: VerifyMode::Strict,
        ..Default::default()
    };
    let store = FilesystemMemoryStore::open(&fixture, cfg);
    assert!(
        store.is_ok(),
        "signed_valid fixture should verify: {store:?}"
    );
}

#[test]
fn test_fixture_signed_tampered_fails() {
    let fixture = fixtures_dir().join("signed_tampered.agm.mem");
    if !fixture.exists() {
        eprintln!("SKIP: signed_tampered fixture not found (run with REGEN_FIXTURES=1 first)");
        return;
    }
    let cfg = FilesystemConfig {
        package: "test.fixtures".to_owned(),
        signing: SigningMode::Enabled { key: key_a() },
        verify_mode: VerifyMode::Strict,
        ..Default::default()
    };
    let result = FilesystemMemoryStore::open(&fixture, cfg);
    assert!(
        matches!(result, Err(MemoryStoreError::SignatureMismatch)),
        "tampered fixture should fail: {result:?}"
    );
}
