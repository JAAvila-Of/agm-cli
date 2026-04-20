//! CLI end-to-end tests for `agm mem sign` and `agm mem verify`.

use std::fs;

use assert_cmd::Command;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn agm() -> Command {
    Command::cargo_bin("agm").unwrap()
}

/// Write a minimal unsigned `.agm.mem` file to `path`.
fn write_unsigned(path: &std::path::Path) {
    fs::write(
        path,
        "# agm.mem: 1.0\n# package: e2e.test\n# updated_at: 2026-04-20T00:00:00Z\n\nentry e2e.key\ntopic: test\nscope: project\nttl: permanent\nvalue: hello world\ncreated_at: 2026-04-20T00:00:00Z\nupdated_at: 2026-04-20T00:00:00Z\n",
    )
    .unwrap();
}

/// Write a minimal valid `.agm` file to `path`.
fn write_minimal_agm(path: &std::path::Path) {
    fs::write(
        path,
        "agm: 1.0\npackage: e2e.test\nversion: 0.1.0\n\nnode smoke\ntype: facts\nsummary: smoke test\n",
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// Test: agm mem sign with --key hex:<literal>
// ---------------------------------------------------------------------------

#[test]
fn test_mem_sign_hex_key_writes_signature() {
    let tmp = TempDir::new().unwrap();
    let mem_file = tmp.path().join("test.agm.mem");
    write_unsigned(&mem_file);

    let hex_key = "4141414141414141414141414141414141414141414141414141414141414141";
    agm()
        .arg("mem")
        .arg("sign")
        .arg(&mem_file)
        .arg("--key")
        .arg(format!("hex:{hex_key}"))
        .assert()
        .success();

    let content = fs::read_to_string(&mem_file).unwrap();
    assert!(
        content.contains("# hmac-sha256: "),
        "signed file should contain trailing signature line"
    );
}

// ---------------------------------------------------------------------------
// Test: agm mem sign --key generate → prints key to stderr
// ---------------------------------------------------------------------------

#[test]
fn test_mem_sign_generate_key_prints_to_stderr() {
    let tmp = TempDir::new().unwrap();
    let mem_file = tmp.path().join("generate.agm.mem");
    write_unsigned(&mem_file);

    let output = agm()
        .arg("mem")
        .arg("sign")
        .arg(&mem_file)
        .arg("--key")
        .arg("generate")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "sign --key generate should succeed"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("WARNING") || stderr.contains("warning") || stderr.len() > 10,
        "generate should print warning to stderr"
    );

    // The file should now be signed.
    let content = fs::read_to_string(&mem_file).unwrap();
    assert!(content.contains("# hmac-sha256: "));
}

// ---------------------------------------------------------------------------
// Test: agm mem verify → exit 0 on valid signature (env var key)
// ---------------------------------------------------------------------------

#[test]
fn test_mem_verify_valid_signature_exit_0() {
    let tmp = TempDir::new().unwrap();
    let mem_file = tmp.path().join("valid.agm.mem");
    write_unsigned(&mem_file);

    let hex_key = "4141414141414141414141414141414141414141414141414141414141414141";

    // Sign first.
    agm()
        .arg("mem")
        .arg("sign")
        .arg(&mem_file)
        .arg("--key")
        .arg(format!("hex:{hex_key}"))
        .assert()
        .success();

    // Verify with the same key via env var.
    agm()
        .env("AGM_E2E_TEST_KEY", hex_key)
        .arg("mem")
        .arg("verify")
        .arg(&mem_file)
        .arg("--key")
        .arg("env:AGM_E2E_TEST_KEY")
        .assert()
        .success()
        .code(0);
}

// ---------------------------------------------------------------------------
// Test: agm mem verify → exit 1 on tampered file
// ---------------------------------------------------------------------------

#[test]
fn test_mem_verify_tampered_exit_1() {
    let tmp = TempDir::new().unwrap();
    let mem_file = tmp.path().join("tamper.agm.mem");
    write_unsigned(&mem_file);

    let hex_key = "4141414141414141414141414141414141414141414141414141414141414141";

    // Sign.
    agm()
        .arg("mem")
        .arg("sign")
        .arg(&mem_file)
        .arg("--key")
        .arg(format!("hex:{hex_key}"))
        .assert()
        .success();

    // Tamper: replace value.
    let content = fs::read_to_string(&mem_file).unwrap();
    let tampered = content.replace("hello world", "TAMPERED VALUE");
    fs::write(&mem_file, tampered).unwrap();

    // Verify should fail with exit code 1.
    agm()
        .arg("mem")
        .arg("verify")
        .arg(&mem_file)
        .arg("--key")
        .arg(format!("hex:{hex_key}"))
        .assert()
        .code(1);
}

// ---------------------------------------------------------------------------
// Test: agm mem verify on unsigned file + --verify-mode strict → exit 2
// ---------------------------------------------------------------------------

#[test]
fn test_mem_verify_unsigned_strict_exit_2() {
    let tmp = TempDir::new().unwrap();
    let mem_file = tmp.path().join("unsigned.agm.mem");
    write_unsigned(&mem_file);

    let hex_key = "4141414141414141414141414141414141414141414141414141414141414141";

    agm()
        .arg("mem")
        .arg("verify")
        .arg(&mem_file)
        .arg("--key")
        .arg(format!("hex:{hex_key}"))
        .arg("--verify-mode")
        .arg("strict")
        .assert()
        .code(2);
}

// ---------------------------------------------------------------------------
// Test: agm mem import with --sign and --strategy union
// ---------------------------------------------------------------------------

#[test]
fn test_mem_import_with_sign() {
    let tmp = TempDir::new().unwrap();
    let src_file = tmp.path().join("src.agm.mem");
    let dst_agm = tmp.path().join("project.agm");
    let dst_mem = tmp.path().join("project.agm.mem");

    // Create the destination AGM (just a stub; mem sidecar is what we care about).
    fs::write(&dst_agm, "").unwrap();

    // Source memory file.
    write_unsigned(&src_file);

    let hex_key = "4141414141414141414141414141414141414141414141414141414141414141";

    // Import with signing.
    agm()
        .arg("mem")
        .arg("import")
        .arg(&dst_agm)
        .arg("--from")
        .arg(&src_file)
        .arg("--strategy")
        .arg("union")
        .arg("--sign")
        .arg(format!("hex:{hex_key}"))
        .assert()
        .success();

    // The output file should be signed.
    assert!(dst_mem.exists(), "destination mem file should be created");
    let content = fs::read_to_string(&dst_mem).unwrap();
    assert!(
        content.contains("# hmac-sha256: "),
        "imported file should be signed"
    );

    // Verify the signed file.
    agm()
        .arg("mem")
        .arg("verify")
        .arg(&dst_mem)
        .arg("--key")
        .arg(format!("hex:{hex_key}"))
        .assert()
        .code(0);
}

// ---------------------------------------------------------------------------
// Test: existing mem_cmd list still works (backward compatibility smoke test)
// ---------------------------------------------------------------------------

#[test]
fn test_mem_list_still_works() {
    let tmp = TempDir::new().unwrap();
    let mem_file = tmp.path().join("smoke.agm.mem");
    write_unsigned(&mem_file);

    // The CLI expects the .agm path; mem sidecar is <agm_path>.mem
    let agm_path = tmp.path().join("smoke.agm");
    write_minimal_agm(&agm_path);

    agm()
        .arg("mem")
        .arg("list")
        .arg(&agm_path)
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Test: agm mem sign with sidecar-file envelope
// ---------------------------------------------------------------------------

#[test]
fn test_mem_sign_sidecar_file_envelope() {
    let tmp = TempDir::new().unwrap();
    let mem_file = tmp.path().join("sidecar.agm.mem");
    write_unsigned(&mem_file);

    let hex_key = "4141414141414141414141414141414141414141414141414141414141414141";

    agm()
        .arg("mem")
        .arg("sign")
        .arg(&mem_file)
        .arg("--key")
        .arg(format!("hex:{hex_key}"))
        .arg("--envelope")
        .arg("sidecar-file")
        .assert()
        .success();

    let sig_file = tmp.path().join("sidecar.agm.mem.sig");
    assert!(sig_file.exists(), "sidecar .sig file should be created");

    let body = fs::read_to_string(&mem_file).unwrap();
    assert!(
        !body.contains("# hmac-sha256: "),
        "body should NOT contain trailing signature when using sidecar-file envelope"
    );
}
