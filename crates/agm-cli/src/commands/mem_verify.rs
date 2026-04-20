//! `agm mem verify` — verify HMAC-SHA256 signature of an `.agm.mem` file.
//!
//! Exit codes:
//!   0 — signature is valid
//!   1 — signature mismatch (tampered or wrong key)
//!   2 — signature missing (under `--verify-mode strict`)
//!   3 — other error (IO, parse, key unavailable)

use std::path::Path;

use agm_core::memory::store::signing::resolve_key;
use agm_core::memory::store::{
    FilesystemConfig, FilesystemMemoryStore, MemoryStoreError, SignatureEnvelope, SigningMode,
    VerifyMode,
};

use super::helpers;

const EXIT_SIG_MISMATCH: i32 = 1;
const EXIT_SIG_MISSING: i32 = 2;
const EXIT_OTHER_ERROR: i32 = 3;

/// Verify the signature of `file` using `key_spec`.
pub fn run(
    file: &Path,
    key_spec: &str,
    envelope: SignatureEnvelope,
    verify_mode: VerifyMode,
) -> i32 {
    let key = match resolve_key(key_spec) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "hint: supply a key with --key env:VAR, --key file:/path, or --key hex:<literal>"
            );
            return EXIT_OTHER_ERROR;
        }
    };

    if !file.exists() {
        eprintln!("error: file not found: {}", file.display());
        return EXIT_OTHER_ERROR;
    }

    let package = read_package_from_file(file).unwrap_or_default();

    let cfg = FilesystemConfig {
        package,
        signing: SigningMode::Enabled { key },
        verify_mode,
        envelope,
        ..Default::default()
    };

    // Open the store — this reads and verifies per `verify_mode`.
    match FilesystemMemoryStore::open(file, cfg) {
        Ok(store) => {
            // Call check_signature_on_disk explicitly (belt-and-suspenders).
            match store.check_signature_on_disk() {
                Ok(()) => {
                    println!("OK: signature is valid");
                    helpers::EXIT_SUCCESS
                }
                Err(e) => map_store_error(&e),
            }
        }
        Err(e) => map_store_error(&e),
    }
}

fn map_store_error(e: &MemoryStoreError) -> i32 {
    match e {
        MemoryStoreError::SignatureMismatch => {
            eprintln!("error: signature mismatch: file has been tampered with or the key is wrong");
            eprintln!(
                "hint: verify the key used to sign this file; try --key file:/path/to/other.key"
            );
            EXIT_SIG_MISMATCH
        }
        MemoryStoreError::SignatureMissing => {
            eprintln!("error: signature required but missing");
            eprintln!("hint: sign the file with `agm mem sign <file> --key ...` first");
            EXIT_SIG_MISSING
        }
        other => {
            eprintln!("error: {other}");
            EXIT_OTHER_ERROR
        }
    }
}

fn read_package_from_file(file: &Path) -> Option<String> {
    let content = std::fs::read_to_string(file).ok()?;
    for line in content.lines() {
        if let Some(pkg) = line.strip_prefix("# package: ") {
            return Some(pkg.trim().to_owned());
        }
    }
    None
}
