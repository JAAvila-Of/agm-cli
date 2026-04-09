//! Agent backend trait and request/response types.
//!
//! This module defines the interface that the scheduler uses to dispatch
//! node execution to an agent. `ShellAgent` executes code blocks as file
//! operations or shell commands. `MockAgent` is a configurable test double.
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use agm_core::model::code::{CodeAction, CodeBlock};

// ---------------------------------------------------------------------------
// Public types (existing — unchanged)
// ---------------------------------------------------------------------------

/// Request sent to an agent for node execution.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// The ID of the node being executed.
    pub node_id: String,
    /// The built context/prompt string for the agent.
    pub context: String,
    /// Code blocks to execute or apply.
    pub code_blocks: Vec<CodeBlock>,
    /// Working directory for file operations.
    pub working_dir: PathBuf,
    /// Maximum time the agent may spend on this node.
    pub timeout: Duration,
}

/// Response from an agent after executing a node.
#[derive(Debug, Clone)]
pub struct AgentResponse {
    /// Whether the agent considers execution successful.
    pub success: bool,
    /// Output text (stdout, logs, or agent response).
    pub output: String,
    /// Wall-clock time the agent spent.
    pub duration: Duration,
}

/// Trait for agent backends that execute AGM nodes.
///
/// Implementations must be `Send + Sync` to support parallel dispatch
/// via `Arc<dyn AgentBackend>` across `std::thread` boundaries.
pub trait AgentBackend: Send + Sync {
    /// Execute a node given the request. Returns the agent's response.
    ///
    /// Implementations should:
    /// 1. Process the context string (e.g., send to LLM or interpret as shell commands)
    /// 2. Apply/execute code blocks as appropriate
    /// 3. Return success/failure with output
    ///
    /// The timeout in `AgentRequest` is advisory -- the agent should attempt to
    /// respect it, but the scheduler also enforces it at the thread level.
    fn execute(&self, request: AgentRequest) -> anyhow::Result<AgentResponse>;

    /// Returns the agent's name (e.g., "claude-sonnet", "shell", "mock").
    ///
    /// Used for the `executed_by` field in execution state.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum bytes captured per output stream (stdout/stderr).
const MAX_OUTPUT_BYTES: usize = 8192;

// ---------------------------------------------------------------------------
// ShellAgent
// ---------------------------------------------------------------------------

/// An agent that executes code blocks as file operations or shell commands.
///
/// File operations (`create`, `append`, `prepend`, `replace`, `insert_before`,
/// `insert_after`, `full`) write to `target` paths relative to
/// `request.working_dir`.
///
/// Shell execution occurs when a code block has `lang` set to a shell language
/// (`sh`, `bash`, `shell`, `cmd`) and no `target` path.
pub struct ShellAgent;

impl AgentBackend for ShellAgent {
    fn execute(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
        let start = Instant::now();
        let mut outputs: Vec<String> = Vec::new();

        for (i, block) in request.code_blocks.iter().enumerate() {
            log::debug!(
                "[shell-agent] node={} block={}/{} action={} target={:?} lang={:?}",
                request.node_id,
                i + 1,
                request.code_blocks.len(),
                block.action,
                block.target,
                block.lang,
            );

            let result = if is_shell_command(block) {
                execute_shell_command(
                    &block.body,
                    &request.working_dir,
                    request.timeout.saturating_sub(start.elapsed()),
                )
            } else {
                execute_file_action(block, &request.working_dir)
            };

            match result {
                Ok(output) => outputs.push(output),
                Err(e) => {
                    outputs.push(format!("Error in block {}: {}", i + 1, e));
                    return Ok(AgentResponse {
                        success: false,
                        output: outputs.join("\n"),
                        duration: start.elapsed(),
                    });
                }
            }
        }

        Ok(AgentResponse {
            success: true,
            output: outputs.join("\n"),
            duration: start.elapsed(),
        })
    }

    fn name(&self) -> &str {
        "shell-agent"
    }
}

// ---------------------------------------------------------------------------
// Shell command detection helper
// ---------------------------------------------------------------------------

/// A code block is treated as a shell command when:
/// 1. Its `lang` is a shell language (sh, bash, shell, cmd), AND
/// 2. It has no `target` path (nothing to write to)
///
/// This heuristic lets the ShellAgent distinguish between "write this bash
/// script to a file" (has target) and "run this command" (no target).
fn is_shell_command(block: &CodeBlock) -> bool {
    let is_shell_lang = block
        .lang
        .as_deref()
        .is_some_and(|l| matches!(l, "sh" | "bash" | "shell" | "cmd" | "powershell" | "zsh"));
    is_shell_lang && block.target.is_none()
}

// ---------------------------------------------------------------------------
// Path traversal validation helper
// ---------------------------------------------------------------------------

/// Returns `true` if any component of `path` is `..`.
fn has_dotdot(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|c| c == std::path::Component::ParentDir)
}

// ---------------------------------------------------------------------------
// File action executor
// ---------------------------------------------------------------------------

/// Executes a file-based code block action.
///
/// All target paths are resolved relative to `working_dir`. The function
/// validates that:
/// - `target` is present (returns error if missing for file actions)
/// - The resolved path does not escape `working_dir` via `..` traversal
fn execute_file_action(block: &CodeBlock, working_dir: &Path) -> anyhow::Result<String> {
    let target_rel = block.target.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "Code block with action '{}' requires a target path",
            block.action
        )
    })?;

    // Security: reject any path that contains `..` components (spec S34.5)
    if has_dotdot(target_rel) {
        anyhow::bail!(
            "Target path '{}' contains '..' and is rejected for security reasons",
            target_rel
        );
    }

    let target_path = working_dir.join(target_rel);

    // For paths where the parent already exists, also verify via canonicalize
    // that the resolved path stays within working_dir.
    if let Some(parent) = target_path.parent() {
        if parent.exists() {
            let canonical_working = working_dir
                .canonicalize()
                .unwrap_or_else(|_| working_dir.to_path_buf());
            let canonical_target = parent
                .canonicalize()
                .map(|p| p.join(target_path.file_name().unwrap_or_default()))
                .unwrap_or_else(|_| target_path.clone());
            if !canonical_target.starts_with(&canonical_working) {
                anyhow::bail!(
                    "Target path '{}' resolves outside working directory '{}'",
                    target_rel,
                    working_dir.display()
                );
            }
        }
    }

    match block.action {
        CodeAction::Create => {
            if target_path.exists() {
                anyhow::bail!("Target file already exists: {}", target_path.display());
            }
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target_path, &block.body)?;
            Ok(format!("Created {}", target_rel))
        }
        CodeAction::Full => {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target_path, &block.body)?;
            Ok(format!("Wrote (full) {}", target_rel))
        }
        CodeAction::Append => {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&target_path)?;
            file.write_all(block.body.as_bytes())?;
            Ok(format!("Appended to {}", target_rel))
        }
        CodeAction::Prepend => {
            let existing = if target_path.exists() {
                fs::read_to_string(&target_path)?
            } else {
                String::new()
            };
            let content = format!("{}{}", block.body, existing);
            fs::write(&target_path, content)?;
            Ok(format!("Prepended to {}", target_rel))
        }
        CodeAction::Replace => {
            let old = block
                .old
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("Replace action requires 'old' field"))?;
            let content = fs::read_to_string(&target_path)?;
            let count = content.matches(old).count();
            if count == 0 {
                anyhow::bail!(
                    "Replace failed: 'old' text not found in {}",
                    target_path.display()
                );
            }
            if count > 1 {
                anyhow::bail!(
                    "Replace failed: 'old' text found {} times in {} (must be exactly 1)",
                    count,
                    target_path.display()
                );
            }
            let new_content = content.replacen(old, &block.body, 1);
            fs::write(&target_path, new_content)?;
            Ok(format!("Replaced in {}", target_rel))
        }
        CodeAction::InsertBefore => {
            let anchor = block
                .anchor
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("InsertBefore action requires 'anchor' field"))?;
            let content = fs::read_to_string(&target_path)?;
            if !content.contains(anchor) {
                anyhow::bail!(
                    "InsertBefore failed: anchor not found in {}",
                    target_path.display()
                );
            }
            let new_content = content.replacen(anchor, &format!("{}{}", block.body, anchor), 1);
            fs::write(&target_path, new_content)?;
            Ok(format!("Inserted before anchor in {}", target_rel))
        }
        CodeAction::InsertAfter => {
            let anchor = block
                .anchor
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("InsertAfter action requires 'anchor' field"))?;
            let content = fs::read_to_string(&target_path)?;
            if !content.contains(anchor) {
                anyhow::bail!(
                    "InsertAfter failed: anchor not found in {}",
                    target_path.display()
                );
            }
            let new_content = content.replacen(anchor, &format!("{}{}", anchor, block.body), 1);
            fs::write(&target_path, new_content)?;
            Ok(format!("Inserted after anchor in {}", target_rel))
        }
    }
}

// ---------------------------------------------------------------------------
// Shell command executor
// ---------------------------------------------------------------------------

/// Executes a shell command string.
///
/// - Unix: `sh -c "<body>"`
/// - Windows: `cmd /C "<body>"`
///
/// Captures stdout + stderr. Kills the process if `timeout` is exceeded.
/// Returns `Ok(combined_output)` on exit code 0, `Err` on non-zero or timeout.
fn execute_shell_command(
    body: &str,
    working_dir: &Path,
    timeout: Duration,
) -> anyhow::Result<String> {
    log::debug!("[shell-agent] exec: {}", body);

    #[cfg(unix)]
    let mut cmd = Command::new("sh");
    #[cfg(unix)]
    cmd.args(["-c", body]);

    #[cfg(windows)]
    let mut cmd = Command::new("cmd");
    #[cfg(windows)]
    cmd.args(["/C", body]);

    cmd.current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn shell: {}", e))?;

    let start = Instant::now();

    // Poll loop with timeout (same pattern as verifier.rs)
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    anyhow::bail!("Command timed out after {:?}: {}", timeout, body);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                anyhow::bail!("Error waiting for command: {}", e);
            }
        }
    }

    let output = child.wait_with_output()?;
    let exit_code = output.status.code().unwrap_or(-1);

    let stdout_raw = &output.stdout[..output.stdout.len().min(MAX_OUTPUT_BYTES)];
    let stderr_raw = &output.stderr[..output.stderr.len().min(MAX_OUTPUT_BYTES)];
    let stdout = String::from_utf8_lossy(stdout_raw);
    let stderr = String::from_utf8_lossy(stderr_raw);

    let combined = if stderr.is_empty() {
        stdout.into_owned()
    } else {
        format!("{}\n[stderr]\n{}", stdout, stderr)
    };

    if exit_code != 0 {
        anyhow::bail!(
            "Command exited with code {}: {}\n{}",
            exit_code,
            body,
            combined
        );
    }

    Ok(combined)
}

// ---------------------------------------------------------------------------
// MockAgent (for testing)
// ---------------------------------------------------------------------------

/// A configurable mock agent for testing.
///
/// Records all received requests and returns pre-configured responses.
/// Supports per-node responses, a default success/fail mode, and
/// simulated delay for concurrency testing.
pub struct MockAgent {
    /// Pre-configured responses per node_id.
    responses: HashMap<String, AgentResponse>,
    /// Default behavior when no per-node response is configured.
    default_success: bool,
    /// Records all received requests (thread-safe for parallel dispatch).
    requests: Mutex<Vec<AgentRequest>>,
    /// Optional delay to simulate work (for concurrency testing).
    delay: Option<Duration>,
}

impl MockAgent {
    /// Creates a new `MockAgent` with `default_success: true` and no delay.
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            default_success: true,
            requests: Mutex::new(Vec::new()),
            delay: None,
        }
    }

    /// Creates a new `MockAgent` with the given default success behavior.
    pub fn with_default(success: bool) -> Self {
        Self {
            responses: HashMap::new(),
            default_success: success,
            requests: Mutex::new(Vec::new()),
            delay: None,
        }
    }

    /// Adds a pre-configured response for a specific node ID.
    /// Builder pattern -- returns `self`.
    #[must_use]
    pub fn with_response(mut self, node_id: &str, response: AgentResponse) -> Self {
        self.responses.insert(node_id.to_owned(), response);
        self
    }

    /// Sets the simulated delay per request.
    /// Builder pattern -- returns `self`.
    #[must_use]
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Returns a snapshot of all requests received so far.
    pub fn received_requests(&self) -> Vec<AgentRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl Default for MockAgent {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentBackend for MockAgent {
    fn execute(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
        self.requests.lock().unwrap().push(request.clone());

        if let Some(delay) = self.delay {
            std::thread::sleep(delay);
        }

        let node_id = &request.node_id;
        if let Some(response) = self.responses.get(node_id) {
            Ok(response.clone())
        } else {
            Ok(AgentResponse {
                success: self.default_success,
                output: if self.default_success {
                    format!("Mock success for {}", node_id)
                } else {
                    format!("Mock failure for {}", node_id)
                },
                duration: self.delay.unwrap_or(Duration::ZERO),
            })
        }
    }

    fn name(&self) -> &str {
        "mock-agent"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_request(node_id: &str, blocks: Vec<CodeBlock>, dir: &Path) -> AgentRequest {
        AgentRequest {
            node_id: node_id.to_owned(),
            context: String::new(),
            code_blocks: blocks,
            working_dir: dir.to_path_buf(),
            timeout: Duration::from_secs(10),
        }
    }

    fn make_code_block(action: CodeAction, body: &str, target: Option<&str>) -> CodeBlock {
        CodeBlock {
            lang: None,
            target: target.map(str::to_owned),
            action,
            body: body.to_owned(),
            anchor: None,
            old: None,
        }
    }

    fn make_shell_block(body: &str) -> CodeBlock {
        CodeBlock {
            lang: Some("sh".to_owned()),
            target: None,
            action: CodeAction::Full, // action is ignored for shell commands
            body: body.to_owned(),
            anchor: None,
            old: None,
        }
    }

    // -----------------------------------------------------------------------
    // ShellAgent tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_shell_agent_name_returns_shell_agent() {
        assert_eq!(ShellAgent.name(), "shell-agent");
    }

    #[cfg(unix)]
    #[test]
    fn test_shell_agent_execute_echo_captures_output() {
        let dir = tempdir().unwrap();
        let req = make_request("n1", vec![make_shell_block("echo hello")], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "expected success");
        assert!(resp.output.contains("hello"), "output: {}", resp.output);
    }

    #[cfg(unix)]
    #[test]
    fn test_shell_agent_execute_nonzero_exit_returns_failure() {
        let dir = tempdir().unwrap();
        let req = make_request("n1", vec![make_shell_block("exit 1")], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure");
    }

    #[cfg(windows)]
    #[test]
    fn test_shell_agent_execute_echo_captures_output() {
        let dir = tempdir().unwrap();
        let block = CodeBlock {
            lang: Some("cmd".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "echo hello".to_owned(),
            anchor: None,
            old: None,
        };
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "expected success");
        assert!(resp.output.contains("hello"), "output: {}", resp.output);
    }

    #[cfg(windows)]
    #[test]
    fn test_shell_agent_execute_nonzero_exit_returns_failure() {
        let dir = tempdir().unwrap();
        let block = CodeBlock {
            lang: Some("cmd".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "exit 1".to_owned(),
            anchor: None,
            old: None,
        };
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure");
    }

    #[cfg(unix)]
    #[test]
    fn test_shell_agent_execute_timeout_kills_process() {
        let dir = tempdir().unwrap();
        let block = CodeBlock {
            lang: Some("sh".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "sleep 10".to_owned(),
            anchor: None,
            old: None,
        };
        let mut req = make_request("n1", vec![block], dir.path());
        req.timeout = Duration::from_millis(200);
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure due to timeout");
        assert!(
            resp.output.contains("timed out") || resp.output.contains("Error"),
            "output: {}",
            resp.output
        );
    }

    #[test]
    fn test_shell_agent_create_action_writes_file() {
        let dir = tempdir().unwrap();
        let block = make_code_block(CodeAction::Create, "hello world", Some("out.txt"));
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        let content = fs::read_to_string(dir.path().join("out.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_shell_agent_create_action_existing_file_fails() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("exists.txt"), "original").unwrap();
        let block = make_code_block(CodeAction::Create, "new content", Some("exists.txt"));
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure for pre-existing file");
    }

    #[test]
    fn test_shell_agent_full_action_overwrites_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "old content").unwrap();
        let block = make_code_block(CodeAction::Full, "new content", Some("file.txt"));
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        let content = fs::read_to_string(dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_shell_agent_append_action_appends_to_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "line1\n").unwrap();
        let block = make_code_block(CodeAction::Append, "line2\n", Some("file.txt"));
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        let content = fs::read_to_string(dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "line1\nline2\n");
    }

    #[test]
    fn test_shell_agent_prepend_action_prepends_to_file() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "line2\n").unwrap();
        let block = make_code_block(CodeAction::Prepend, "line1\n", Some("file.txt"));
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        let content = fs::read_to_string(dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "line1\nline2\n");
    }

    #[test]
    fn test_shell_agent_replace_action_replaces_text() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "foo bar baz").unwrap();
        let mut block = make_code_block(CodeAction::Replace, "REPLACED", Some("file.txt"));
        block.old = Some("bar".to_owned());
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        let content = fs::read_to_string(dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "foo REPLACED baz");
    }

    #[test]
    fn test_shell_agent_replace_action_old_not_found_fails() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "foo bar baz").unwrap();
        let mut block = make_code_block(CodeAction::Replace, "new", Some("file.txt"));
        block.old = Some("notpresent".to_owned());
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure");
    }

    #[test]
    fn test_shell_agent_replace_action_old_multiple_matches_fails() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "foo foo foo").unwrap();
        let mut block = make_code_block(CodeAction::Replace, "bar", Some("file.txt"));
        block.old = Some("foo".to_owned());
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure for multiple matches");
    }

    #[test]
    fn test_shell_agent_insert_before_action_inserts_correctly() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "ANCHOR rest").unwrap();
        let mut block = make_code_block(CodeAction::InsertBefore, "PREFIX ", Some("file.txt"));
        block.anchor = Some("ANCHOR".to_owned());
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        let content = fs::read_to_string(dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "PREFIX ANCHOR rest");
    }

    #[test]
    fn test_shell_agent_insert_after_action_inserts_correctly() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "ANCHOR rest").unwrap();
        let mut block = make_code_block(CodeAction::InsertAfter, " SUFFIX", Some("file.txt"));
        block.anchor = Some("ANCHOR".to_owned());
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        let content = fs::read_to_string(dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "ANCHOR SUFFIX rest");
    }

    #[test]
    fn test_shell_agent_path_traversal_rejected() {
        let dir = tempdir().unwrap();
        let block = make_code_block(CodeAction::Create, "evil", Some("../../etc/passwd"));
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure for path traversal");
        assert!(
            resp.output.contains(".."),
            "output should mention traversal: {}",
            resp.output
        );
    }

    #[test]
    fn test_shell_agent_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let block = make_code_block(CodeAction::Create, "content", Some("sub/dir/file.txt"));
        let req = make_request("n1", vec![block], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success, "output: {}", resp.output);
        assert!(dir.path().join("sub/dir/file.txt").exists());
    }

    #[test]
    fn test_shell_agent_no_code_blocks_returns_success() {
        let dir = tempdir().unwrap();
        let req = make_request("n1", vec![], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(resp.success);
    }

    #[test]
    fn test_shell_agent_multiple_blocks_first_fail_stops() {
        let dir = tempdir().unwrap();
        // First block: Create on existing file (will fail)
        fs::write(dir.path().join("exists.txt"), "original").unwrap();
        let block1 = make_code_block(CodeAction::Create, "new", Some("exists.txt"));
        // Second block: would succeed if reached
        let block2 = make_code_block(CodeAction::Create, "content", Some("second.txt"));
        let req = make_request("n1", vec![block1, block2], dir.path());
        let resp = ShellAgent.execute(req).unwrap();
        assert!(!resp.success, "expected failure on first block");
        // Second file must NOT have been created
        assert!(
            !dir.path().join("second.txt").exists(),
            "second block should not have executed"
        );
    }

    // -----------------------------------------------------------------------
    // MockAgent tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mock_agent_name_returns_mock_agent() {
        assert_eq!(MockAgent::new().name(), "mock-agent");
    }

    #[test]
    fn test_mock_agent_default_success_true() {
        let dir = tempdir().unwrap();
        let agent = MockAgent::new();
        let req = make_request("n1", vec![], dir.path());
        let resp = agent.execute(req).unwrap();
        assert!(resp.success);
    }

    #[test]
    fn test_mock_agent_default_success_false() {
        let dir = tempdir().unwrap();
        let agent = MockAgent::with_default(false);
        let req = make_request("n1", vec![], dir.path());
        let resp = agent.execute(req).unwrap();
        assert!(!resp.success);
    }

    #[test]
    fn test_mock_agent_with_response_returns_configured() {
        let dir = tempdir().unwrap();
        let configured = AgentResponse {
            success: true,
            output: "custom output".to_owned(),
            duration: Duration::from_millis(42),
        };
        let agent = MockAgent::new().with_response("n1", configured.clone());
        let req = make_request("n1", vec![], dir.path());
        let resp = agent.execute(req).unwrap();
        assert_eq!(resp.output, configured.output);
        assert_eq!(resp.success, configured.success);
    }

    #[test]
    fn test_mock_agent_with_response_uses_default_for_other() {
        let dir = tempdir().unwrap();
        let configured = AgentResponse {
            success: false,
            output: "node1 specific".to_owned(),
            duration: Duration::ZERO,
        };
        let agent = MockAgent::new().with_response("n1", configured);
        // Request for a different node_id => falls back to default (success: true)
        let req = make_request("n2", vec![], dir.path());
        let resp = agent.execute(req).unwrap();
        assert!(resp.success, "should use default for unconfigured node");
    }

    #[test]
    fn test_mock_agent_records_requests() {
        let dir = tempdir().unwrap();
        let agent = MockAgent::new();
        for i in 0..3u8 {
            let req = make_request(&format!("n{}", i), vec![], dir.path());
            agent.execute(req).unwrap();
        }
        let recorded = agent.received_requests();
        assert_eq!(recorded.len(), 3);
        assert_eq!(recorded[0].node_id, "n0");
        assert_eq!(recorded[1].node_id, "n1");
        assert_eq!(recorded[2].node_id, "n2");
    }

    #[test]
    fn test_mock_agent_with_delay_blocks() {
        let dir = tempdir().unwrap();
        let delay = Duration::from_millis(100);
        let agent = MockAgent::new().with_delay(delay);
        let req = make_request("n1", vec![], dir.path());
        let start = Instant::now();
        agent.execute(req).unwrap();
        assert!(
            start.elapsed() >= delay,
            "elapsed {:?} < expected delay {:?}",
            start.elapsed(),
            delay
        );
    }

    // -----------------------------------------------------------------------
    // Helper function tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_shell_command_sh_no_target_true() {
        let block = CodeBlock {
            lang: Some("sh".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "echo hi".to_owned(),
            anchor: None,
            old: None,
        };
        assert!(is_shell_command(&block));
    }

    #[test]
    fn test_is_shell_command_bash_with_target_false() {
        let block = CodeBlock {
            lang: Some("bash".to_owned()),
            target: Some("script.sh".to_owned()),
            action: CodeAction::Create,
            body: "echo hi".to_owned(),
            anchor: None,
            old: None,
        };
        assert!(!is_shell_command(&block));
    }

    #[test]
    fn test_is_shell_command_rust_no_target_false() {
        let block = CodeBlock {
            lang: Some("rust".to_owned()),
            target: None,
            action: CodeAction::Full,
            body: "fn main() {}".to_owned(),
            anchor: None,
            old: None,
        };
        assert!(!is_shell_command(&block));
    }
}
