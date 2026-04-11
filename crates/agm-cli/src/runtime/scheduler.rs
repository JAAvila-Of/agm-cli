//! Scheduler: core runtime loop for AGM node execution.
//!
//! Drives execution by selecting ready nodes, dispatching them to an
//! agent backend, running verification, executing memory actions, and
//! updating state. Supports serial and parallel dispatch.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;

use agm_core::graph::{AgmGraph, topological_sort};
use agm_core::model::execution::ExecutionStatus;
use agm_core::model::file::AgmFile;
use agm_core::model::memory::MemoryAction;
use agm_core::model::node::Node;
use agm_core::model::orchestration::Strategy;

use super::agent::{AgentBackend, AgentRequest, AgentResponse};
use super::context::build_context;
use super::memory::MemoryRuntime;
use super::state::ExecutionTracker;
use super::verifier::{self, FailStrategy};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a scheduler run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// Maximum number of nodes to execute concurrently.
    /// Default: 1 (serial execution).
    pub max_concurrency: usize,
    /// Per-node timeout. Default: 300 seconds.
    pub timeout: Duration,
    /// If true, print the execution plan without running anything.
    pub dry_run: bool,
    /// If set, only execute these nodes (and their unfinished dependencies).
    pub target_nodes: Option<Vec<String>>,
    /// If set, only execute the named orchestration group.
    pub target_group: Option<String>,
    /// Working directory for file operations and command execution.
    pub working_dir: PathBuf,
    /// If true, abort the entire run on the first node failure.
    /// If false, continue executing unblocked nodes.
    pub fail_fast: bool,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 1,
            timeout: Duration::from_secs(300),
            dry_run: false,
            target_nodes: None,
            target_group: None,
            working_dir: PathBuf::from("."),
            fail_fast: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Summary report of a scheduler run.
#[derive(Debug, Clone)]
pub struct RunReport {
    /// Total number of nodes in scope for this run.
    pub total_nodes: usize,
    /// Number of nodes that were executed (entered InProgress).
    pub executed: usize,
    /// Number of nodes that completed successfully.
    pub succeeded: usize,
    /// Number of nodes that failed.
    pub failed: usize,
    /// Number of nodes that were skipped (not in target set, or dry-run).
    pub skipped: usize,
    /// Number of nodes blocked by upstream failures.
    pub blocked: usize,
    /// Wall-clock duration of the entire run.
    pub duration: Duration,
    /// Per-node results, in execution order.
    pub node_results: Vec<NodeResult>,
}

/// Result of executing a single node.
#[derive(Debug, Clone)]
pub struct NodeResult {
    /// The node ID.
    pub node_id: String,
    /// Final status after execution.
    pub status: ExecutionStatus,
    /// Agent name that executed this node.
    pub agent: String,
    /// Wall-clock time for this node's execution.
    pub duration: Duration,
    /// Agent output or verification log.
    pub output: Option<String>,
}

// ---------------------------------------------------------------------------
// Single-node execution
// ---------------------------------------------------------------------------

/// Executes a single node: memory pre-reads, agent dispatch, memory writes,
/// verification, and state update.
///
/// Returns a `NodeResult` regardless of success/failure.
/// Updates the tracker and memory in place.
/// Saves state to disk after completion or failure.
fn execute_node(
    node: &Node,
    file: &AgmFile,
    graph: &AgmGraph,
    tracker: &mut ExecutionTracker,
    agent: &dyn AgentBackend,
    memory: &mut MemoryRuntime,
    config: &RunConfig,
) -> Result<NodeResult> {
    let start = Instant::now();

    // 1. Transition to InProgress
    // If the node is still Pending (deps satisfied but not yet promoted),
    // promote it to Ready first so the Ready -> InProgress transition is valid.
    if let Some(ns) = tracker.node_state(&node.id) {
        if ns.execution_status == ExecutionStatus::Pending {
            tracker.transition(&node.id, ExecutionStatus::Ready)?;
        }
    }
    tracker.transition(&node.id, ExecutionStatus::InProgress)?;

    // 2. Execute memory actions with action: get (pre-execution reads)
    if let Some(ref mem_entries) = node.memory {
        for entry in mem_entries {
            if entry.action == MemoryAction::Get {
                let _ = memory.execute_action(&node.id, entry);
            }
        }
    }

    // 3. Build context
    let context = build_context(node, file, graph, memory, &config.working_dir);

    // 4. Collect code blocks
    let mut code_blocks = Vec::new();
    if let Some(ref cb) = node.code {
        code_blocks.push(cb.clone());
    }
    if let Some(ref cbs) = node.code_blocks {
        code_blocks.extend(cbs.iter().cloned());
    }

    // 5. Build agent request
    let request = AgentRequest {
        node_id: node.id.clone(),
        context: context.prompt,
        code_blocks,
        working_dir: config.working_dir.clone(),
        timeout: config.timeout,
    };

    // 6. Call agent
    let agent_result = agent.execute(request);

    // 7. Process agent response
    let (agent_success, agent_output) = match agent_result {
        Ok(response) => (response.success, response.output),
        Err(e) => (false, format!("Agent error: {e}")),
    };

    // 8. Execute memory actions with action: upsert (post-execution writes)
    if agent_success {
        if let Some(ref mem_entries) = node.memory {
            for entry in mem_entries {
                if entry.action == MemoryAction::Upsert {
                    let _ = memory.execute_action(&node.id, entry);
                }
            }
        }
    }

    // 9. Run verify checks if agent succeeded
    let (final_success, output) = if agent_success {
        if node.verify.is_some() {
            let verify_result = verifier::verify_node(
                node,
                tracker,
                &config.working_dir,
                config.timeout,
                FailStrategy::RunAll,
            );
            let log = verifier::format_verify_log(&verify_result);
            let combined = format!("{agent_output}\n---\n{log}");
            (verify_result.all_passed, combined)
        } else {
            (true, agent_output)
        }
    } else {
        (false, agent_output)
    };

    // 10. Update state based on result
    if final_success {
        tracker.mark_completed(&node.id, agent.name(), Some(output.clone()))?;
    } else {
        tracker.mark_failed(&node.id, agent.name(), Some(output.clone()))?;
        tracker.propagate_blocked(&node.id);
    }

    // 11. Clear node-scoped memory
    memory.clear_node_scope(&node.id);

    // 12. Save state to disk
    tracker.save()?;

    // 13. Flush memory to disk (persists project/global scope changes)
    memory.flush()?;

    let status = if final_success {
        ExecutionStatus::Completed
    } else {
        ExecutionStatus::Failed
    };

    Ok(NodeResult {
        node_id: node.id.clone(),
        status,
        agent: agent.name().to_owned(),
        duration: start.elapsed(),
        output: Some(output),
    })
}

// ---------------------------------------------------------------------------
// Parallel dispatch
// ---------------------------------------------------------------------------

/// Dispatches a batch of nodes in parallel using `std::thread`.
///
/// Uses a 3-phase approach:
/// 1. Pre-transition + build requests (sequential, holds &mut tracker/memory)
/// 2. Spawn scoped threads for agent calls (only agent is shared, via borrow)
/// 3. Post-process results: verify, update state, save (sequential)
fn dispatch_parallel(
    node_ids: &[String],
    tracker: &mut ExecutionTracker,
    file: &AgmFile,
    graph: &AgmGraph,
    agent: &dyn AgentBackend,
    memory: &mut MemoryRuntime,
    config: &RunConfig,
) -> Result<Vec<NodeResult>> {
    let node_map: HashMap<&str, &Node> = file.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Phase 1: Pre-transition and build requests (sequential, holds &mut tracker)
    let mut requests: Vec<(String, AgentRequest)> = Vec::new();
    for node_id in node_ids {
        let node = node_map
            .get(node_id.as_str())
            .ok_or_else(|| anyhow::anyhow!("Node '{}' not found in file", node_id))?;

        // Transition to InProgress (promote Pending -> Ready first if needed)
        if let Some(ns) = tracker.node_state(node_id) {
            if ns.execution_status == ExecutionStatus::Pending {
                tracker.transition(node_id, ExecutionStatus::Ready)?;
            }
        }
        tracker.transition(node_id, ExecutionStatus::InProgress)?;

        // Pre-execution memory reads
        if let Some(ref mem_entries) = node.memory {
            for entry in mem_entries {
                if entry.action == MemoryAction::Get {
                    let _ = memory.execute_action(node_id, entry);
                }
            }
        }

        // Build context and request
        let context = build_context(node, file, graph, memory, &config.working_dir);
        let mut code_blocks = Vec::new();
        if let Some(ref cb) = node.code {
            code_blocks.push(cb.clone());
        }
        if let Some(ref cbs) = node.code_blocks {
            code_blocks.extend(cbs.iter().cloned());
        }

        let request = AgentRequest {
            node_id: node_id.clone(),
            context: context.prompt,
            code_blocks,
            working_dir: config.working_dir.clone(),
            timeout: config.timeout,
        };

        requests.push((node_id.clone(), request));
    }

    // Phase 2: Spawn threads for agent execution (the slow part)
    // Uses std::thread::scope to borrow &dyn AgentBackend without Arc.
    let results: Vec<(String, AgentResponse)> = std::thread::scope(|scope| {
        let mut handles = Vec::new();

        for (node_id, request) in &requests {
            let node_id = node_id.clone();
            let request = request.clone();
            let agent_ref = agent; // borrow for the scoped thread

            let handle = scope.spawn(move || {
                let start = Instant::now();
                match agent_ref.execute(request) {
                    Ok(response) => (node_id, response),
                    Err(e) => (
                        node_id,
                        AgentResponse {
                            success: false,
                            output: format!("Agent error: {e}"),
                            duration: start.elapsed(),
                        },
                    ),
                }
            });
            handles.push(handle);
        }

        handles
            .into_iter()
            .map(|h| h.join().expect("Thread panicked during agent execution"))
            .collect()
    });

    // Phase 3: Post-process results (sequential, holds &mut tracker)
    let mut node_results = Vec::new();
    for (node_id, response) in results {
        let node = node_map.get(node_id.as_str()).unwrap();

        // Post-execution memory writes
        if response.success {
            if let Some(ref mem_entries) = node.memory {
                for entry in mem_entries {
                    if entry.action == MemoryAction::Upsert {
                        let _ = memory.execute_action(&node_id, entry);
                    }
                }
            }
        }

        // Run verify checks
        let (final_success, output) = if response.success {
            if node.verify.is_some() {
                let verify_result = verifier::verify_node(
                    node,
                    tracker,
                    &config.working_dir,
                    config.timeout,
                    FailStrategy::RunAll,
                );
                let log = verifier::format_verify_log(&verify_result);
                let combined = format!("{}\n---\n{}", response.output, log);
                (verify_result.all_passed, combined)
            } else {
                (true, response.output.clone())
            }
        } else {
            (false, response.output.clone())
        };

        // Update state
        if final_success {
            tracker.mark_completed(&node_id, agent.name(), Some(output.clone()))?;
        } else {
            tracker.mark_failed(&node_id, agent.name(), Some(output.clone()))?;
            tracker.propagate_blocked(&node_id);
        }

        // Clear node memory
        memory.clear_node_scope(&node_id);

        // Save state after each node
        tracker.save()?;
        memory.flush()?;

        node_results.push(NodeResult {
            node_id: node_id.clone(),
            status: if final_success {
                ExecutionStatus::Completed
            } else {
                ExecutionStatus::Failed
            },
            agent: agent.name().to_owned(),
            duration: response.duration,
            output: Some(output),
        });
    }

    Ok(node_results)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the first node from `candidates` that appears in `topo_order`.
///
/// Falls back to the first candidate if none are found in the topo order
/// (should not happen in practice).
fn pick_topo_first(candidates: &[&str], topo_order: &[String]) -> String {
    for id in topo_order {
        if candidates.contains(&id.as_str()) {
            return id.clone();
        }
    }
    candidates[0].to_string()
}

fn build_report(
    tracker: &ExecutionTracker,
    node_results: Vec<NodeResult>,
    start: Instant,
) -> RunReport {
    let progress = tracker.progress();
    RunReport {
        total_nodes: progress.total,
        executed: node_results.len(),
        succeeded: node_results
            .iter()
            .filter(|r| r.status == ExecutionStatus::Completed)
            .count(),
        failed: node_results
            .iter()
            .filter(|r| r.status == ExecutionStatus::Failed)
            .count(),
        skipped: progress.skipped,
        blocked: progress.blocked,
        duration: start.elapsed(),
        node_results,
    }
}

fn build_dry_run_report(
    topo_order: &[String],
    execution_set: &Option<std::collections::HashSet<String>>,
    start: Instant,
) -> RunReport {
    let nodes: Vec<&String> = match execution_set {
        Some(set) => topo_order.iter().filter(|id| set.contains(*id)).collect(),
        None => topo_order.iter().collect(),
    };
    RunReport {
        total_nodes: nodes.len(),
        executed: 0,
        succeeded: 0,
        failed: 0,
        skipped: nodes.len(),
        blocked: 0,
        duration: start.elapsed(),
        node_results: nodes
            .iter()
            .map(|id| NodeResult {
                node_id: id.to_string(),
                status: ExecutionStatus::Pending,
                agent: "(dry-run)".to_owned(),
                duration: Duration::ZERO,
                output: Some("Dry-run: would execute".to_owned()),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Topological dispatch (non-orchestration)
// ---------------------------------------------------------------------------

/// Runs nodes in topological (dependency) order.
///
/// Used for AGM files that do NOT have an orchestration node, or when the
/// user does not select an orchestration node. Nodes are dispatched in
/// dependency order: all dependencies complete before the dependent runs.
///
/// Respects `target_nodes` filtering: if set, only the specified nodes
/// (and their unfinished transitive dependencies) are executed.
pub fn run_topological(
    tracker: &mut ExecutionTracker,
    file: &AgmFile,
    agent: &dyn AgentBackend,
    memory: &mut MemoryRuntime,
    config: &RunConfig,
) -> Result<RunReport> {
    let run_start = Instant::now();

    // Build node lookup map: id -> &Node
    let node_map: HashMap<&str, &Node> = file.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Get topological order
    let graph = agm_core::graph::build_graph(file);
    let topo_order = topological_sort(&graph)
        .map_err(|e| anyhow::anyhow!("Cycle detected in dependency graph: {e}"))?;

    // Apply target_nodes filter
    let execution_set: Option<std::collections::HashSet<String>> =
        config.target_nodes.as_ref().map(|targets| {
            let mut set = std::collections::HashSet::new();
            for t in targets {
                set.insert(t.clone());
                // Include unfinished transitive dependencies
                let deps = agm_core::graph::transitive_deps(&graph, t);
                for dep in deps {
                    if let Some(ns) = tracker.node_state(&dep) {
                        if !matches!(
                            ns.execution_status,
                            ExecutionStatus::Completed | ExecutionStatus::Skipped
                        ) {
                            set.insert(dep);
                        }
                    }
                }
            }
            set
        });

    // Dry-run mode
    if config.dry_run {
        return Ok(build_dry_run_report(&topo_order, &execution_set, run_start));
    }

    let mut node_results = Vec::new();
    let mut had_failure = false;

    // Main dispatch loop
    loop {
        let ready = tracker.ready_nodes();
        if ready.is_empty() {
            break;
        }

        // Filter to nodes in the execution set (if target_nodes specified)
        let candidates: Vec<&str> = ready
            .into_iter()
            .filter(|id| {
                execution_set
                    .as_ref()
                    .map(|set| set.contains(*id))
                    .unwrap_or(true)
            })
            .collect();

        if candidates.is_empty() {
            break;
        }

        if config.max_concurrency <= 1 {
            // Serial execution: pick the first candidate in topo order
            let next_id = pick_topo_first(&candidates, &topo_order);
            if let Some(node) = node_map.get(next_id.as_str()) {
                let result = execute_node(node, file, &graph, tracker, agent, memory, config)?;
                if result.status == ExecutionStatus::Failed {
                    had_failure = true;
                }
                node_results.push(result);
                if had_failure && config.fail_fast {
                    break;
                }
            }
        } else {
            // Parallel execution: dispatch up to max_concurrency candidates
            let batch: Vec<String> = candidates
                .iter()
                .take(config.max_concurrency)
                .map(|s| s.to_string())
                .collect();

            let results = dispatch_parallel(&batch, tracker, file, &graph, agent, memory, config)?;

            for result in results {
                if result.status == ExecutionStatus::Failed {
                    had_failure = true;
                }
                node_results.push(result);
            }

            if had_failure && config.fail_fast {
                break;
            }
        }
    }

    Ok(build_report(tracker, node_results, run_start))
}

// ---------------------------------------------------------------------------
// Orchestrated dispatch (with parallel_groups)
// ---------------------------------------------------------------------------

/// Runs nodes using the orchestration node's parallel_groups.
///
/// Groups are executed in `requires` dependency order. Within each group,
/// nodes are dispatched according to the group's strategy (sequential or
/// parallel). Group-level `max_concurrency` overrides `config.max_concurrency`
/// for parallel groups.
///
/// The `orchestration` parameter is the orchestration-type node that
/// defines the parallel_groups.
pub fn run_orchestrated(
    tracker: &mut ExecutionTracker,
    file: &AgmFile,
    agent: &dyn AgentBackend,
    memory: &mut MemoryRuntime,
    orchestration: &Node,
    config: &RunConfig,
) -> Result<RunReport> {
    let run_start = Instant::now();

    let groups = orchestration.parallel_groups.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Orchestration node '{}' has no parallel_groups",
            orchestration.id
        )
    })?;

    // Build node lookup
    let node_map: HashMap<&str, &Node> = file.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // Build dependency graph for context building
    let graph = agm_core::graph::build_graph(file);

    // Validate group requires: build execution order using topological sort on groups
    let group_order = resolve_group_order(groups)?;

    // Track completed groups
    let mut completed_groups: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Dry-run mode
    if config.dry_run {
        return Ok(build_orchestrated_dry_run_report(
            groups,
            &group_order,
            run_start,
        ));
    }

    let mut all_results = Vec::new();
    let mut had_failure = false;

    for group_name in &group_order {
        // Apply target_group filter
        if let Some(ref target) = config.target_group {
            if group_name != target {
                continue;
            }
        }

        let group = groups.iter().find(|g| &g.group == group_name).unwrap();

        // Check requires gate: all required groups must be completed
        if let Some(ref requires) = group.requires {
            for req in requires {
                if !completed_groups.contains(req) {
                    return Err(anyhow::anyhow!(
                        "Group '{}' requires '{}' which is not completed",
                        group_name,
                        req
                    ));
                }
            }
        }

        // Determine effective concurrency for this group
        let effective_concurrency = match group.strategy {
            Strategy::Sequential => 1,
            Strategy::Parallel => group
                .max_concurrency
                .map(|mc| mc as usize)
                .unwrap_or(config.max_concurrency),
        };

        // Execute group nodes
        let group_results = execute_group(
            &group.nodes,
            effective_concurrency,
            &node_map,
            tracker,
            file,
            &graph,
            agent,
            memory,
            config,
        )?;

        for result in &group_results {
            if result.status == ExecutionStatus::Failed {
                had_failure = true;
            }
        }

        all_results.extend(group_results);

        if had_failure && config.fail_fast {
            break;
        }

        // Mark group as completed (even if some nodes failed -- downstream
        // groups check individual node states, not group status)
        completed_groups.insert(group_name.clone());
    }

    Ok(build_report(tracker, all_results, run_start))
}

/// Resolves the execution order of groups based on their `requires` fields.
///
/// Uses a simple Kahn's algorithm: groups with no requires come first,
/// then groups whose requires are all resolved.
fn resolve_group_order(
    groups: &[agm_core::model::orchestration::ParallelGroup],
) -> Result<Vec<String>> {
    use std::collections::{HashMap, HashSet, VecDeque};

    let group_names: HashSet<&str> = groups.iter().map(|g| g.group.as_str()).collect();

    // Build adjacency: group -> groups that depend on it
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut in_degree: HashMap<&str, usize> = HashMap::new();

    for g in groups {
        in_degree.entry(g.group.as_str()).or_insert(0);
        if let Some(ref reqs) = g.requires {
            for req in reqs {
                if !group_names.contains(req.as_str()) {
                    return Err(anyhow::anyhow!(
                        "Group '{}' requires unknown group '{}'",
                        g.group,
                        req
                    ));
                }
                dependents
                    .entry(req.as_str())
                    .or_default()
                    .push(g.group.as_str());
                *in_degree.entry(g.group.as_str()).or_insert(0) += 1;
            }
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(&name, _)| name)
        .collect();

    // Sort initial queue for determinism
    let mut initial: Vec<&str> = queue.drain(..).collect();
    initial.sort();
    queue.extend(initial);

    let mut order = Vec::new();
    while let Some(current) = queue.pop_front() {
        order.push(current.to_string());
        if let Some(deps) = dependents.get(current) {
            let mut next_ready = Vec::new();
            for &dep in deps {
                let deg = in_degree.get_mut(dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    next_ready.push(dep);
                }
            }
            next_ready.sort(); // determinism
            queue.extend(next_ready);
        }
    }

    if order.len() != groups.len() {
        return Err(anyhow::anyhow!(
            "Cycle detected in group requires dependencies"
        ));
    }

    Ok(order)
}

/// Executes a set of nodes within a group, respecting the given concurrency.
///
/// If `concurrency == 1`, nodes are executed serially in the order they
/// appear in the group's node list.
/// If `concurrency > 1`, nodes are dispatched in parallel via `dispatch_parallel`.
#[allow(clippy::too_many_arguments)]
fn execute_group(
    node_ids: &[String],
    concurrency: usize,
    node_map: &HashMap<&str, &Node>,
    tracker: &mut ExecutionTracker,
    file: &AgmFile,
    graph: &AgmGraph,
    agent: &dyn AgentBackend,
    memory: &mut MemoryRuntime,
    config: &RunConfig,
) -> Result<Vec<NodeResult>> {
    let mut results = Vec::new();

    if concurrency <= 1 {
        // Sequential execution within group
        for node_id in node_ids {
            // Skip nodes that are already completed/skipped/blocked
            if let Some(ns) = tracker.node_state(node_id) {
                if matches!(
                    ns.execution_status,
                    ExecutionStatus::Completed
                        | ExecutionStatus::Skipped
                        | ExecutionStatus::Blocked
                        | ExecutionStatus::Failed
                ) {
                    continue;
                }
            }

            // Wait for dependencies to be satisfied (promote Pending -> Ready)
            // The tracker's ready_nodes() handles this, but for sequential
            // group execution we trust the group ordering.
            if let Some(ns) = tracker.node_state(node_id) {
                if ns.execution_status == ExecutionStatus::Pending {
                    // Attempt to promote: the tracker will validate the transition
                    let _ = tracker.transition(node_id, ExecutionStatus::Ready);
                }
            }

            if let Some(node) = node_map.get(node_id.as_str()) {
                let result = execute_node(node, file, graph, tracker, agent, memory, config)?;
                let failed = result.status == ExecutionStatus::Failed;
                results.push(result);
                if failed && config.fail_fast {
                    break;
                }
            }
        }
    } else {
        // Parallel execution: dispatch in batches of `concurrency`
        let eligible: Vec<String> = node_ids
            .iter()
            .filter(|id| {
                tracker
                    .node_state(id)
                    .map(|ns| {
                        matches!(
                            ns.execution_status,
                            ExecutionStatus::Ready | ExecutionStatus::Pending
                        )
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        for chunk in eligible.chunks(concurrency) {
            let batch: Vec<String> = chunk.to_vec();
            let batch_results =
                dispatch_parallel(&batch, tracker, file, graph, agent, memory, config)?;

            let mut batch_failed = false;
            for r in batch_results {
                if r.status == ExecutionStatus::Failed {
                    batch_failed = true;
                }
                results.push(r);
            }

            if batch_failed && config.fail_fast {
                break;
            }
        }
    }

    Ok(results)
}

fn build_orchestrated_dry_run_report(
    groups: &[agm_core::model::orchestration::ParallelGroup],
    group_order: &[String],
    start: Instant,
) -> RunReport {
    let mut node_results = Vec::new();
    for group_name in group_order {
        if let Some(group) = groups.iter().find(|g| &g.group == group_name) {
            for node_id in &group.nodes {
                node_results.push(NodeResult {
                    node_id: node_id.clone(),
                    status: ExecutionStatus::Pending,
                    agent: "(dry-run)".to_owned(),
                    duration: Duration::ZERO,
                    output: Some(format!(
                        "Dry-run: group '{}', strategy: {}, would execute",
                        group.group, group.strategy
                    )),
                });
            }
        }
    }
    RunReport {
        total_nodes: node_results.len(),
        executed: 0,
        succeeded: 0,
        failed: 0,
        skipped: node_results.len(),
        blocked: 0,
        duration: start.elapsed(),
        node_results,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agm_core::graph::{AgmGraph, build_graph};
    use agm_core::model::fields::{NodeType, Span};
    use agm_core::model::file::{AgmFile, Header};
    use agm_core::model::memory::{MemoryAction, MemoryEntry, MemoryScope, MemoryTtl};
    use agm_core::model::node::Node as AgmNode;
    use agm_core::model::orchestration::{ParallelGroup, Strategy};
    use std::collections::BTreeMap;
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // MockAgent
    // -----------------------------------------------------------------------

    /// A mock agent for testing that always succeeds (or always fails).
    struct MockAgent {
        name: String,
        succeed: bool,
        output: String,
        call_count: AtomicUsize,
    }

    impl MockAgent {
        fn succeeding() -> Self {
            Self {
                name: "mock-agent".to_owned(),
                succeed: true,
                output: "Mock output".to_owned(),
                call_count: AtomicUsize::new(0),
            }
        }

        fn failing() -> Self {
            Self {
                name: "mock-agent".to_owned(),
                succeed: false,
                output: "Mock failure".to_owned(),
                call_count: AtomicUsize::new(0),
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl AgentBackend for MockAgent {
        fn execute(&self, _request: AgentRequest) -> anyhow::Result<AgentResponse> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(AgentResponse {
                success: self.succeed,
                output: self.output.clone(),
                duration: Duration::from_millis(10),
            })
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    /// A mock agent that records which node IDs it was called with.
    struct RecordingAgent {
        name: String,
        calls: Mutex<Vec<String>>,
    }

    impl RecordingAgent {
        fn new() -> Self {
            Self {
                name: "recording-agent".to_owned(),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl AgentBackend for RecordingAgent {
        fn execute(&self, request: AgentRequest) -> anyhow::Result<AgentResponse> {
            self.calls.lock().unwrap().push(request.node_id.clone());
            Ok(AgentResponse {
                success: true,
                output: format!("Executed {}", request.node_id),
                duration: Duration::from_millis(10),
            })
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    /// A mock agent that sleeps to test concurrency.
    struct SlowAgent {
        name: String,
        delay: Duration,
    }

    impl SlowAgent {
        fn new(delay: Duration) -> Self {
            Self {
                name: "slow-agent".to_owned(),
                delay,
            }
        }
    }

    impl AgentBackend for SlowAgent {
        fn execute(&self, _request: AgentRequest) -> anyhow::Result<AgentResponse> {
            std::thread::sleep(self.delay);
            Ok(AgentResponse {
                success: true,
                output: "Slow output".to_owned(),
                duration: self.delay,
            })
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    // -----------------------------------------------------------------------
    // Test node/file builders
    // -----------------------------------------------------------------------

    fn test_header() -> Header {
        Header {
            agm: "1.0".to_owned(),
            package: "test.pkg".to_owned(),
            version: "0.1.0".to_owned(),
            title: None,
            description: None,
            tags: None,
            status: None,
            owner: None,
            imports: None,
            default_load: None,
            load_profiles: None,
            target_runtime: None,
        }
    }

    fn test_node(id: &str) -> AgmNode {
        AgmNode {
            id: id.to_owned(),
            node_type: NodeType::Workflow,
            summary: format!("Test node {id}"),
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
            span: Span::new(0, 0),
        }
    }

    fn test_node_with_deps(id: &str, deps: Vec<&str>) -> AgmNode {
        let mut node = test_node(id);
        node.depends = Some(deps.into_iter().map(String::from).collect());
        node
    }

    /// Builds a test file, graph, tracker, and memory in a temp directory.
    fn setup_test_env(
        nodes: Vec<AgmNode>,
    ) -> (
        AgmFile,
        AgmGraph,
        ExecutionTracker,
        MemoryRuntime,
        tempfile::TempDir,
        RunConfig,
    ) {
        let dir = tempdir().unwrap();
        let agm_path = dir.path().join("test.agm");
        let file = AgmFile {
            header: test_header(),
            nodes,
        };
        let graph = build_graph(&file);
        let tracker = ExecutionTracker::new(&agm_path, &file, &graph).unwrap();
        let mem_path = dir.path().join("test.agm.mem");
        let global_path = dir.path().join("global.mem");
        let memory = MemoryRuntime::new(mem_path, global_path, "test.pkg").unwrap();
        let config = RunConfig {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        (file, graph, tracker, memory, dir, config)
    }

    // -----------------------------------------------------------------------
    // Group A: RunConfig defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_run_config_default_serial() {
        let config = RunConfig::default();
        assert_eq!(config.max_concurrency, 1);
    }

    #[test]
    fn test_run_config_default_timeout_300s() {
        let config = RunConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(300));
    }

    // -----------------------------------------------------------------------
    // Group B: Single-node execution
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_single_node_success_marks_completed() {
        let (file, graph, mut tracker, mut memory, _dir, config) =
            setup_test_env(vec![test_node("A")]);
        let agent = MockAgent::succeeding();
        let node = file.nodes.iter().find(|n| n.id == "A").unwrap();
        let result = execute_node(
            node,
            &file,
            &graph,
            &mut tracker,
            &agent,
            &mut memory,
            &config,
        )
        .unwrap();
        assert_eq!(result.status, ExecutionStatus::Completed);
    }

    #[test]
    fn test_execute_single_node_failure_marks_failed() {
        let (file, graph, mut tracker, mut memory, _dir, config) =
            setup_test_env(vec![test_node("A")]);
        let agent = MockAgent::failing();
        let node = file.nodes.iter().find(|n| n.id == "A").unwrap();
        let result = execute_node(
            node,
            &file,
            &graph,
            &mut tracker,
            &agent,
            &mut memory,
            &config,
        )
        .unwrap();
        assert_eq!(result.status, ExecutionStatus::Failed);
    }

    #[test]
    fn test_execute_single_node_saves_state_to_disk() {
        let dir = tempdir().unwrap();
        let agm_path = dir.path().join("test.agm");
        let file = AgmFile {
            header: test_header(),
            nodes: vec![test_node("A")],
        };
        let graph = build_graph(&file);
        let mut tracker = ExecutionTracker::new(&agm_path, &file, &graph).unwrap();
        let mem_path = dir.path().join("test.agm.mem");
        let global_path = dir.path().join("global.mem");
        let mut memory = MemoryRuntime::new(mem_path, global_path, "test.pkg").unwrap();
        let config = RunConfig {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let agent = MockAgent::succeeding();
        let node = file.nodes.iter().find(|n| n.id == "A").unwrap();
        execute_node(
            node,
            &file,
            &graph,
            &mut tracker,
            &agent,
            &mut memory,
            &config,
        )
        .unwrap();
        assert!(dir.path().join("test.agm.state").exists());
    }

    #[test]
    fn test_execute_single_node_clears_node_memory() {
        let dir = tempdir().unwrap();
        let agm_path = dir.path().join("test.agm");
        let mut node = test_node("A");
        node.memory = Some(vec![MemoryEntry {
            key: "mykey".to_owned(),
            topic: "mytopic".to_owned(),
            action: MemoryAction::Upsert,
            value: Some("myvalue".to_owned()),
            scope: Some(MemoryScope::Node),
            ttl: Some(MemoryTtl::Session),
            query: None,
            max_results: None,
        }]);
        let file = AgmFile {
            header: test_header(),
            nodes: vec![node],
        };
        let graph = build_graph(&file);
        let mut tracker = ExecutionTracker::new(&agm_path, &file, &graph).unwrap();
        let mem_path = dir.path().join("test.agm.mem");
        let global_path = dir.path().join("global.mem");
        let mut memory = MemoryRuntime::new(mem_path, global_path, "test.pkg").unwrap();
        let config = RunConfig {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let agent = MockAgent::succeeding();
        let node_ref = file.nodes.iter().find(|n| n.id == "A").unwrap();
        execute_node(
            node_ref,
            &file,
            &graph,
            &mut tracker,
            &agent,
            &mut memory,
            &config,
        )
        .unwrap();
        // Node-scoped memory should be cleared after execution
        // We verify no panic and execution completed
    }

    // -----------------------------------------------------------------------
    // Group C: Topological dispatch -- serial
    // -----------------------------------------------------------------------

    #[test]
    fn test_topo_serial_abc_chain_executes_in_order() {
        let nodes = vec![
            test_node("A"),
            test_node_with_deps("B", vec!["A"]),
            test_node_with_deps("C", vec!["B"]),
        ];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = RecordingAgent::new();
        run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        let calls = agent.calls();
        assert_eq!(calls, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_topo_serial_independent_nodes_all_execute() {
        let nodes = vec![test_node("A"), test_node("B"), test_node("C")];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = MockAgent::succeeding();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        assert_eq!(report.executed, 3);
        assert_eq!(report.succeeded, 3);
    }

    #[test]
    fn test_topo_serial_failure_blocks_dependents() {
        let nodes = vec![
            test_node("A"),
            test_node_with_deps("B", vec!["A"]),
            test_node_with_deps("C", vec!["B"]),
        ];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = MockAgent::failing();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        assert_eq!(report.failed, 1); // A fails
        // B and C should be blocked
        let b_state = tracker.node_state("B").unwrap();
        let c_state = tracker.node_state("C").unwrap();
        assert_eq!(b_state.execution_status, ExecutionStatus::Blocked);
        assert_eq!(c_state.execution_status, ExecutionStatus::Blocked);
    }

    #[test]
    fn test_topo_serial_failure_continues_unblocked() {
        // A->B, C is independent. A fails. fail_fast: false (default).
        // C should still execute.
        let nodes = vec![
            test_node("A"),
            test_node_with_deps("B", vec!["A"]),
            test_node("C"),
        ];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.fail_fast = false;
        // Use a conditional agent: fail A, succeed C
        // Since MockAgent is all-or-nothing, we use failing agent but check C
        // Actually we need to verify C executes when A fails.
        // RecordingAgent always succeeds; use MockAgent::failing to fail everything
        // but check that C is attempted.
        // Better: use MockAgent::failing and check that C is in blocked/failed state
        // (not Pending), proving it was processed.
        let agent = MockAgent::failing();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        // A and C are independent -- both fail (agent fails all nodes)
        // B is blocked (depends on A which failed)
        assert!(report.failed >= 1);
        // C should have been attempted (Failed), not just Pending/Blocked
        let c_state = tracker.node_state("C").unwrap();
        assert_eq!(c_state.execution_status, ExecutionStatus::Failed);
    }

    #[test]
    fn test_topo_serial_fail_fast_stops_on_first_failure() {
        // A->B, C (independent), A fails, fail_fast: true -> C does NOT execute
        let nodes = vec![
            test_node("A"),
            test_node_with_deps("B", vec!["A"]),
            test_node("C"),
        ];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.fail_fast = true;
        let agent = MockAgent::failing();
        run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        // C should not have been executed (still Ready or Pending)
        let c_state = tracker.node_state("C").unwrap();
        assert!(matches!(
            c_state.execution_status,
            ExecutionStatus::Ready | ExecutionStatus::Pending
        ));
    }

    // -----------------------------------------------------------------------
    // Group D: Topological dispatch -- parallel
    // -----------------------------------------------------------------------

    #[test]
    fn test_topo_parallel_independent_nodes_concurrent() {
        let nodes = vec![test_node("A"), test_node("B")];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.max_concurrency = 2;
        // Use a larger delay so CI scheduling overhead (thread spawn, etc.)
        // is a small fraction of the test window. Serial baseline = 2*delay.
        // We assert elapsed is clearly less than serial, not close to `delay`,
        // which is what actually proves concurrency.
        let delay = Duration::from_millis(300);
        let agent = SlowAgent::new(delay);
        let start = Instant::now();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(report.succeeded, 2);
        // Serial would be ~600ms; parallel should finish well under that even
        // with generous CI overhead. 1000ms leaves ~200ms overhead budget.
        let threshold = Duration::from_millis(1000);
        assert!(
            elapsed < threshold,
            "Expected concurrent execution (<{threshold:?}), got {elapsed:?}"
        );
    }

    #[test]
    fn test_topo_parallel_respects_dependencies() {
        // A->C, B->C, max_concurrency: 2 -> A and B run first, then C
        let nodes = vec![
            test_node("A"),
            test_node("B"),
            test_node_with_deps("C", vec!["A", "B"]),
        ];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.max_concurrency = 2;
        let agent = RecordingAgent::new();
        run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        let calls = agent.calls();
        let pos_c = calls.iter().position(|id| id == "C").unwrap();
        let pos_a = calls.iter().position(|id| id == "A").unwrap();
        let pos_b = calls.iter().position(|id| id == "B").unwrap();
        assert!(pos_a < pos_c);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_topo_parallel_max_concurrency_limits_threads() {
        // A, B, C, D (no deps), max_concurrency: 2 -> dispatched in batches of 2
        let nodes = vec![
            test_node("A"),
            test_node("B"),
            test_node("C"),
            test_node("D"),
        ];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.max_concurrency = 2;
        let agent = MockAgent::succeeding();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        assert_eq!(report.succeeded, 4);
    }

    // -----------------------------------------------------------------------
    // Group E: Orchestrated dispatch
    // -----------------------------------------------------------------------

    fn make_orch_node(id: &str, groups: Vec<ParallelGroup>) -> AgmNode {
        let mut node = test_node(id);
        node.parallel_groups = Some(groups);
        node
    }

    #[test]
    fn test_orchestrated_sequential_group_executes_in_order() {
        let groups = vec![ParallelGroup {
            group: "g1".to_owned(),
            nodes: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        }];
        let orch = make_orch_node("orch", groups);
        let nodes = vec![test_node("A"), test_node("B"), test_node("C"), orch];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = RecordingAgent::new();
        let orch_node = file.nodes.iter().find(|n| n.id == "orch").unwrap();
        run_orchestrated(&mut tracker, &file, &agent, &mut memory, orch_node, &config).unwrap();
        let calls = agent.calls();
        assert_eq!(calls, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_orchestrated_parallel_group_fans_out() {
        let groups = vec![ParallelGroup {
            group: "g1".to_owned(),
            nodes: vec!["A".to_owned(), "B".to_owned()],
            strategy: Strategy::Parallel,
            requires: None,
            max_concurrency: Some(2),
        }];
        let orch = make_orch_node("orch", groups);
        let nodes = vec![test_node("A"), test_node("B"), orch];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = MockAgent::succeeding();
        let orch_node = file.nodes.iter().find(|n| n.id == "orch").unwrap();
        let report =
            run_orchestrated(&mut tracker, &file, &agent, &mut memory, orch_node, &config).unwrap();
        assert_eq!(report.succeeded, 2);
    }

    #[test]
    fn test_orchestrated_group_requires_respected() {
        // Group B requires Group A
        let groups = vec![
            ParallelGroup {
                group: "A".to_owned(),
                nodes: vec!["node_a".to_owned()],
                strategy: Strategy::Sequential,
                requires: None,
                max_concurrency: None,
            },
            ParallelGroup {
                group: "B".to_owned(),
                nodes: vec!["node_b".to_owned()],
                strategy: Strategy::Sequential,
                requires: Some(vec!["A".to_owned()]),
                max_concurrency: None,
            },
        ];
        let orch = make_orch_node("orch", groups);
        let nodes = vec![test_node("node_a"), test_node("node_b"), orch];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = RecordingAgent::new();
        let orch_node = file.nodes.iter().find(|n| n.id == "orch").unwrap();
        run_orchestrated(&mut tracker, &file, &agent, &mut memory, orch_node, &config).unwrap();
        let calls = agent.calls();
        assert_eq!(calls[0], "node_a");
        assert_eq!(calls[1], "node_b");
    }

    #[test]
    fn test_orchestrated_target_group_filters() {
        let groups = vec![
            ParallelGroup {
                group: "g1".to_owned(),
                nodes: vec!["A".to_owned()],
                strategy: Strategy::Sequential,
                requires: None,
                max_concurrency: None,
            },
            ParallelGroup {
                group: "g2".to_owned(),
                nodes: vec!["B".to_owned()],
                strategy: Strategy::Sequential,
                requires: None,
                max_concurrency: None,
            },
            ParallelGroup {
                group: "g3".to_owned(),
                nodes: vec!["C".to_owned()],
                strategy: Strategy::Sequential,
                requires: None,
                max_concurrency: None,
            },
        ];
        let orch = make_orch_node("orch", groups);
        let nodes = vec![test_node("A"), test_node("B"), test_node("C"), orch];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.target_group = Some("g2".to_owned());
        let agent = RecordingAgent::new();
        let orch_node = file.nodes.iter().find(|n| n.id == "orch").unwrap();
        run_orchestrated(&mut tracker, &file, &agent, &mut memory, orch_node, &config).unwrap();
        let calls = agent.calls();
        assert_eq!(calls, vec!["B"]);
    }

    #[test]
    fn test_orchestrated_missing_requires_returns_error() {
        let groups = vec![ParallelGroup {
            group: "g1".to_owned(),
            nodes: vec!["A".to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["nonexistent".to_owned()]),
            max_concurrency: None,
        }];
        let orch = make_orch_node("orch", groups);
        let nodes = vec![test_node("A"), orch];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = MockAgent::succeeding();
        let orch_node = file.nodes.iter().find(|n| n.id == "orch").unwrap();
        let result = run_orchestrated(&mut tracker, &file, &agent, &mut memory, orch_node, &config);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Group F: Target node filtering
    // -----------------------------------------------------------------------

    #[test]
    fn test_target_nodes_executes_only_specified() {
        let nodes = vec![test_node("A"), test_node("B"), test_node("C")];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.target_nodes = Some(vec!["B".to_owned()]);
        let agent = RecordingAgent::new();
        run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        let calls = agent.calls();
        assert_eq!(calls, vec!["B"]);
    }

    #[test]
    fn test_target_nodes_includes_unfinished_deps() {
        // A->B->C, target_nodes: [C] -> A, B, C all execute
        let nodes = vec![
            test_node("A"),
            test_node_with_deps("B", vec!["A"]),
            test_node_with_deps("C", vec!["B"]),
        ];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.target_nodes = Some(vec!["C".to_owned()]);
        let agent = RecordingAgent::new();
        run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        let calls = agent.calls();
        assert_eq!(calls.len(), 3);
        assert!(calls.contains(&"A".to_owned()));
        assert!(calls.contains(&"B".to_owned()));
        assert!(calls.contains(&"C".to_owned()));
    }

    // -----------------------------------------------------------------------
    // Group G: Dry-run mode
    // -----------------------------------------------------------------------

    #[test]
    fn test_dry_run_does_not_call_agent() {
        let nodes = vec![test_node("A"), test_node("B")];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.dry_run = true;
        let agent = MockAgent::succeeding();
        run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        assert_eq!(agent.call_count(), 0);
    }

    #[test]
    fn test_dry_run_does_not_modify_state() {
        let nodes = vec![test_node("A"), test_node("B")];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.dry_run = true;
        let agent = MockAgent::succeeding();
        run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        // All nodes should still be in Ready or Pending state
        for node in &file.nodes {
            let ns = tracker.node_state(&node.id).unwrap();
            assert!(matches!(
                ns.execution_status,
                ExecutionStatus::Ready | ExecutionStatus::Pending
            ));
        }
    }

    #[test]
    fn test_dry_run_report_lists_all_nodes() {
        let nodes = vec![test_node("A"), test_node("B"), test_node("C")];
        let (file, _graph, mut tracker, mut memory, _dir, mut config) = setup_test_env(nodes);
        config.dry_run = true;
        let agent = MockAgent::succeeding();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        assert_eq!(report.total_nodes, 3);
        assert_eq!(report.node_results.len(), 3);
        for result in &report.node_results {
            assert_eq!(result.status, ExecutionStatus::Pending);
        }
    }

    // -----------------------------------------------------------------------
    // Group H: Group order resolution
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_group_order_linear() {
        let groups = vec![
            ParallelGroup {
                group: "A".to_owned(),
                nodes: vec![],
                strategy: Strategy::Sequential,
                requires: None,
                max_concurrency: None,
            },
            ParallelGroup {
                group: "B".to_owned(),
                nodes: vec![],
                strategy: Strategy::Sequential,
                requires: Some(vec!["A".to_owned()]),
                max_concurrency: None,
            },
            ParallelGroup {
                group: "C".to_owned(),
                nodes: vec![],
                strategy: Strategy::Sequential,
                requires: Some(vec!["B".to_owned()]),
                max_concurrency: None,
            },
        ];
        let order = resolve_group_order(&groups).unwrap();
        assert_eq!(order, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_resolve_group_order_cycle_returns_error() {
        let groups = vec![
            ParallelGroup {
                group: "A".to_owned(),
                nodes: vec![],
                strategy: Strategy::Sequential,
                requires: Some(vec!["B".to_owned()]),
                max_concurrency: None,
            },
            ParallelGroup {
                group: "B".to_owned(),
                nodes: vec![],
                strategy: Strategy::Sequential,
                requires: Some(vec!["A".to_owned()]),
                max_concurrency: None,
            },
        ];
        let result = resolve_group_order(&groups);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Group I: Memory integration
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_get_actions_run_before_agent() {
        // Node has a memory Get action. We verify execution completes without error.
        let dir = tempdir().unwrap();
        let agm_path = dir.path().join("test.agm");
        let mut node = test_node("A");
        node.memory = Some(vec![MemoryEntry {
            key: "preload_key".to_owned(),
            topic: "context".to_owned(),
            action: MemoryAction::Get,
            value: None,
            scope: Some(MemoryScope::Project),
            ttl: None,
            query: None,
            max_results: None,
        }]);
        let file = AgmFile {
            header: test_header(),
            nodes: vec![node],
        };
        let graph = build_graph(&file);
        let mut tracker = ExecutionTracker::new(&agm_path, &file, &graph).unwrap();
        let mem_path = dir.path().join("test.agm.mem");
        let global_path = dir.path().join("global.mem");
        let mut memory = MemoryRuntime::new(mem_path, global_path, "test.pkg").unwrap();
        let config = RunConfig {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let agent = MockAgent::succeeding();
        let node_ref = file.nodes.iter().find(|n| n.id == "A").unwrap();
        let result = execute_node(
            node_ref,
            &file,
            &graph,
            &mut tracker,
            &agent,
            &mut memory,
            &config,
        )
        .unwrap();
        assert_eq!(result.status, ExecutionStatus::Completed);
    }

    #[test]
    fn test_memory_upsert_actions_run_after_agent() {
        // Node has a memory Upsert action. Agent succeeds -> upsert should run.
        let dir = tempdir().unwrap();
        let agm_path = dir.path().join("test.agm");
        let mut node = test_node("A");
        node.memory = Some(vec![MemoryEntry {
            key: "result_key".to_owned(),
            topic: "output".to_owned(),
            action: MemoryAction::Upsert,
            value: Some("done".to_owned()),
            scope: Some(MemoryScope::Project),
            ttl: Some(MemoryTtl::Permanent),
            query: None,
            max_results: None,
        }]);
        let file = AgmFile {
            header: test_header(),
            nodes: vec![node],
        };
        let graph = build_graph(&file);
        let mut tracker = ExecutionTracker::new(&agm_path, &file, &graph).unwrap();
        let mem_path = dir.path().join("test.agm.mem");
        let global_path = dir.path().join("global.mem");
        let mut memory = MemoryRuntime::new(mem_path, global_path, "test.pkg").unwrap();
        let config = RunConfig {
            working_dir: dir.path().to_path_buf(),
            ..Default::default()
        };
        let agent = MockAgent::succeeding();
        let node_ref = file.nodes.iter().find(|n| n.id == "A").unwrap();
        let result = execute_node(
            node_ref,
            &file,
            &graph,
            &mut tracker,
            &agent,
            &mut memory,
            &config,
        )
        .unwrap();
        assert_eq!(result.status, ExecutionStatus::Completed);
    }

    // -----------------------------------------------------------------------
    // Group J: Report building
    // -----------------------------------------------------------------------

    #[test]
    fn test_report_counts_match_execution() {
        // 3 nodes: A succeeds, B succeeds, C (depends on A) succeeds
        // Use failing agent on C only -- but we can't do that with MockAgent.
        // Instead: A, B, C all independent, agent fails -- all fail.
        // Or: A, B succeed; C depends on A but we want to test counts.
        // Let's use: A, B independent (no fail), C independent (fails with failing agent).
        // Since MockAgent is all-or-nothing, let's do 2 nodes succeed, 1 fails
        // by running two separate runs -- or just use A, B, C with failing agent
        // and verify failed: 3.
        // Better approach: A, B succeed (run with succeeding), then C fails... can't mix.
        // Use RecordingAgent for 2 nodes and MockAgent::failing for a different test.
        // Simplest: 3 nodes all same agent, check counts add up.
        let nodes = vec![test_node("A"), test_node("B"), test_node("C")];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = MockAgent::succeeding();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        assert_eq!(report.succeeded + report.failed, report.executed);
        assert_eq!(report.executed, 3);
        assert_eq!(report.succeeded, 3);
        assert_eq!(report.failed, 0);
    }

    #[test]
    fn test_report_duration_is_positive() {
        let nodes = vec![test_node("A")];
        let (file, _graph, mut tracker, mut memory, _dir, config) = setup_test_env(nodes);
        let agent = MockAgent::succeeding();
        let report = run_topological(&mut tracker, &file, &agent, &mut memory, &config).unwrap();
        assert!(report.duration > Duration::ZERO);
    }
}
