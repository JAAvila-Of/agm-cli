//! CLI end-to-end tests for `agm diff`.

use assert_cmd::Command;

fn fixture_path(relative: &str) -> std::path::PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest)
        .join("../..")
        .join("tests/fixtures")
        .join(relative)
}

fn agm_cmd() -> Command {
    Command::cargo_bin("agm").expect("agm binary should exist")
}

#[test]
fn test_cli_diff_identical_files_exits_0() {
    let base = fixture_path("diff/identical/base.agm");
    agm_cmd()
        .args(["diff", base.to_str().unwrap(), base.to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}

#[test]
fn test_cli_diff_added_node_exits_1() {
    let left = fixture_path("diff/added_node/left.agm");
    let right = fixture_path("diff/added_node/right.agm");
    agm_cmd()
        .args(["diff", left.to_str().unwrap(), right.to_str().unwrap()])
        .assert()
        .code(1);
}

#[test]
fn test_cli_diff_breaking_change_exits_2() {
    let left = fixture_path("diff/breaking_change/left.agm");
    let right = fixture_path("diff/breaking_change/right.agm");
    agm_cmd()
        .args(["diff", left.to_str().unwrap(), right.to_str().unwrap()])
        .assert()
        .code(2);
}

#[test]
fn test_cli_diff_breaking_only_filters_output() {
    let left = fixture_path("diff/breaking_change/left.agm");
    let right = fixture_path("diff/breaking_change/right.agm");
    let output = agm_cmd()
        .args([
            "diff",
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            "--breaking-only",
        ])
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    // Should not contain any "(info)" or "(minor)" severity markers
    assert!(
        !stdout.contains("(info)"),
        "breaking-only output should not contain info changes"
    );
    assert!(
        !stdout.contains("(minor)"),
        "breaking-only output should not contain minor changes"
    );
}

#[test]
fn test_cli_diff_quiet_mode_no_output() {
    let left = fixture_path("diff/breaking_change/left.agm");
    let right = fixture_path("diff/breaking_change/right.agm");
    agm_cmd()
        .args([
            "diff",
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            "--quiet",
        ])
        .assert()
        .code(2)
        .stdout("");
}

#[test]
fn test_cli_diff_json_format_valid_json() {
    let left = fixture_path("diff/modified_fields/left.agm");
    let right = fixture_path("diff/modified_fields/right.agm");
    let output = agm_cmd()
        .args([
            "diff",
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    // Should be valid JSON that deserializes to DiffReport
    let _: agm_core::diff::DiffReport =
        serde_json::from_str(&stdout).expect("JSON output must deserialize to DiffReport");
}

#[test]
fn test_cli_diff_markdown_format() {
    let left = fixture_path("diff/header_change/left.agm");
    let right = fixture_path("diff/header_change/right.agm");
    let output = agm_cmd()
        .args([
            "diff",
            left.to_str().unwrap(),
            right.to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(
        stdout.contains("# AGM Semantic Diff"),
        "markdown output must contain the top-level header"
    );
    assert!(
        stdout.contains("## Summary"),
        "markdown output must contain Summary section"
    );
}

#[test]
fn test_cli_diff_nonexistent_file_exits_2() {
    agm_cmd()
        .args(["diff", "nonexistent.agm", "also_nonexistent.agm"])
        .assert()
        .code(2);
}

#[test]
fn test_cli_diff_invalid_agm_file_exits_1() {
    // Create an invalid temp file
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "this is not valid agm content!!!").unwrap();
    let valid = fixture_path("diff/identical/base.agm");

    agm_cmd()
        .args([
            "diff",
            tmp.path().to_str().unwrap(),
            valid.to_str().unwrap(),
        ])
        .assert()
        .code(1);
}

// ---------------------------------------------------------------------------
// 6.3 Diff CLI e2e on generated files
// ---------------------------------------------------------------------------

/// Writes a minimal valid .agm file to `path`.
/// `include_ids` are the node indices to include.
/// `modified_ids` are the 0-based indices whose summary differs from "original summary N".
fn write_agm_file(
    path: &std::path::Path,
    include_ids: std::ops::Range<usize>,
    modified_ids: &[usize],
) {
    let mut content = "agm: 1.0\npackage: gen.pkg\nversion: 0.1.0\n\n".to_owned();
    for i in include_ids {
        let summary = if modified_ids.contains(&i) {
            format!("modified summary {i}")
        } else {
            format!("original summary {i}")
        };
        content.push_str(&format!(
            "node gen.n{i:02}\ntype: facts\nsummary: {summary}\n\n"
        ));
    }
    std::fs::write(path, content).expect("should write agm file");
}

#[test]
fn test_diff_cli_large_generated_files_text_format() {
    let dir = tempfile::tempdir().unwrap();
    let left_path = dir.path().join("left.agm");
    let right_path = dir.path().join("right.agm");

    // Left: nodes 0..20, Right: nodes 3..23 (removes 0-2, adds 20-22), 5 modified (3-7).
    // Removed nodes are breaking -> exit code 2.
    write_agm_file(&left_path, 0..20, &[]);
    write_agm_file(&right_path, 3..23, &[3, 4, 5, 6, 7]);

    let output = agm_cmd()
        .args([
            "diff",
            left_path.to_str().unwrap(),
            right_path.to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(
        !stdout.is_empty(),
        "diff output must not be empty for differing files"
    );
    assert!(
        stdout.contains("=== AGM Semantic Diff ==="),
        "text output must contain the diff header marker"
    );
}

#[test]
fn test_diff_cli_large_generated_files_json_format() {
    let dir = tempfile::tempdir().unwrap();
    let left_path = dir.path().join("left.agm");
    let right_path = dir.path().join("right.agm");

    // Removed nodes are breaking -> exit code 2.
    write_agm_file(&left_path, 0..20, &[]);
    write_agm_file(&right_path, 3..23, &[3, 4, 5, 6, 7]);

    let output = agm_cmd()
        .args([
            "diff",
            left_path.to_str().unwrap(),
            right_path.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let _: agm_core::diff::DiffReport =
        serde_json::from_str(&stdout).expect("JSON output must be a valid DiffReport");
}

#[test]
fn test_diff_cli_large_generated_files_markdown_format() {
    let dir = tempfile::tempdir().unwrap();
    let left_path = dir.path().join("left.agm");
    let right_path = dir.path().join("right.agm");

    // Removed nodes are breaking -> exit code 2.
    write_agm_file(&left_path, 0..20, &[]);
    write_agm_file(&right_path, 3..23, &[3, 4, 5, 6, 7]);

    let output = agm_cmd()
        .args([
            "diff",
            left_path.to_str().unwrap(),
            right_path.to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(
        stdout.contains("# AGM Semantic Diff"),
        "markdown output must contain top-level header"
    );
    assert!(
        stdout.contains("## Summary"),
        "markdown output must contain Summary section"
    );
}
