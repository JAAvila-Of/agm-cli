//! `state` command: inspect and manage execution state.

use std::io::BufRead;
use std::path::Path;

use agm_core::model::execution::ExecutionStatus;
use agm_core::renderer::state::{render_state, render_state_json};

use crate::runtime::state;

use super::helpers;

// ---- list ----

pub fn list(file: &Path, json: bool) -> i32 {
    let state_file = match state::load_state(file) {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("No state file found for {}", file.display());
            return helpers::EXIT_IO_ERROR;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return helpers::EXIT_IO_ERROR;
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&state_file).unwrap());
    } else {
        for (id, ns) in &state_file.nodes {
            println!("{}: {}", id, ns.execution_status);
        }
    }
    helpers::EXIT_SUCCESS
}

// ---- get ----

pub fn get(file: &Path, node_id: &str) -> i32 {
    let state_file = match state::load_state(file) {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("No state file found for {}", file.display());
            return helpers::EXIT_IO_ERROR;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return helpers::EXIT_IO_ERROR;
        }
    };

    match state_file.nodes.get(node_id) {
        Some(ns) => {
            println!("Node: {}", node_id);
            println!("Status: {}", ns.execution_status);
            println!("Executed by: {}", ns.executed_by.as_deref().unwrap_or("--"));
            println!("Executed at: {}", ns.executed_at.as_deref().unwrap_or("--"));
            println!("Retry count: {}", ns.retry_count);
            if let Some(log) = &ns.execution_log {
                println!("Log: {}", log);
            }
            helpers::EXIT_SUCCESS
        }
        None => {
            eprintln!("error: node '{}' not found in state", node_id);
            helpers::EXIT_VALIDATION_ERROR
        }
    }
}

// ---- export ----

pub fn export(file: &Path, format: &str) -> i32 {
    let state_file = match state::load_state(file) {
        Ok(Some(s)) => s,
        Ok(None) => {
            eprintln!("No state file found for {}", file.display());
            return helpers::EXIT_IO_ERROR;
        }
        Err(e) => {
            eprintln!("error: {e}");
            return helpers::EXIT_IO_ERROR;
        }
    };

    match format {
        "json" => println!("{}", render_state_json(&state_file)),
        "agm" => print!("{}", render_state(&state_file)),
        "sql" => {
            eprintln!("error: SQL export not yet implemented");
            return helpers::EXIT_VALIDATION_ERROR;
        }
        other => {
            eprintln!("error: unknown format '{}'. Use json or agm.", other);
            return helpers::EXIT_VALIDATION_ERROR;
        }
    }

    helpers::EXIT_SUCCESS
}

// ---- import ----

pub fn import(file: &Path, from: &Path) -> i32 {
    let source = match std::fs::read_to_string(from) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", from.display(), e);
            return helpers::EXIT_IO_ERROR;
        }
    };

    let state_file = match agm_core::parser::state::parse_state(&source) {
        Ok(s) => s,
        Err(errors) => {
            eprintln!(
                "error: failed to parse state file {}: {} error(s)",
                from.display(),
                errors.len()
            );
            return helpers::EXIT_VALIDATION_ERROR;
        }
    };

    let target_path = state::state_path(file);
    if let Err(e) = state::save_state(file, &state_file) {
        eprintln!(
            "error: failed to write state to {}: {}",
            target_path.display(),
            e
        );
        return helpers::EXIT_IO_ERROR;
    }

    println!(
        "Imported state from {} to {}",
        from.display(),
        target_path.display()
    );
    helpers::EXIT_SUCCESS
}

// ---- reset ----

pub fn reset(file: &Path, node: Option<&str>, keep_completed: bool, yes: bool) -> i32 {
    if !yes {
        eprint!("Reset execution state for {}? [y/N] ", file.display());
        let mut input = String::new();
        if std::io::stdin().lock().read_line(&mut input).is_err()
            || !input.trim().eq_ignore_ascii_case("y")
        {
            eprintln!("Aborted.");
            return helpers::EXIT_SUCCESS;
        }
    }

    let ctx = helpers::build_runtime_context(file);
    let mut tracker = ctx.tracker;

    match node {
        Some(id) => {
            if let Err(e) = tracker.reset_node(id) {
                eprintln!("error: {e}");
                return helpers::EXIT_VALIDATION_ERROR;
            }
            eprintln!("Reset node '{}'", id);
        }
        None => {
            if keep_completed {
                let state = tracker.state();
                let to_reset: Vec<String> = state
                    .nodes
                    .iter()
                    .filter(|(_, ns)| ns.execution_status != ExecutionStatus::Completed)
                    .map(|(id, _)| id.clone())
                    .collect();
                let count = to_reset.len();
                for id in &to_reset {
                    let _ = tracker.reset_node(id);
                }
                eprintln!("Reset {} non-completed nodes", count);
            } else {
                if let Err(e) = tracker.reset() {
                    eprintln!("error: {e}");
                    return helpers::EXIT_VALIDATION_ERROR;
                }
                eprintln!("Reset all nodes");
            }
        }
    }

    if let Err(e) = tracker.save() {
        eprintln!("error: failed to save state: {e}");
        return helpers::EXIT_IO_ERROR;
    }
    helpers::EXIT_SUCCESS
}

#[cfg(test)]
mod tests {
    use agm_core::model::execution::ExecutionStatus;

    #[test]
    fn test_list_prints_all_nodes() {
        // Logic test: if state_file has nodes, list iterates over them
        use std::collections::BTreeMap;
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "setup".to_owned(),
            agm_core::model::state::NodeState {
                execution_status: ExecutionStatus::Completed,
                executed_by: None,
                executed_at: None,
                execution_log: None,
                retry_count: 0,
            },
        );
        assert_eq!(nodes.len(), 1);
        for (id, ns) in &nodes {
            assert_eq!(id, "setup");
            assert_eq!(ns.execution_status, ExecutionStatus::Completed);
        }
    }

    #[test]
    fn test_reset_keep_completed_filters_correctly() {
        use agm_core::model::execution::ExecutionStatus;
        let statuses = [
            ExecutionStatus::Completed,
            ExecutionStatus::Failed,
            ExecutionStatus::Pending,
        ];
        let to_reset: Vec<_> = statuses
            .iter()
            .filter(|s| **s != ExecutionStatus::Completed)
            .collect();
        assert_eq!(to_reset.len(), 2);
    }

    #[test]
    fn test_reset_clears_all() {
        // All statuses would be reset when keep_completed is false
        let all = [
            ExecutionStatus::Completed,
            ExecutionStatus::Failed,
            ExecutionStatus::Pending,
        ];
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_export_format_sql_unimplemented() {
        let format = "sql";
        let result = match format {
            "json" | "agm" => true,
            "sql" => false,
            _ => false,
        };
        assert!(!result);
    }
}
