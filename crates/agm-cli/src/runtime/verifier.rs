//! Verifier: executes verify checks declared on AGM nodes.
#![allow(dead_code)]

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use regex::Regex;

use agm_core::model::node::Node;
use agm_core::model::verify::VerifyCheck;

use super::state::ExecutionTracker;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default timeout for command checks (60 seconds).
const DEFAULT_TIMEOUT_SECS: u64 = 60;

/// Maximum bytes captured per output stream (stdout/stderr).
const MAX_OUTPUT_BYTES: usize = 4096;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Strategy for handling multiple verify checks.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub enum FailStrategy {
    /// Stop on first failure (do not run remaining checks).
    FailFast,
    /// Run all checks regardless of failures (default).
    #[default]
    RunAll,
}

/// Result of running all verify checks for a single node.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifyResult {
    /// The node ID that was verified.
    pub node_id: String,
    /// Individual results for each check, in execution order.
    pub checks: Vec<CheckResult>,
    /// `true` if every check passed.
    pub all_passed: bool,
}

/// Result of a single verification check.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    /// The check definition that was executed.
    pub check: VerifyCheck,
    /// Whether the check passed.
    pub passed: bool,
    /// Human-readable message describing the outcome.
    /// On success: brief confirmation.
    /// On failure: what was expected vs. what happened.
    /// On command checks: includes captured stdout/stderr.
    pub message: String,
    /// Wall-clock time spent executing this check.
    pub duration: Duration,
}

// ---------------------------------------------------------------------------
// Pattern matching helper (shared by file_contains / file_not_contains)
// ---------------------------------------------------------------------------

/// Checks whether `content` matches `pattern`.
///
/// Returns `Ok(true)` if found, `Ok(false)` if not found,
/// `Err(message)` if the regex is invalid.
fn match_pattern(content: &str, pattern: &str) -> Result<bool, String> {
    let is_regex = pattern.len() >= 2 && pattern.starts_with('/') && pattern.ends_with('/');
    if is_regex {
        let regex_str = &pattern[1..pattern.len() - 1];
        match Regex::new(regex_str) {
            Ok(re) => Ok(re.is_match(content)),
            Err(e) => Err(format!("Invalid regex '{}': {}", regex_str, e)),
        }
    } else {
        Ok(content.contains(pattern))
    }
}

// ---------------------------------------------------------------------------
// Individual check runners (private)
// ---------------------------------------------------------------------------

/// Executes a `file_exists` verification check.
///
/// Resolves `file` relative to `working_dir` and checks existence.
fn run_file_exists_check(file: &str, working_dir: &Path) -> CheckResult {
    let start = Instant::now();
    let path = working_dir.join(file);
    let exists = path.exists();
    CheckResult {
        check: VerifyCheck::FileExists {
            file: file.to_owned(),
        },
        passed: exists,
        message: if exists {
            format!("File exists: {}", path.display())
        } else {
            format!("File not found: {}", path.display())
        },
        duration: start.elapsed(),
    }
}

/// Executes a `file_contains` verification check.
///
/// Reads the file at `working_dir/file` and checks whether any line
/// matches `pattern`.
///
/// Pattern interpretation:
/// - If `pattern` starts with `/` and ends with `/`: treat as regex
///   (strip the delimiters, compile with `Regex::new`)
/// - Otherwise: treat as literal substring match
///
/// Returns a failed check if the file cannot be read or if a regex
/// pattern is invalid.
fn run_file_contains_check(file: &str, pattern: &str, working_dir: &Path) -> CheckResult {
    let start = Instant::now();
    let path = working_dir.join(file);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return CheckResult {
                check: VerifyCheck::FileContains {
                    file: file.to_owned(),
                    pattern: pattern.to_owned(),
                },
                passed: false,
                message: format!("Cannot read file: {}: {}", path.display(), e),
                duration: start.elapsed(),
            };
        }
    };

    match match_pattern(&content, pattern) {
        Ok(found) => CheckResult {
            check: VerifyCheck::FileContains {
                file: file.to_owned(),
                pattern: pattern.to_owned(),
            },
            passed: found,
            message: if found {
                format!("File '{}' contains pattern: {}", file, pattern)
            } else {
                format!("File '{}' does not contain pattern: {}", file, pattern)
            },
            duration: start.elapsed(),
        },
        Err(msg) => CheckResult {
            check: VerifyCheck::FileContains {
                file: file.to_owned(),
                pattern: pattern.to_owned(),
            },
            passed: false,
            message: msg,
            duration: start.elapsed(),
        },
    }
}

/// Executes a `file_not_contains` verification check.
///
/// Inverse of `file_contains`: passes when the pattern is NOT found.
/// Same pattern interpretation rules (literal vs. regex with `/` delimiters).
///
/// If the file does not exist, the check **passes** (the file trivially
/// does not contain the pattern).
fn run_file_not_contains_check(file: &str, pattern: &str, working_dir: &Path) -> CheckResult {
    let start = Instant::now();
    let path = working_dir.join(file);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            // File missing or unreadable -> passes (pattern is trivially absent)
            return CheckResult {
                check: VerifyCheck::FileNotContains {
                    file: file.to_owned(),
                    pattern: pattern.to_owned(),
                },
                passed: true,
                message: format!("File '{}' not found (check passes trivially)", file),
                duration: start.elapsed(),
            };
        }
    };

    match match_pattern(&content, pattern) {
        Ok(found) => CheckResult {
            check: VerifyCheck::FileNotContains {
                file: file.to_owned(),
                pattern: pattern.to_owned(),
            },
            passed: !found,
            message: if !found {
                format!("File '{}' does not contain pattern: {}", file, pattern)
            } else {
                format!("File '{}' unexpectedly contains pattern: {}", file, pattern)
            },
            duration: start.elapsed(),
        },
        Err(msg) => CheckResult {
            check: VerifyCheck::FileNotContains {
                file: file.to_owned(),
                pattern: pattern.to_owned(),
            },
            passed: false,
            message: msg,
            duration: start.elapsed(),
        },
    }
}

/// Executes a `node_status` verification check.
///
/// Looks up `node` in the `ExecutionTracker` and compares its
/// `execution_status` (via `Display` / `to_string()`) to `expected_status`.
///
/// Comparison is case-insensitive and uses the snake_case string
/// representation (e.g., `"completed"`, `"in_progress"`).
fn run_node_status_check(
    node: &str,
    expected_status: &str,
    tracker: &ExecutionTracker,
) -> CheckResult {
    let start = Instant::now();
    match tracker.node_state(node) {
        None => CheckResult {
            check: VerifyCheck::NodeStatus {
                node: node.to_owned(),
                status: expected_status.to_owned(),
            },
            passed: false,
            message: format!("Node '{}' not found in execution state", node),
            duration: start.elapsed(),
        },
        Some(state) => {
            let actual = state.execution_status.to_string();
            let passed = actual.eq_ignore_ascii_case(expected_status.trim());
            CheckResult {
                check: VerifyCheck::NodeStatus {
                    node: node.to_owned(),
                    status: expected_status.to_owned(),
                },
                passed,
                message: if passed {
                    format!(
                        "Node '{}' has status '{}' (expected '{}')",
                        node, actual, expected_status
                    )
                } else {
                    format!(
                        "Node '{}' has status '{}', expected '{}'",
                        node, actual, expected_status
                    )
                },
                duration: start.elapsed(),
            }
        }
    }
}

/// Executes a `command` verification check.
///
/// Spawns a shell process with the given `run` string:
/// - Unix: `sh -c "<run>"`
/// - Windows: `cmd /C "<run>"`
///
/// The `expect` parameter determines the pass condition:
/// - `None` or `Some("exit_code_0")`: pass if exit code == 0
/// - `Some("exit_code_nonzero")`: pass if exit code != 0
/// - `Some("output_contains: <text>")`: pass if stdout or stderr contains `<text>`
/// - `Some("output_matches: <regex>")`: pass if stdout or stderr matches the regex
///
/// The command runs in `working_dir` with the given `timeout`.
/// If the process exceeds the timeout, it is killed and the check fails.
fn run_command_check(
    run: &str,
    expect: Option<&str>,
    working_dir: &Path,
    timeout: Duration,
) -> CheckResult {
    let start = Instant::now();

    #[cfg(unix)]
    let mut cmd = std::process::Command::new("sh");
    #[cfg(unix)]
    cmd.args(["-c", run]);

    #[cfg(windows)]
    let mut cmd = std::process::Command::new("cmd");
    #[cfg(windows)]
    cmd.args(["/C", run]);

    cmd.current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CheckResult {
                check: VerifyCheck::Command {
                    run: run.to_owned(),
                    expect: expect.map(str::to_owned),
                },
                passed: false,
                message: format!("Failed to spawn command: {}", e),
                duration: start.elapsed(),
            };
        }
    };

    // Poll loop with timeout
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return CheckResult {
                        check: VerifyCheck::Command {
                            run: run.to_owned(),
                            expect: expect.map(str::to_owned),
                        },
                        passed: false,
                        message: format!("Command timed out after {:?}", timeout),
                        duration: start.elapsed(),
                    };
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                return CheckResult {
                    check: VerifyCheck::Command {
                        run: run.to_owned(),
                        expect: expect.map(str::to_owned),
                    },
                    passed: false,
                    message: format!("Command error while waiting: {}", e),
                    duration: start.elapsed(),
                };
            }
        }
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            return CheckResult {
                check: VerifyCheck::Command {
                    run: run.to_owned(),
                    expect: expect.map(str::to_owned),
                },
                passed: false,
                message: format!("Command error: {}", e),
                duration: start.elapsed(),
            };
        }
    };

    let expect_str = expect.unwrap_or("exit_code_0");
    let exit_code = output.status.code().unwrap_or(-1);

    let stdout_raw = &output.stdout[..output.stdout.len().min(MAX_OUTPUT_BYTES)];
    let stderr_raw = &output.stderr[..output.stderr.len().min(MAX_OUTPUT_BYTES)];

    let stdout_truncated = output.stdout.len() > MAX_OUTPUT_BYTES;
    let stderr_truncated = output.stderr.len() > MAX_OUTPUT_BYTES;

    let stdout = {
        let s = String::from_utf8_lossy(stdout_raw);
        if stdout_truncated {
            format!("{}... (truncated)", s)
        } else if s.is_empty() {
            "(empty)".to_owned()
        } else {
            s.into_owned()
        }
    };
    let stderr = {
        let s = String::from_utf8_lossy(stderr_raw);
        if stderr_truncated {
            format!("{}... (truncated)", s)
        } else if s.is_empty() {
            "(empty)".to_owned()
        } else {
            s.into_owned()
        }
    };

    let combined_output = format!(
        "{}{}",
        String::from_utf8_lossy(stdout_raw),
        String::from_utf8_lossy(stderr_raw)
    );

    let passed = match expect_str {
        "exit_code_0" => exit_code == 0,
        "exit_code_nonzero" => exit_code != 0,
        s if s.starts_with("output_contains: ") => {
            let text = s["output_contains: ".len()..].trim();
            combined_output.contains(text)
        }
        s if s.starts_with("output_matches: ") => {
            let regex_str = s["output_matches: ".len()..].trim();
            match Regex::new(regex_str) {
                Ok(re) => re.is_match(&combined_output),
                Err(e) => {
                    return CheckResult {
                        check: VerifyCheck::Command {
                            run: run.to_owned(),
                            expect: expect.map(str::to_owned),
                        },
                        passed: false,
                        message: format!("Invalid regex '{}': {}", regex_str, e),
                        duration: start.elapsed(),
                    };
                }
            }
        }
        _ => {
            return CheckResult {
                check: VerifyCheck::Command {
                    run: run.to_owned(),
                    expect: expect.map(str::to_owned),
                },
                passed: false,
                message: format!("Unknown expect value: {}", expect_str),
                duration: start.elapsed(),
            };
        }
    };

    let message = if passed {
        format!(
            "Command succeeded: exit_code={}\nstdout: {}\nstderr: {}",
            exit_code, stdout, stderr
        )
    } else {
        format!(
            "Command failed: expected {}, got exit_code={}\nstdout: {}\nstderr: {}",
            expect_str, exit_code, stdout, stderr
        )
    };

    CheckResult {
        check: VerifyCheck::Command {
            run: run.to_owned(),
            expect: expect.map(str::to_owned),
        },
        passed,
        message,
        duration: start.elapsed(),
    }
}

// ---------------------------------------------------------------------------
// Batch runner (public)
// ---------------------------------------------------------------------------

/// Runs all `verify:` checks for a node.
///
/// If the node has no `verify` field (or it is empty), returns a
/// `VerifyResult` with `all_passed: true` and an empty `checks` vec.
///
/// The `strategy` parameter controls behavior on failure:
/// - `FailFast`: stop after the first failing check
/// - `RunAll`: execute all checks regardless of failures (default)
///
/// The `timeout` applies to each individual command check, not to the
/// total verification time.
pub fn verify_node(
    node: &Node,
    tracker: &ExecutionTracker,
    working_dir: &Path,
    timeout: Duration,
    strategy: FailStrategy,
) -> VerifyResult {
    let checks_def = match &node.verify {
        Some(v) if !v.is_empty() => v,
        _ => {
            return VerifyResult {
                node_id: node.id.clone(),
                checks: vec![],
                all_passed: true,
            };
        }
    };

    let mut results = Vec::with_capacity(checks_def.len());

    for check in checks_def {
        let result = match check {
            VerifyCheck::Command { run, expect } => {
                run_command_check(run, expect.as_deref(), working_dir, timeout)
            }
            VerifyCheck::FileExists { file } => run_file_exists_check(file, working_dir),
            VerifyCheck::FileContains { file, pattern } => {
                run_file_contains_check(file, pattern, working_dir)
            }
            VerifyCheck::FileNotContains { file, pattern } => {
                run_file_not_contains_check(file, pattern, working_dir)
            }
            VerifyCheck::NodeStatus {
                node: target_node,
                status,
            } => run_node_status_check(target_node, status, tracker),
        };

        let failed = !result.passed;
        results.push(result);

        if failed && strategy == FailStrategy::FailFast {
            break;
        }
    }

    let all_passed = results.iter().all(|r| r.passed);

    VerifyResult {
        node_id: node.id.clone(),
        checks: results,
        all_passed,
    }
}

// ---------------------------------------------------------------------------
// Log formatting (public)
// ---------------------------------------------------------------------------

/// Returns a short label for a `VerifyCheck` variant, suitable for log lines.
fn check_label(check: &VerifyCheck) -> String {
    match check {
        VerifyCheck::Command { run, .. } => format!("command: {}", run),
        VerifyCheck::FileExists { file } => format!("file_exists: {}", file),
        VerifyCheck::FileContains { file, .. } => format!("file_contains: {}", file),
        VerifyCheck::FileNotContains { file, .. } => format!("file_not_contains: {}", file),
        VerifyCheck::NodeStatus { node, status } => {
            format!("node_status: {} == {}", node, status)
        }
    }
}

/// Formats a `VerifyResult` as a human-readable log string.
///
/// Example output:
/// ```text
/// Verification: 3/3 checks passed
///   [PASS] command: echo hello (2ms)
///   [PASS] file_exists: src/main.rs (0ms)
///   [FAIL] file_contains: src/lib.rs -- File 'src/lib.rs' does not contain pattern: fn main (1ms)
/// ```
#[must_use]
pub fn format_verify_log(result: &VerifyResult) -> String {
    let total = result.checks.len();
    let passed_count = result.checks.iter().filter(|c| c.passed).count();

    let mut lines = Vec::with_capacity(total + 1);
    lines.push(format!(
        "Verification: {}/{} checks passed",
        passed_count, total
    ));

    for check in &result.checks {
        let tag = if check.passed { "[PASS]" } else { "[FAIL]" };
        let label = check_label(&check.check);
        let ms = check.duration.as_millis();

        if check.passed {
            lines.push(format!("  {} {} ({}ms)", tag, label, ms));
        } else {
            // Include only the first line of the message for compactness
            let first_line = check.message.lines().next().unwrap_or(&check.message);
            lines.push(format!("  {} {} -- {} ({}ms)", tag, label, first_line, ms));
        }
    }

    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agm_core::graph::build_graph;
    use agm_core::model::fields::{NodeType, Span};
    use agm_core::model::file::{AgmFile, Header};
    use agm_core::model::node::Node as AgmNode;
    use agm_core::model::verify::VerifyCheck;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// Creates a minimal Node with the given verify checks.
    fn node_with_checks(id: &str, checks: Vec<VerifyCheck>) -> AgmNode {
        AgmNode {
            id: id.to_owned(),
            node_type: NodeType::Workflow,
            summary: format!("Test node {}", id),
            verify: if checks.is_empty() {
                None
            } else {
                Some(checks)
            },
            priority: None,
            stability: None,
            confidence: None,
            status: None,
            depends: None,
            related_to: None,
            replaces: None,
            conflicts: None,
            see_also: None,
            items: None,
            steps: None,
            fields: None,
            input: None,
            output: None,
            detail: None,
            rationale: None,
            tradeoffs: None,
            resolution: None,
            examples: None,
            notes: None,
            code: None,
            code_blocks: None,
            agent_context: None,
            target: None,
            execution_status: None,
            executed_by: None,
            executed_at: None,
            execution_log: None,
            retry_count: None,
            parallel_groups: None,
            memory: None,
            scope: None,
            applies_when: None,
            valid_from: None,
            valid_until: None,
            tags: None,
            aliases: None,
            keywords: None,
            extra_fields: BTreeMap::new(),
            span: Span::new(0, 0),
        }
    }

    fn minimal_header() -> Header {
        Header {
            agm: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            title: None,
            owner: None,
            imports: None,
            default_load: None,
            description: None,
            tags: None,
            status: None,
            load_profiles: None,
            target_runtime: None,
        }
    }

    /// Creates a minimal ExecutionTracker for testing.
    /// All nodes start as Ready (no dependencies).
    fn test_tracker(node_ids: &[&str]) -> ExecutionTracker {
        let nodes: Vec<AgmNode> = node_ids
            .iter()
            .map(|id| node_with_checks(id, vec![]))
            .collect();
        let file = AgmFile {
            header: minimal_header(),
            nodes,
        };
        let graph = build_graph(&file);
        let dir = tempdir().unwrap();
        let agm_path = dir.path().join("test.agm");
        // We need the dir to stay alive; leak it for the duration of the test.
        std::mem::forget(dir);
        ExecutionTracker::new(&agm_path, &file, &graph).unwrap()
    }

    // -----------------------------------------------------------------------
    // Platform-specific test constants
    // -----------------------------------------------------------------------

    #[cfg(unix)]
    const FAIL_CMD: &str = "false";
    #[cfg(windows)]
    const FAIL_CMD: &str = "exit /b 1";

    #[cfg(unix)]
    const SLEEP_CMD: &str = "sleep 5";
    #[cfg(windows)]
    const SLEEP_CMD: &str = "ping -n 6 127.0.0.1 >nul";

    #[cfg(unix)]
    const BOTH_CMD: &str = "echo out && echo err >&2";
    #[cfg(windows)]
    const BOTH_CMD: &str = "echo out & echo err 1>&2";

    // -----------------------------------------------------------------------
    // Group A: Command check -- exit code (tests #1-4)
    // -----------------------------------------------------------------------

    #[test]
    fn test_command_check_exit_code_0_passes() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            "echo hello",
            Some("exit_code_0"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_command_check_exit_code_0_default_expect() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            "echo hello",
            None,
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_command_check_exit_code_nonzero_passes() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            FAIL_CMD,
            Some("exit_code_nonzero"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_command_check_exit_code_nonzero_fails_on_success() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            "echo hello",
            Some("exit_code_nonzero"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(!result.passed, "expected fail, got: {}", result.message);
    }

    // -----------------------------------------------------------------------
    // Group B: Command check -- output matching (tests #5-7)
    // -----------------------------------------------------------------------

    #[test]
    fn test_command_check_output_contains_passes() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            "echo hello world",
            Some("output_contains: hello"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_command_check_output_contains_fails() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            "echo hello",
            Some("output_contains: goodbye"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(!result.passed, "expected fail, got: {}", result.message);
    }

    #[test]
    fn test_command_check_output_matches_regex_passes() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            "echo hello123",
            Some(r"output_matches: hello\d+"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    // -----------------------------------------------------------------------
    // Group C: Command check -- edge cases (tests #8-10)
    // -----------------------------------------------------------------------

    #[test]
    fn test_command_check_output_matches_invalid_regex_fails() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            "echo hello",
            Some("output_matches: [invalid"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        assert!(!result.passed);
        assert!(
            result.message.contains("Invalid regex") || result.message.contains("invalid"),
            "expected descriptive error, got: {}",
            result.message
        );
    }

    #[test]
    fn test_command_check_timeout_fails() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            SLEEP_CMD,
            Some("exit_code_0"),
            dir.path(),
            Duration::from_secs(1),
        );
        assert!(!result.passed);
        assert!(
            result.message.to_lowercase().contains("timeout")
                || result.message.to_lowercase().contains("timed out"),
            "expected timeout message, got: {}",
            result.message
        );
    }

    #[test]
    fn test_command_check_captures_stdout_and_stderr() {
        let dir = tempdir().unwrap();
        let result = run_command_check(
            BOTH_CMD,
            Some("exit_code_0"),
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        );
        // The message should contain both stream content
        assert!(
            result.message.contains("out") || result.message.contains("stdout"),
            "expected stdout in message, got: {}",
            result.message
        );
    }

    // -----------------------------------------------------------------------
    // Group D: File exists check (tests #11-12)
    // -----------------------------------------------------------------------

    #[test]
    fn test_file_exists_check_passes_for_existing_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("hello.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let result = run_file_exists_check("hello.txt", dir.path());
        assert!(result.passed, "expected pass, got: {}", result.message);
        assert!(result.message.contains("exists"));
    }

    #[test]
    fn test_file_exists_check_fails_for_missing_file() {
        let dir = tempdir().unwrap();
        let result = run_file_exists_check("missing.txt", dir.path());
        assert!(!result.passed);
        assert!(result.message.contains("not found") || result.message.contains("missing"));
    }

    // -----------------------------------------------------------------------
    // Group E: File contains check (tests #13-16)
    // -----------------------------------------------------------------------

    #[test]
    fn test_file_contains_literal_passes() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("lib.rs"), "fn main() {}").unwrap();

        let result = run_file_contains_check("lib.rs", "fn main", dir.path());
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_file_contains_literal_fails() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("lib.rs"), "fn helper() {}").unwrap();

        let result = run_file_contains_check("lib.rs", "fn main", dir.path());
        assert!(!result.passed);
    }

    #[test]
    fn test_file_contains_regex_passes() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "version = 1.2.3").unwrap();

        let result =
            run_file_contains_check("Cargo.toml", r"/version = \d+\.\d+\.\d+/", dir.path());
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_file_contains_invalid_regex_fails() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("lib.rs"), "some content").unwrap();

        let result = run_file_contains_check("lib.rs", "/[unclosed/", dir.path());
        assert!(!result.passed);
        assert!(
            result.message.contains("Invalid regex"),
            "expected regex error, got: {}",
            result.message
        );
    }

    // -----------------------------------------------------------------------
    // Group F: File not-contains check (tests #17-19)
    // -----------------------------------------------------------------------

    #[test]
    fn test_file_not_contains_passes_when_absent() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("lib.rs"), "fn helper() {}").unwrap();

        let result = run_file_not_contains_check("lib.rs", "unsafe", dir.path());
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_file_not_contains_fails_when_present() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("lib.rs"), "unsafe { }").unwrap();

        let result = run_file_not_contains_check("lib.rs", "unsafe", dir.path());
        assert!(!result.passed);
        assert!(result.message.contains("unexpectedly"));
    }

    #[test]
    fn test_file_not_contains_passes_when_file_missing() {
        let dir = tempdir().unwrap();
        let result = run_file_not_contains_check("nonexistent.txt", "pattern", dir.path());
        assert!(result.passed, "expected pass, got: {}", result.message);
        assert!(result.message.contains("trivially") || result.message.contains("not found"));
    }

    // -----------------------------------------------------------------------
    // Group G: Node status check (tests #20-22)
    // -----------------------------------------------------------------------

    #[test]
    fn test_node_status_check_passes_matching_status() {
        let tracker = test_tracker(&["node.a"]);
        // Nodes with no deps start as Ready
        let result = run_node_status_check("node.a", "ready", &tracker);
        assert!(result.passed, "expected pass, got: {}", result.message);
    }

    #[test]
    fn test_node_status_check_fails_mismatched_status() {
        let tracker = test_tracker(&["node.a"]);
        let result = run_node_status_check("node.a", "completed", &tracker);
        assert!(!result.passed);
        assert!(result.message.contains("ready"));
        assert!(result.message.contains("completed"));
    }

    #[test]
    fn test_node_status_check_fails_unknown_node() {
        let tracker = test_tracker(&["node.a"]);
        let result = run_node_status_check("node.unknown", "ready", &tracker);
        assert!(!result.passed);
        assert!(
            result.message.contains("not found"),
            "expected 'not found', got: {}",
            result.message
        );
    }

    // -----------------------------------------------------------------------
    // Group H: verify_node batch execution (tests #23-26)
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_node_all_pass_returns_true() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "content").unwrap();
        std::fs::write(dir.path().join("b.txt"), "content").unwrap();

        let tracker = test_tracker(&["node.x"]);
        let node = node_with_checks(
            "node.x",
            vec![
                VerifyCheck::FileExists {
                    file: "a.txt".to_owned(),
                },
                VerifyCheck::FileExists {
                    file: "b.txt".to_owned(),
                },
            ],
        );

        let result = verify_node(
            &node,
            &tracker,
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            FailStrategy::RunAll,
        );
        assert!(result.all_passed);
        assert_eq!(result.checks.len(), 2);
    }

    #[test]
    fn test_verify_node_any_fail_returns_false() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("exists.txt"), "content").unwrap();

        let tracker = test_tracker(&["node.x"]);
        let node = node_with_checks(
            "node.x",
            vec![
                VerifyCheck::FileExists {
                    file: "exists.txt".to_owned(),
                },
                VerifyCheck::FileExists {
                    file: "missing.txt".to_owned(),
                },
            ],
        );

        let result = verify_node(
            &node,
            &tracker,
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            FailStrategy::RunAll,
        );
        assert!(!result.all_passed);
        assert_eq!(result.checks.len(), 2);
        assert!(result.checks[0].passed);
        assert!(!result.checks[1].passed);
    }

    #[test]
    fn test_verify_node_no_checks_returns_true() {
        let dir = tempdir().unwrap();
        let tracker = test_tracker(&["node.x"]);
        let node = node_with_checks("node.x", vec![]);

        let result = verify_node(
            &node,
            &tracker,
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            FailStrategy::RunAll,
        );
        assert!(result.all_passed);
        assert!(result.checks.is_empty());
    }

    #[test]
    fn test_verify_node_fail_fast_stops_early() {
        let dir = tempdir().unwrap();
        // All three files are missing -> all would fail, but FailFast stops after first
        let tracker = test_tracker(&["node.x"]);
        let node = node_with_checks(
            "node.x",
            vec![
                VerifyCheck::FileExists {
                    file: "missing1.txt".to_owned(),
                },
                VerifyCheck::FileExists {
                    file: "missing2.txt".to_owned(),
                },
                VerifyCheck::FileExists {
                    file: "missing3.txt".to_owned(),
                },
            ],
        );

        let result = verify_node(
            &node,
            &tracker,
            dir.path(),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            FailStrategy::FailFast,
        );
        assert!(!result.all_passed);
        assert_eq!(
            result.checks.len(),
            1,
            "FailFast should stop after first failure"
        );
    }

    // -----------------------------------------------------------------------
    // Group I: Pattern matching helper (tests #27-28)
    // -----------------------------------------------------------------------

    #[test]
    fn test_match_pattern_literal_found() {
        let result = match_pattern("hello world", "world");
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn test_match_pattern_regex_found() {
        let result = match_pattern("v1.2.3", r"/v\d+/");
        assert_eq!(result, Ok(true));
    }

    // -----------------------------------------------------------------------
    // Group J: Log formatting (tests #29-30)
    // -----------------------------------------------------------------------

    fn make_check_result(check: VerifyCheck, passed: bool, message: &str) -> CheckResult {
        CheckResult {
            check,
            passed,
            message: message.to_owned(),
            duration: Duration::from_millis(2),
        }
    }

    #[test]
    fn test_format_verify_log_all_passed() {
        let result = VerifyResult {
            node_id: "node.x".to_owned(),
            checks: vec![
                make_check_result(
                    VerifyCheck::Command {
                        run: "echo hello".to_owned(),
                        expect: None,
                    },
                    true,
                    "Command succeeded: exit_code=0",
                ),
                make_check_result(
                    VerifyCheck::FileExists {
                        file: "src/main.rs".to_owned(),
                    },
                    true,
                    "File exists: /path/src/main.rs",
                ),
            ],
            all_passed: true,
        };

        let log = format_verify_log(&result);
        assert!(log.contains("2/2 checks passed"), "got: {}", log);
        assert!(log.contains("[PASS]"));
        assert!(!log.contains("[FAIL]"));
    }

    #[test]
    fn test_format_verify_log_with_failure() {
        let result = VerifyResult {
            node_id: "node.x".to_owned(),
            checks: vec![
                make_check_result(
                    VerifyCheck::Command {
                        run: "echo hello".to_owned(),
                        expect: None,
                    },
                    true,
                    "Command succeeded: exit_code=0",
                ),
                make_check_result(
                    VerifyCheck::FileExists {
                        file: "src/missing.rs".to_owned(),
                    },
                    false,
                    "File not found: /path/src/missing.rs",
                ),
            ],
            all_passed: false,
        };

        let log = format_verify_log(&result);
        assert!(log.contains("1/2 checks passed"), "got: {}", log);
        assert!(log.contains("[PASS]"));
        assert!(log.contains("[FAIL]"));
        assert!(log.contains("File not found"));
    }

    // -----------------------------------------------------------------------
    // Group K: File path resolution (test #31)
    // -----------------------------------------------------------------------

    #[test]
    fn test_file_checks_resolve_relative_to_working_dir() {
        let dir = tempdir().unwrap();
        // Create a subdirectory with a file
        let subdir = dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("data.txt"), "hello content").unwrap();

        // Use working_dir = dir.path(), file = "subdir/data.txt"
        let exists_result = run_file_exists_check("subdir/data.txt", dir.path());
        assert!(
            exists_result.passed,
            "file_exists should resolve relative to working_dir: {}",
            exists_result.message
        );

        let contains_result =
            run_file_contains_check("subdir/data.txt", "hello content", dir.path());
        assert!(
            contains_result.passed,
            "file_contains should resolve relative to working_dir: {}",
            contains_result.message
        );
    }
}
