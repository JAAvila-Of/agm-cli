//! Execution state tracker: state machine, file I/O, and high-level API.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use agm_core::graph::{AgmGraph, RelationKind, transitive_dependents};
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::file::AgmFile;
use agm_core::model::state::{NodeState, StateFile};
use agm_core::parser::state::parse_state;
use agm_core::renderer::state::render_state;

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn now_iso8601() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("RFC 3339 formatting cannot fail")
}

fn generate_session_id() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "run-{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Reconciles an existing `StateFile` with the current `AgmFile`.
///
/// 1. Prunes nodes in `state` that are NOT in `file` (stale entries)
/// 2. Adds new nodes from `file` that are NOT in `state` (new nodes since last run)
///    - New nodes with no unmet deps -> Ready
///    - New nodes with unmet deps -> Pending
/// 3. Preserves existing statuses for nodes that are in both
fn reconcile_state(state: &mut StateFile, file: &AgmFile, _graph: &AgmGraph) {
    // Collect current node IDs from the file
    let file_node_ids: std::collections::HashSet<String> =
        file.nodes.iter().map(|n| n.id.clone()).collect();

    // Step 1: Prune stale entries
    state.nodes.retain(|id, _| file_node_ids.contains(id));

    // Step 2: Add new nodes
    for node in &file.nodes {
        if !state.nodes.contains_key(&node.id) {
            let has_unmet_deps = node
                .depends
                .as_ref()
                .map(|deps| !deps.is_empty())
                .unwrap_or(false);

            let status = if has_unmet_deps {
                ExecutionStatus::Pending
            } else {
                ExecutionStatus::Ready
            };

            state.nodes.insert(
                node.id.clone(),
                NodeState {
                    execution_status: status,
                    executed_by: None,
                    executed_at: None,
                    execution_log: None,
                    retry_count: 0,
                },
            );
        }
    }
    // Step 3: existing entries are untouched (retained in step 1)
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Validates whether a state transition from `from` to `to` is allowed
/// per spec S26.4.
///
/// Returns `Ok(())` if the transition is valid, or `Err(String)` with a
/// human-readable message describing why the transition is invalid.
pub fn validate_transition(from: &ExecutionStatus, to: &ExecutionStatus) -> Result<(), String> {
    let valid = matches!(
        (from, to),
        // pending -> ready, blocked, skipped
        (ExecutionStatus::Pending, ExecutionStatus::Ready)
            | (ExecutionStatus::Pending, ExecutionStatus::Blocked)
            | (ExecutionStatus::Pending, ExecutionStatus::Skipped)
            // ready -> in_progress
            | (ExecutionStatus::Ready, ExecutionStatus::InProgress)
            // in_progress -> completed, failed
            | (ExecutionStatus::InProgress, ExecutionStatus::Completed)
            | (ExecutionStatus::InProgress, ExecutionStatus::Failed)
            // failed -> ready (retry)
            | (ExecutionStatus::Failed, ExecutionStatus::Ready)
            // blocked -> ready (when blocker resolved)
            | (ExecutionStatus::Blocked, ExecutionStatus::Ready)
    );

    if valid {
        Ok(())
    } else {
        Err(format!("Invalid state transition: {} -> {}", from, to))
    }
}

/// Derives the `.agm.state` sidecar path from the `.agm` file path.
///
/// `"foo.agm"` -> `"foo.agm.state"`
/// `"/home/user/project/migration.agm"` -> `"/home/user/project/migration.agm.state"`
#[must_use]
pub fn state_path(agm_path: &Path) -> PathBuf {
    let mut p = agm_path.as_os_str().to_owned();
    p.push(".state");
    PathBuf::from(p)
}

/// Loads an existing `.agm.state` sidecar file.
///
/// Returns `Ok(None)` when no sidecar file exists at the expected path.
/// Returns `Err` if the file exists but cannot be read or parsed.
pub fn load_state(agm_path: &Path) -> Result<Option<StateFile>> {
    let path = state_path(agm_path);
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read state file: {}", path.display()))?;
    let state = parse_state(&content).map_err(|errors| {
        anyhow::anyhow!(
            "Failed to parse state file {}: {}",
            path.display(),
            errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    Ok(Some(state))
}

/// Saves state atomically: writes to a temp file, then renames.
///
/// The temp file is created in the same directory as the target to ensure
/// the rename is atomic (same filesystem).
pub fn save_state(agm_path: &Path, state: &StateFile) -> Result<()> {
    let path = state_path(agm_path);
    let tmp_path = {
        let mut p = path.as_os_str().to_owned();
        p.push(".tmp");
        PathBuf::from(p)
    };

    let content = render_state(state);

    // Write to temp file
    let mut file = std::fs::File::create(&tmp_path)
        .with_context(|| format!("Failed to create temp state file: {}", tmp_path.display()))?;
    file.write_all(content.as_bytes())
        .with_context(|| format!("Failed to write temp state file: {}", tmp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("Failed to sync temp state file: {}", tmp_path.display()))?;

    // Atomic rename
    std::fs::rename(&tmp_path, &path).with_context(|| {
        format!(
            "Failed to rename {} -> {}",
            tmp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ExecutionProgress
// ---------------------------------------------------------------------------

/// Summary of execution progress across all nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionProgress {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub blocked: usize,
    pub skipped: usize,
    pub pending: usize,
    pub ready: usize,
    pub in_progress: usize,
    /// Percentage of nodes that are done: `(completed + skipped) / total * 100.0`.
    /// Returns `0.0` when `total == 0`.
    pub percent: f64,
}

// ---------------------------------------------------------------------------
// ExecutionTracker
// ---------------------------------------------------------------------------

/// High-level API for managing execution state of an AGM file.
///
/// Wraps a `StateFile` and an `AgmGraph`, providing state machine
/// transitions, blocked propagation, and file I/O.
pub struct ExecutionTracker {
    agm_path: PathBuf,
    state: StateFile,
    graph: AgmGraph,
}

impl ExecutionTracker {
    /// Creates a new tracker by loading existing state or initializing fresh.
    ///
    /// If a `.agm.state` sidecar exists, loads it and reconciles with the
    /// current file (preserving existing statuses, pruning stale nodes,
    /// adding new nodes). If no sidecar exists, initializes from scratch.
    pub fn new(agm_path: &Path, file: &AgmFile, graph: &AgmGraph) -> Result<Self> {
        let existing = load_state(agm_path)?;
        let state = match existing {
            Some(mut existing_state) => {
                reconcile_state(&mut existing_state, file, graph);
                // Update the updated_at timestamp
                existing_state.updated_at = now_iso8601();
                existing_state
            }
            None => {
                let session_id = generate_session_id();
                Self::initialize(file, graph, &session_id)
            }
        };
        Ok(Self {
            agm_path: agm_path.to_owned(),
            state,
            graph: graph.clone(),
        })
    }

    /// Creates a fresh `StateFile` for the given AGM file.
    ///
    /// - Nodes with no `depends` (or all deps already satisfied) -> `Ready`
    /// - Nodes with unmet `depends` -> `Pending`
    /// - Session ID is the provided string
    /// - Timestamps are set to current time
    pub fn initialize(file: &AgmFile, _graph: &AgmGraph, session_id: &str) -> StateFile {
        let now = now_iso8601();
        let mut nodes = BTreeMap::new();

        for node in &file.nodes {
            let has_unmet_deps = node
                .depends
                .as_ref()
                .map(|deps| !deps.is_empty())
                .unwrap_or(false);

            let status = if has_unmet_deps {
                ExecutionStatus::Pending
            } else {
                ExecutionStatus::Ready
            };

            nodes.insert(
                node.id.clone(),
                NodeState {
                    execution_status: status,
                    executed_by: None,
                    executed_at: None,
                    execution_log: None,
                    retry_count: 0,
                },
            );
        }

        StateFile {
            format_version: "1.0".to_owned(),
            package: file.header.package.clone(),
            version: file.header.version.clone(),
            session_id: session_id.to_owned(),
            started_at: now.clone(),
            updated_at: now,
            nodes,
        }
    }

    /// Validates and applies a state transition for a node.
    ///
    /// Returns `Err` if:
    /// - The node does not exist in the state
    /// - The transition is invalid per spec S26.4
    pub fn transition(&mut self, node_id: &str, to: ExecutionStatus) -> Result<()> {
        let node_state = self
            .state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| anyhow::anyhow!("Node '{}' not found in state", node_id))?;

        validate_transition(&node_state.execution_status, &to)
            .map_err(|msg| anyhow::anyhow!("{}", msg))?;

        node_state.execution_status = to;
        self.state.updated_at = now_iso8601();
        Ok(())
    }

    /// Marks a node as completed with agent info and optional log path.
    ///
    /// Validates: `InProgress -> Completed`.
    /// Sets `executed_by`, `executed_at` (current time), and optionally `execution_log`.
    pub fn mark_completed(
        &mut self,
        node_id: &str,
        agent: &str,
        log: Option<String>,
    ) -> Result<()> {
        let node_state = self
            .state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| anyhow::anyhow!("Node '{}' not found in state", node_id))?;

        validate_transition(&node_state.execution_status, &ExecutionStatus::Completed)
            .map_err(|msg| anyhow::anyhow!("{}", msg))?;

        node_state.execution_status = ExecutionStatus::Completed;
        node_state.executed_by = Some(agent.to_owned());
        node_state.executed_at = Some(now_iso8601());
        node_state.execution_log = log;

        self.state.updated_at = now_iso8601();
        Ok(())
    }

    /// Marks a node as failed with agent info and optional log path.
    ///
    /// Validates: `InProgress -> Failed`.
    /// Sets `executed_by`, `executed_at`, and optionally `execution_log`.
    /// Does NOT automatically propagate blocked -- caller must call
    /// `propagate_blocked` separately if desired.
    pub fn mark_failed(&mut self, node_id: &str, agent: &str, log: Option<String>) -> Result<()> {
        let node_state = self
            .state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| anyhow::anyhow!("Node '{}' not found in state", node_id))?;

        validate_transition(&node_state.execution_status, &ExecutionStatus::Failed)
            .map_err(|msg| anyhow::anyhow!("{}", msg))?;

        node_state.execution_status = ExecutionStatus::Failed;
        node_state.executed_by = Some(agent.to_owned());
        node_state.executed_at = Some(now_iso8601());
        node_state.execution_log = log;

        self.state.updated_at = now_iso8601();
        Ok(())
    }

    /// Retries a failed node: transitions `Failed -> Ready` and increments `retry_count`.
    ///
    /// Returns `Err` if the node is not in `Failed` state.
    pub fn retry(&mut self, node_id: &str) -> Result<()> {
        let node_state = self
            .state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| anyhow::anyhow!("Node '{}' not found in state", node_id))?;

        validate_transition(&node_state.execution_status, &ExecutionStatus::Ready)
            .map_err(|msg| anyhow::anyhow!("{}", msg))?;

        node_state.execution_status = ExecutionStatus::Ready;
        node_state.retry_count += 1;
        // Clear execution metadata for the new attempt
        node_state.executed_by = None;
        node_state.executed_at = None;
        node_state.execution_log = None;

        self.state.updated_at = now_iso8601();
        Ok(())
    }

    /// Resets ALL nodes to `Pending`, clears all execution metadata,
    /// and resets retry counts to 0.
    ///
    /// After reset, the caller should re-evaluate which nodes are Ready
    /// (i.e., call `initialize` or use `ready_nodes`).
    pub fn reset(&mut self) -> Result<()> {
        for node_state in self.state.nodes.values_mut() {
            *node_state = NodeState::default();
        }
        self.state.updated_at = now_iso8601();
        Ok(())
    }

    /// Resets a single node to `Pending`, clears its execution metadata,
    /// and resets its retry count to 0.
    ///
    /// Returns `Err` if the node does not exist.
    pub fn reset_node(&mut self, node_id: &str) -> Result<()> {
        let node_state = self
            .state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| anyhow::anyhow!("Node '{}' not found in state", node_id))?;

        *node_state = NodeState::default();
        self.state.updated_at = now_iso8601();
        Ok(())
    }

    /// Returns node IDs that are currently `Ready` for execution.
    ///
    /// A node is ready if:
    /// 1. Its current status is `Ready`, OR
    /// 2. Its current status is `Pending` AND all its dependencies are
    ///    `Completed` or `Skipped`
    ///
    /// Case 2 handles the scenario where a node was `Pending` and its
    /// dependencies have since been completed (promotes to Ready).
    ///
    /// Returns IDs in the order they appear in the `BTreeMap` (lexicographic).
    #[must_use]
    pub fn ready_nodes(&self) -> Vec<&str> {
        self.state
            .nodes
            .iter()
            .filter(|(node_id, ns)| match ns.execution_status {
                ExecutionStatus::Ready => true,
                ExecutionStatus::Pending => {
                    // Check if all dependencies are completed/skipped
                    self.all_deps_satisfied(node_id)
                }
                _ => false,
            })
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Returns `true` if all `Depends` edges for this node point to nodes
    /// that are `Completed` or `Skipped`.
    fn all_deps_satisfied(&self, node_id: &str) -> bool {
        let deps = self.graph.edges_of_kind(node_id, RelationKind::Depends);
        deps.iter().all(|dep_id| {
            self.state
                .nodes
                .get(*dep_id)
                .map(|ns| {
                    matches!(
                        ns.execution_status,
                        ExecutionStatus::Completed | ExecutionStatus::Skipped
                    )
                })
                .unwrap_or(false) // unknown dep -> not satisfied
        })
    }

    /// Computes execution progress summary.
    #[must_use]
    pub fn progress(&self) -> ExecutionProgress {
        let total = self.state.nodes.len();
        let mut completed = 0;
        let mut failed = 0;
        let mut blocked = 0;
        let mut skipped = 0;
        let mut pending = 0;
        let mut ready = 0;
        let mut in_progress = 0;

        for ns in self.state.nodes.values() {
            match ns.execution_status {
                ExecutionStatus::Completed => completed += 1,
                ExecutionStatus::Failed => failed += 1,
                ExecutionStatus::Blocked => blocked += 1,
                ExecutionStatus::Skipped => skipped += 1,
                ExecutionStatus::Pending => pending += 1,
                ExecutionStatus::Ready => ready += 1,
                ExecutionStatus::InProgress => in_progress += 1,
            }
        }

        let percent = if total == 0 {
            0.0
        } else {
            (completed + skipped) as f64 / total as f64 * 100.0
        };

        ExecutionProgress {
            total,
            completed,
            failed,
            blocked,
            skipped,
            pending,
            ready,
            in_progress,
            percent,
        }
    }

    /// When a node fails, propagates `Blocked` to all transitive dependents.
    ///
    /// Only nodes currently in `Pending` or `Ready` are blocked. Nodes that
    /// are already `Completed`, `Skipped`, `InProgress`, `Failed`, or
    /// `Blocked` are left unchanged.
    ///
    /// Returns the list of node IDs that were newly set to `Blocked`.
    pub fn propagate_blocked(&mut self, failed_node: &str) -> Vec<String> {
        let dependents = transitive_dependents(&self.graph, failed_node);
        let mut newly_blocked = Vec::new();

        for dep_id in &dependents {
            if let Some(ns) = self.state.nodes.get_mut(dep_id.as_str()) {
                if matches!(
                    ns.execution_status,
                    ExecutionStatus::Pending | ExecutionStatus::Ready
                ) {
                    ns.execution_status = ExecutionStatus::Blocked;
                    newly_blocked.push(dep_id.clone());
                }
            }
        }

        if !newly_blocked.is_empty() {
            self.state.updated_at = now_iso8601();
        }

        newly_blocked
    }

    /// Saves the current state to disk via atomic write.
    pub fn save(&self) -> Result<()> {
        save_state(&self.agm_path, &self.state)
    }

    /// Returns a reference to the underlying `StateFile`.
    #[must_use]
    pub fn state(&self) -> &StateFile {
        &self.state
    }

    /// Returns a reference to a specific node's state.
    #[must_use]
    pub fn node_state(&self, node_id: &str) -> Option<&NodeState> {
        self.state.nodes.get(node_id)
    }
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
    use agm_core::model::node::Node;
    use std::collections::BTreeMap;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn test_header() -> Header {
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

    fn test_node(id: &str) -> Node {
        Node {
            id: id.to_owned(),
            node_type: NodeType::Facts,
            summary: format!("test node {id}"),
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
            verify: None,
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
            span: Span::new(1, 1),
        }
    }

    fn test_node_with_deps(id: &str, deps: Vec<&str>) -> Node {
        let mut node = test_node(id);
        node.depends = Some(deps.into_iter().map(str::to_owned).collect());
        node
    }

    fn test_file(nodes: Vec<Node>) -> AgmFile {
        AgmFile {
            header: test_header(),
            nodes,
        }
    }

    fn test_state(nodes: BTreeMap<String, NodeState>) -> StateFile {
        StateFile {
            format_version: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            session_id: "run-test".to_owned(),
            started_at: "2026-04-08T10:00:00Z".to_owned(),
            updated_at: "2026-04-08T10:00:00Z".to_owned(),
            nodes,
        }
    }

    fn make_tracker_from_state(state: StateFile, file: &AgmFile) -> ExecutionTracker {
        let graph = build_graph(file);
        ExecutionTracker {
            agm_path: PathBuf::from("test.agm"),
            state,
            graph,
        }
    }

    // -----------------------------------------------------------------------
    // Group A: validate_transition -- valid cases (8 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_transition_pending_to_ready_returns_ok() {
        assert!(validate_transition(&ExecutionStatus::Pending, &ExecutionStatus::Ready).is_ok());
    }

    #[test]
    fn test_validate_transition_pending_to_blocked_returns_ok() {
        assert!(validate_transition(&ExecutionStatus::Pending, &ExecutionStatus::Blocked).is_ok());
    }

    #[test]
    fn test_validate_transition_pending_to_skipped_returns_ok() {
        assert!(validate_transition(&ExecutionStatus::Pending, &ExecutionStatus::Skipped).is_ok());
    }

    #[test]
    fn test_validate_transition_ready_to_in_progress_returns_ok() {
        assert!(validate_transition(&ExecutionStatus::Ready, &ExecutionStatus::InProgress).is_ok());
    }

    #[test]
    fn test_validate_transition_in_progress_to_completed_returns_ok() {
        assert!(
            validate_transition(&ExecutionStatus::InProgress, &ExecutionStatus::Completed).is_ok()
        );
    }

    #[test]
    fn test_validate_transition_in_progress_to_failed_returns_ok() {
        assert!(
            validate_transition(&ExecutionStatus::InProgress, &ExecutionStatus::Failed).is_ok()
        );
    }

    #[test]
    fn test_validate_transition_failed_to_ready_returns_ok() {
        assert!(validate_transition(&ExecutionStatus::Failed, &ExecutionStatus::Ready).is_ok());
    }

    #[test]
    fn test_validate_transition_blocked_to_ready_returns_ok() {
        assert!(validate_transition(&ExecutionStatus::Blocked, &ExecutionStatus::Ready).is_ok());
    }

    // -----------------------------------------------------------------------
    // Group B: validate_transition -- invalid cases (6 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_transition_completed_to_ready_returns_err() {
        assert!(validate_transition(&ExecutionStatus::Completed, &ExecutionStatus::Ready).is_err());
    }

    #[test]
    fn test_validate_transition_completed_to_pending_returns_err() {
        assert!(
            validate_transition(&ExecutionStatus::Completed, &ExecutionStatus::Pending).is_err()
        );
    }

    #[test]
    fn test_validate_transition_skipped_to_ready_returns_err() {
        assert!(validate_transition(&ExecutionStatus::Skipped, &ExecutionStatus::Ready).is_err());
    }

    #[test]
    fn test_validate_transition_in_progress_to_pending_returns_err() {
        assert!(
            validate_transition(&ExecutionStatus::InProgress, &ExecutionStatus::Pending).is_err()
        );
    }

    #[test]
    fn test_validate_transition_ready_to_pending_returns_err() {
        assert!(validate_transition(&ExecutionStatus::Ready, &ExecutionStatus::Pending).is_err());
    }

    #[test]
    fn test_validate_transition_pending_to_pending_returns_err() {
        assert!(validate_transition(&ExecutionStatus::Pending, &ExecutionStatus::Pending).is_err());
    }

    // -----------------------------------------------------------------------
    // Group C: state_path (2 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_state_path_appends_state_extension() {
        let p = state_path(Path::new("foo.agm"));
        assert_eq!(p, PathBuf::from("foo.agm.state"));
    }

    #[test]
    fn test_state_path_absolute_path_preserved() {
        let p = state_path(Path::new("/home/user/project/file.agm"));
        assert_eq!(p, PathBuf::from("/home/user/project/file.agm.state"));
    }

    // -----------------------------------------------------------------------
    // Group D: initialize (3 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_initialize_no_deps_all_ready() {
        let file = test_file(vec![test_node("a"), test_node("b"), test_node("c")]);
        let graph = build_graph(&file);
        let state = ExecutionTracker::initialize(&file, &graph, "run-test");

        assert_eq!(state.nodes.len(), 3);
        for ns in state.nodes.values() {
            assert_eq!(ns.execution_status, ExecutionStatus::Ready);
        }
    }

    #[test]
    fn test_initialize_with_deps_sets_pending() {
        let node_b = test_node("b");
        let node_a = test_node_with_deps("a", vec!["b"]);
        let file = test_file(vec![node_a, node_b]);
        let graph = build_graph(&file);
        let state = ExecutionTracker::initialize(&file, &graph, "run-test");

        assert_eq!(state.nodes["a"].execution_status, ExecutionStatus::Pending);
        assert_eq!(state.nodes["b"].execution_status, ExecutionStatus::Ready);
    }

    #[test]
    fn test_initialize_empty_file_returns_empty_nodes() {
        let file = test_file(vec![]);
        let graph = build_graph(&file);
        let state = ExecutionTracker::initialize(&file, &graph, "run-test");
        assert!(state.nodes.is_empty());
    }

    // -----------------------------------------------------------------------
    // Group E: reconcile_state (3 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_reconcile_preserves_existing_statuses() {
        let file = test_file(vec![test_node("a"), test_node("b")]);
        let graph = build_graph(&file);

        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                executed_by: Some("agent-1".to_owned()),
                executed_at: Some("2026-04-08T10:00:00Z".to_owned()),
                execution_log: None,
                retry_count: 0,
            },
        );
        nodes.insert("b".to_owned(), NodeState::default());
        let mut state = test_state(nodes);

        reconcile_state(&mut state, &file, &graph);

        assert_eq!(
            state.nodes["a"].execution_status,
            ExecutionStatus::Completed
        );
        assert_eq!(state.nodes["a"].executed_by, Some("agent-1".to_owned()));
    }

    #[test]
    fn test_reconcile_prunes_stale_entries() {
        let file = test_file(vec![test_node("a")]);
        let graph = build_graph(&file);

        let mut nodes = BTreeMap::new();
        nodes.insert("a".to_owned(), NodeState::default());
        nodes.insert("old".to_owned(), NodeState::default()); // stale
        let mut state = test_state(nodes);

        reconcile_state(&mut state, &file, &graph);

        assert!(state.nodes.contains_key("a"));
        assert!(!state.nodes.contains_key("old"));
    }

    #[test]
    fn test_reconcile_adds_new_nodes() {
        let file = test_file(vec![test_node("a"), test_node("new")]);
        let graph = build_graph(&file);

        let mut nodes = BTreeMap::new();
        nodes.insert("a".to_owned(), NodeState::default());
        // "new" is not in state yet
        let mut state = test_state(nodes);

        reconcile_state(&mut state, &file, &graph);

        assert!(state.nodes.contains_key("new"));
        assert_eq!(state.nodes["new"].execution_status, ExecutionStatus::Ready);
    }

    // -----------------------------------------------------------------------
    // Group F: mark_completed and mark_failed (3 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_mark_completed_sets_fields() {
        let file = test_file(vec![test_node("a")]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::InProgress,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        tracker
            .mark_completed("a", "agent-1", Some("log.txt".to_owned()))
            .unwrap();

        let ns = tracker.node_state("a").unwrap();
        assert_eq!(ns.execution_status, ExecutionStatus::Completed);
        assert_eq!(ns.executed_by, Some("agent-1".to_owned()));
        assert!(ns.executed_at.is_some());
        assert_eq!(ns.execution_log, Some("log.txt".to_owned()));
    }

    #[test]
    fn test_mark_failed_sets_fields() {
        let file = test_file(vec![test_node("a")]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::InProgress,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        tracker.mark_failed("a", "agent-1", None).unwrap();

        let ns = tracker.node_state("a").unwrap();
        assert_eq!(ns.execution_status, ExecutionStatus::Failed);
        assert_eq!(ns.executed_by, Some("agent-1".to_owned()));
        assert!(ns.executed_at.is_some());
        assert!(ns.execution_log.is_none());
    }

    #[test]
    fn test_mark_completed_from_pending_returns_err() {
        let file = test_file(vec![test_node("a")]);
        let mut nodes = BTreeMap::new();
        nodes.insert("a".to_owned(), NodeState::default()); // Pending
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        let result = tracker.mark_completed("a", "agent-1", None);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Group G: retry (2 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_retry_failed_to_ready_increments_count() {
        let file = test_file(vec![test_node("a")]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Failed,
                retry_count: 0,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        tracker.retry("a").unwrap();

        let ns = tracker.node_state("a").unwrap();
        assert_eq!(ns.execution_status, ExecutionStatus::Ready);
        assert_eq!(ns.retry_count, 1);
        assert!(ns.executed_by.is_none());
    }

    #[test]
    fn test_retry_completed_returns_err() {
        let file = test_file(vec![test_node("a")]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        let result = tracker.retry("a");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Group H: reset and reset_node (2 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_reset_sets_all_nodes_to_pending() {
        let file = test_file(vec![test_node("a"), test_node("b")]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                executed_by: Some("agent".to_owned()),
                retry_count: 2,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Failed,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        tracker.reset().unwrap();

        for ns in tracker.state().nodes.values() {
            assert_eq!(ns.execution_status, ExecutionStatus::Pending);
            assert_eq!(ns.retry_count, 0);
            assert!(ns.executed_by.is_none());
        }
    }

    #[test]
    fn test_reset_node_resets_single_node() {
        let file = test_file(vec![test_node("a"), test_node("b")]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                retry_count: 1,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                retry_count: 3,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        tracker.reset_node("a").unwrap();

        assert_eq!(
            tracker.node_state("a").unwrap().execution_status,
            ExecutionStatus::Pending
        );
        assert_eq!(tracker.node_state("a").unwrap().retry_count, 0);
        // b is unchanged
        assert_eq!(
            tracker.node_state("b").unwrap().execution_status,
            ExecutionStatus::Completed
        );
        assert_eq!(tracker.node_state("b").unwrap().retry_count, 3);
    }

    // -----------------------------------------------------------------------
    // Group I: ready_nodes (3 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ready_nodes_returns_only_ready_status() {
        let file = test_file(vec![test_node("a"), test_node("b"), test_node("c")]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Ready,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "c".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Ready,
                ..NodeState::default()
            },
        );
        let tracker = make_tracker_from_state(test_state(nodes), &file);

        let ready = tracker.ready_nodes();
        assert_eq!(ready.len(), 2);
        assert!(ready.contains(&"a"));
        assert!(ready.contains(&"c"));
    }

    #[test]
    fn test_ready_nodes_promotes_pending_with_completed_deps() {
        let node_a = test_node_with_deps("a", vec!["b"]);
        let node_b = test_node("b");
        let file = test_file(vec![node_a, node_b]);

        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                ..NodeState::default()
            },
        );
        let tracker = make_tracker_from_state(test_state(nodes), &file);

        let ready = tracker.ready_nodes();
        assert!(ready.contains(&"a"), "a should be promoted to ready");
    }

    #[test]
    fn test_ready_nodes_excludes_pending_with_unmet_deps() {
        let node_a = test_node_with_deps("a", vec!["b"]);
        let node_b = test_node("b");
        let file = test_file(vec![node_a, node_b]);

        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending, // not completed
                ..NodeState::default()
            },
        );
        let tracker = make_tracker_from_state(test_state(nodes), &file);

        let ready = tracker.ready_nodes();
        assert!(!ready.contains(&"a"), "a should NOT be ready");
    }

    // -----------------------------------------------------------------------
    // Group J: progress (2 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_progress_counts_correct() {
        let file = test_file(vec![
            test_node("a"),
            test_node("b"),
            test_node("c"),
            test_node("d"),
            test_node("e"),
        ]);
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Failed,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "c".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "d".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Ready,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "e".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Skipped,
                ..NodeState::default()
            },
        );
        let tracker = make_tracker_from_state(test_state(nodes), &file);

        let p = tracker.progress();
        assert_eq!(p.total, 5);
        assert_eq!(p.completed, 1);
        assert_eq!(p.failed, 1);
        assert_eq!(p.pending, 1);
        assert_eq!(p.ready, 1);
        assert_eq!(p.skipped, 1);
        assert_eq!(p.blocked, 0);
        assert_eq!(p.in_progress, 0);
        // (completed + skipped) / total = 2/5 = 40.0
        assert!((p.percent - 40.0).abs() < 0.001);
    }

    #[test]
    fn test_progress_empty_returns_zero_percent() {
        let file = test_file(vec![]);
        let tracker = make_tracker_from_state(test_state(BTreeMap::new()), &file);

        let p = tracker.progress();
        assert_eq!(p.total, 0);
        assert_eq!(p.percent, 0.0);
    }

    // -----------------------------------------------------------------------
    // Group K: propagate_blocked (2 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_propagate_blocked_blocks_transitive_dependents() {
        // A -> B -> C (A depends on B, B depends on C... wait: A failed, B and C depend on A)
        // The graph direction: if A fails, its dependents (those who depend on A) get blocked.
        // B depends_on A, C depends_on B
        let node_a = test_node("a");
        let node_b = test_node_with_deps("b", vec!["a"]);
        let node_c = test_node_with_deps("c", vec!["b"]);
        let file = test_file(vec![node_a, node_b, node_c]);

        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Failed,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "c".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Pending,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        let blocked = tracker.propagate_blocked("a");

        assert!(blocked.contains(&"b".to_owned()));
        assert!(blocked.contains(&"c".to_owned()));
        assert_eq!(
            tracker.node_state("b").unwrap().execution_status,
            ExecutionStatus::Blocked
        );
        assert_eq!(
            tracker.node_state("c").unwrap().execution_status,
            ExecutionStatus::Blocked
        );
    }

    #[test]
    fn test_propagate_blocked_skips_completed_nodes() {
        // B depends on A, but B is already Completed
        let node_a = test_node("a");
        let node_b = test_node_with_deps("b", vec!["a"]);
        let file = test_file(vec![node_a, node_b]);

        let mut nodes = BTreeMap::new();
        nodes.insert(
            "a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Failed,
                ..NodeState::default()
            },
        );
        nodes.insert(
            "b".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                ..NodeState::default()
            },
        );
        let mut tracker = make_tracker_from_state(test_state(nodes), &file);

        let blocked = tracker.propagate_blocked("a");

        // B is already completed, should NOT be blocked
        assert!(!blocked.contains(&"b".to_owned()));
        assert_eq!(
            tracker.node_state("b").unwrap().execution_status,
            ExecutionStatus::Completed
        );
    }

    // -----------------------------------------------------------------------
    // Group L: File I/O integration (2 tests)
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let agm_path = dir.path().join("test.agm");

        // Build a non-trivial state
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "node.a".to_owned(),
            NodeState {
                execution_status: ExecutionStatus::Completed,
                executed_by: Some("agent-1".to_owned()),
                executed_at: Some("2026-04-08T10:00:00Z".to_owned()),
                execution_log: None,
                retry_count: 1,
            },
        );
        nodes.insert("node.b".to_owned(), NodeState::default());
        let original = test_state(nodes);

        save_state(&agm_path, &original).unwrap();

        let loaded = load_state(&agm_path).unwrap().expect("state should exist");
        assert_eq!(original, loaded);
    }

    #[test]
    fn test_load_state_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let agm_path = dir.path().join("nonexistent.agm");

        let result = load_state(&agm_path).unwrap();
        assert!(result.is_none());
    }
}
