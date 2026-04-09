//! `run` command: parse, validate, build graph, execute nodes via scheduler.

use std::path::Path;
use std::time::Duration;

use agm_core::model::fields::NodeType;

use agm_core::model::execution::ExecutionStatus;

use crate::runtime::agent::ShellAgent;
use crate::runtime::scheduler::{self, RunConfig, RunReport};

use super::helpers;

/// Runs the AGM orchestration pipeline.
///
/// Returns exit code: 0 = all succeeded, 1 = any failed/blocked.
#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    node: Option<&str>,
    group: Option<&str>,
    dry_run: bool,
    concurrency: usize,
    timeout_secs: u64,
    working_dir: &Path,
    fail_fast: bool,
) -> i32 {
    let mut ctx = helpers::build_runtime_context(file);

    let config = RunConfig {
        max_concurrency: concurrency,
        timeout: Duration::from_secs(timeout_secs),
        dry_run,
        target_nodes: node.map(|n| vec![n.to_owned()]),
        target_group: group.map(str::to_owned),
        working_dir: working_dir.to_owned(),
        fail_fast,
    };

    let agent = ShellAgent;

    // Determine if there is an orchestration node and a target group was specified
    let orch_node = ctx
        .parsed
        .file
        .nodes
        .iter()
        .find(|n| n.node_type == NodeType::Orchestration);

    let result = if config.target_group.is_some() {
        match orch_node {
            Some(orch) => scheduler::run_orchestrated(
                &mut ctx.tracker,
                &ctx.parsed.file,
                &agent,
                &mut ctx.memory,
                orch,
                &config,
            ),
            None => {
                eprintln!("error: --group requires an orchestration node in the file");
                return 1;
            }
        }
    } else {
        scheduler::run_topological(
            &mut ctx.tracker,
            &ctx.parsed.file,
            &agent,
            &mut ctx.memory,
            &config,
        )
    };

    match result {
        Ok(report) => {
            if dry_run {
                print_dry_run(&report);
            } else {
                print_report(&report);
            }
            if report.failed == 0 && report.blocked == 0 {
                0
            } else {
                1
            }
        }
        Err(e) => {
            eprintln!("error: run failed: {e}");
            1
        }
    }
}

fn print_report(report: &RunReport) {
    println!(
        "Run complete: {} executed, {} succeeded, {} failed, {} skipped, {} blocked ({:.1}s)",
        report.executed,
        report.succeeded,
        report.failed,
        report.skipped,
        report.blocked,
        report.duration.as_secs_f64(),
    );
    for nr in &report.node_results {
        let icon = match nr.status {
            ExecutionStatus::Completed => "[ok]",
            ExecutionStatus::Failed => "[FAIL]",
            _ => "[--]",
        };
        println!(
            "  {} {} ({:.1}s, {})",
            icon,
            nr.node_id,
            nr.duration.as_secs_f64(),
            nr.agent
        );
    }
}

fn print_dry_run(report: &RunReport) {
    println!(
        "Dry run: would execute {} nodes in order:",
        report.total_nodes
    );
    for (i, nr) in report.node_results.iter().enumerate() {
        println!("  {}. {}", i + 1, nr.node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::scheduler::{NodeResult, RunReport};
    use agm_core::model::execution::ExecutionStatus;
    use std::time::Duration;

    fn make_report(executed: usize, succeeded: usize, failed: usize) -> RunReport {
        RunReport {
            total_nodes: executed,
            executed,
            succeeded,
            failed,
            skipped: 0,
            blocked: 0,
            duration: Duration::from_millis(2300),
            node_results: vec![],
        }
    }

    #[test]
    fn test_print_report_format() {
        let report = make_report(3, 2, 1);
        // Just ensure it doesn't panic -- output goes to stdout
        print_report(&report);
    }

    #[test]
    fn test_print_dry_run_format() {
        let report = RunReport {
            total_nodes: 2,
            executed: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            blocked: 0,
            duration: Duration::ZERO,
            node_results: vec![
                NodeResult {
                    node_id: "setup".to_owned(),
                    status: ExecutionStatus::Pending,
                    agent: "shell".to_owned(),
                    duration: Duration::ZERO,
                    output: None,
                },
                NodeResult {
                    node_id: "build".to_owned(),
                    status: ExecutionStatus::Pending,
                    agent: "shell".to_owned(),
                    duration: Duration::ZERO,
                    output: None,
                },
            ],
        };
        print_dry_run(&report);
    }

    #[test]
    fn test_exit_code_success() {
        let report = make_report(2, 2, 0);
        let code = if report.failed == 0 && report.blocked == 0 {
            0
        } else {
            1
        };
        assert_eq!(code, 0);
    }

    #[test]
    fn test_exit_code_failure() {
        let report = make_report(2, 1, 1);
        let code = if report.failed == 0 && report.blocked == 0 {
            0
        } else {
            1
        };
        assert_eq!(code, 1);
    }
}
