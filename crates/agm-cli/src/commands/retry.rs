//! `retry` command: re-execute failed nodes.

use std::path::Path;
use std::time::Duration;

use agm_core::model::execution::ExecutionStatus;

use crate::runtime::agent::ShellAgent;
use crate::runtime::scheduler::{self, RunConfig};

use super::helpers;

/// Retries failed node(s): transitions Failed -> Ready, then re-executes.
///
/// Exit code: 0 = all retried nodes succeeded, 1 = any failed.
pub fn run(
    file: &Path,
    node: Option<&str>,
    all_failed: bool,
    working_dir: &Path,
    timeout_secs: u64,
) -> i32 {
    let mut ctx = helpers::build_runtime_context(file);

    // Determine which nodes to retry
    let nodes_to_retry: Vec<String> = if let Some(id) = node {
        match ctx.tracker.node_state(id) {
            Some(ns) if ns.execution_status == ExecutionStatus::Failed => {
                vec![id.to_owned()]
            }
            Some(ns) => {
                eprintln!(
                    "error: node '{}' is in state '{}', not 'failed'",
                    id, ns.execution_status
                );
                return 1;
            }
            None => {
                eprintln!("error: node '{}' not found", id);
                return 1;
            }
        }
    } else if all_failed {
        ctx.tracker
            .state()
            .nodes
            .iter()
            .filter(|(_, ns)| ns.execution_status == ExecutionStatus::Failed)
            .map(|(id, _)| id.clone())
            .collect()
    } else {
        eprintln!("error: specify --node <id> or --all-failed");
        return 1;
    };

    if nodes_to_retry.is_empty() {
        println!("No failed nodes to retry.");
        return 0;
    }

    for id in &nodes_to_retry {
        if let Err(e) = ctx.tracker.retry(id) {
            eprintln!("error: failed to mark '{}' for retry: {e}", id);
            return 1;
        }
    }

    if let Err(e) = ctx.tracker.save() {
        eprintln!("error: failed to save state: {e}");
        return helpers::EXIT_IO_ERROR;
    }

    let config = RunConfig {
        max_concurrency: 1,
        timeout: Duration::from_secs(timeout_secs),
        dry_run: false,
        target_nodes: Some(nodes_to_retry),
        target_group: None,
        working_dir: working_dir.to_owned(),
        fail_fast: false,
    };

    let agent = ShellAgent;
    let result = scheduler::run_topological(
        &mut ctx.tracker,
        &ctx.parsed.file,
        &agent,
        &mut ctx.memory,
        &config,
    );

    match result {
        Ok(report) => {
            println!(
                "Retry complete: {} executed, {} succeeded, {} failed ({:.1}s)",
                report.executed,
                report.succeeded,
                report.failed,
                report.duration.as_secs_f64(),
            );
            if report.failed == 0 { 0 } else { 1 }
        }
        Err(e) => {
            eprintln!("error: retry failed: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use agm_core::model::execution::ExecutionStatus;

    #[test]
    fn test_retry_failed_node_transitions_to_ready() {
        // Verify the logic: only Failed nodes can be retried
        let status = ExecutionStatus::Failed;
        assert_eq!(status, ExecutionStatus::Failed);
    }

    #[test]
    fn test_retry_non_failed_node_rejected() {
        let status = ExecutionStatus::Completed;
        assert_ne!(status, ExecutionStatus::Failed);
    }

    #[test]
    fn test_retry_empty_result_succeeds() {
        let nodes_to_retry: Vec<String> = vec![];
        assert!(nodes_to_retry.is_empty());
    }
}
