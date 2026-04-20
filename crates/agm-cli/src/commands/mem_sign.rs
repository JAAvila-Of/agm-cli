//! `agm mem sign` — compute HMAC-SHA256 signature for an existing `.agm.mem` file.

use std::path::Path;

use agm_core::memory::store::signing::resolve_key;
use agm_core::memory::store::{
    FilesystemConfig, FilesystemMemoryStore, MemoryStore as _, SignatureEnvelope, SigningMode,
    VerifyMode,
};

use super::helpers;

/// Sign an existing `.agm.mem` file with the given key spec.
///
/// The key spec is one of:
/// - `env:VAR_NAME`   — reads hex key from environment variable
/// - `file:/path`     — reads hex key from file
/// - `hex:<literal>`  — uses literal hex key (testing only)
/// - `generate`       — generates a fresh 32-byte key; hex is printed to **stderr** once
///
/// Writes the signature into the chosen envelope (default: trailing comment).
pub fn run(file: &Path, key_spec: &str, envelope: SignatureEnvelope) -> i32 {
    // Resolve the key (may print to stderr for `generate`).
    let key = match resolve_key(key_spec) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("error: {e}");
            return helpers::EXIT_VALIDATION_ERROR;
        }
    };

    if !file.exists() {
        eprintln!("error: file not found: {}", file.display());
        return helpers::EXIT_IO_ERROR;
    }

    // Derive the package name from the file by reading its header.
    let package = read_package_from_file(file).unwrap_or_default();

    let cfg = FilesystemConfig {
        package,
        signing: SigningMode::Enabled { key },
        verify_mode: VerifyMode::Permissive,
        envelope,
        ..Default::default()
    };

    // Open the file (reads and verifies per Permissive mode = no verification).
    let mut store = match FilesystemMemoryStore::open(file, cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot open file: {e}");
            return helpers::EXIT_IO_ERROR;
        }
    };

    // Load and re-save with signing applied.
    let mem = match store.load() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: cannot load file: {e}");
            return helpers::EXIT_IO_ERROR;
        }
    };

    if let Err(e) = store.save(&mem) {
        eprintln!("error: cannot write signed file: {e}");
        return helpers::EXIT_IO_ERROR;
    }

    println!("Signed: {}", file.display());
    helpers::EXIT_SUCCESS
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
