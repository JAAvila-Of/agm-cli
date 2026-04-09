//! `status` command: display execution state of an AGM file.

use std::path::Path;

use crate::runtime::state;

use super::helpers;

/// Displays execution status.
///
/// Exit codes:
/// - 0: all completed or skipped
/// - 1: any failed or blocked
/// - 2: in progress (some pending/ready/in_progress) or no state found
pub fn run(file: &Path, json: bool, node: Option<&str>) -> i32 {
    let parsed = helpers::parse_and_build_graph(file);

    let state_file = match state::load_state(file) {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("No execution state found for {}", file.display());
            return helpers::EXIT_IN_PROGRESS;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return helpers::EXIT_IN_PROGRESS;
        }
    };

    if let Some(node_id) = node {
        let ns = match state_file.nodes.get(node_id) {
            Some(s) => s,
            None => {
                eprintln!("error: node '{}' not found in state", node_id);
                return helpers::EXIT_VALIDATION_ERROR;
            }
        };
        if json {
            println!("{}", serde_json::to_string_pretty(ns).unwrap());
        } else {
            println!("Node: {}", node_id);
            println!("Status: {}", ns.execution_status);
            println!("Executed by: {}", ns.executed_by.as_deref().unwrap_or("--"));
            println!("Executed at: {}", ns.executed_at.as_deref().unwrap_or("--"));
            println!("Retry count: {}", ns.retry_count);
        }
        return helpers::EXIT_SUCCESS;
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&state_file).unwrap());
        return helpers::EXIT_SUCCESS;
    }

    // Build tracker to compute progress
    let graph = agm_core::graph::build_graph(&parsed.file);
    let tracker = match crate::runtime::state::ExecutionTracker::new(file, &parsed.file, &graph) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return helpers::EXIT_IO_ERROR;
        }
    };

    let p = tracker.progress();
    let total = p.total;
    let completed = p.completed;
    let pct = if total == 0 {
        0
    } else {
        completed * 100 / total
    };

    println!("Status: {}/{} completed ({}%)", completed, total, pct);
    println!();
    println!("{:<25} {:<14} {:<14} Time", "Node", "Status", "Agent");
    println!("{:<25} {:<14} {:<14} ----", "----", "------", "-----");

    for node in &parsed.file.nodes {
        let ns = match state_file.nodes.get(&node.id) {
            Some(s) => s,
            None => continue,
        };
        let agent = ns.executed_by.as_deref().unwrap_or("--");
        let time = "--";
        println!(
            "{:<25} {:<14} {:<14} {}",
            node.id,
            ns.execution_status.to_string(),
            agent,
            time
        );
    }

    // Compute exit code
    if p.failed > 0 || p.blocked > 0 {
        1
    } else if p.completed + p.skipped == p.total {
        0
    } else {
        helpers::EXIT_IN_PROGRESS
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_exit_code_all_completed() {
        // progress: completed=2, skipped=0, failed=0, blocked=0, total=2
        let (completed, skipped, failed, blocked, total) = (2, 0, 0, 0, 2usize);
        let code = if failed > 0 || blocked > 0 {
            1
        } else if completed + skipped == total {
            0
        } else {
            2
        };
        assert_eq!(code, 0);
    }

    #[test]
    fn test_exit_code_any_failed() {
        let (completed, skipped, failed, blocked, total) = (1, 0, 1, 0, 2usize);
        let code = if failed > 0 || blocked > 0 {
            1
        } else if completed + skipped == total {
            0
        } else {
            2
        };
        assert_eq!(code, 1);
    }

    #[test]
    fn test_exit_code_in_progress() {
        let (completed, skipped, failed, blocked, total) = (1, 0, 0, 0, 3usize);
        let code = if failed > 0 || blocked > 0 {
            1
        } else if completed + skipped == total {
            0
        } else {
            2
        };
        assert_eq!(code, 2);
    }
}
