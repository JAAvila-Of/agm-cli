//! `verify` command: run verify checks on nodes.

use std::path::Path;
use std::time::Duration;

use crate::runtime::verifier::{self, FailStrategy, VerifyResult};

use super::helpers;

/// Runs verify checks on node(s).
///
/// Exit codes: 0 = all pass, 1 = any fail, 2 = no verify checks found.
pub fn run(
    file: &Path,
    node: Option<&str>,
    all: bool,
    json: bool,
    working_dir: &Path,
    timeout_secs: u64,
) -> i32 {
    let ctx = helpers::build_runtime_context(file);

    let target_nodes: Vec<&agm_core::model::node::Node> = if let Some(id) = node {
        vec![helpers::find_node_or_exit(&ctx.parsed.file, id)]
    } else if all {
        ctx.parsed
            .file
            .nodes
            .iter()
            .filter(|n| n.verify.is_some())
            .collect()
    } else {
        eprintln!("error: specify --node <id> or --all");
        return helpers::EXIT_VALIDATION_ERROR;
    };

    if target_nodes.is_empty() {
        eprintln!("No verify checks found.");
        return 2;
    }

    let timeout = Duration::from_secs(timeout_secs);
    let mut any_failed = false;
    let mut results: Vec<VerifyResult> = Vec::new();

    for node in &target_nodes {
        if node.verify.is_none() {
            continue;
        }
        let result = verifier::verify_node(
            node,
            &ctx.tracker,
            working_dir,
            timeout,
            FailStrategy::RunAll,
        );
        if !result.all_passed {
            any_failed = true;
        }
        results.push(result);
    }

    if results.is_empty() {
        eprintln!("No verify checks found.");
        return 2;
    }

    if json {
        let json_results: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "node_id": r.node_id,
                    "all_passed": r.all_passed,
                    "checks": r.checks.iter().map(|c| {
                        serde_json::json!({
                            "passed": c.passed,
                            "message": c.message,
                            "duration_ms": c.duration.as_millis(),
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_results).unwrap());
    } else {
        for result in &results {
            println!("{}", verifier::format_verify_log(result));
        }
    }

    if any_failed { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use crate::runtime::verifier::VerifyResult;

    fn make_result(all_passed: bool) -> VerifyResult {
        VerifyResult {
            node_id: "setup".to_owned(),
            checks: vec![],
            all_passed,
        }
    }

    #[test]
    fn test_exit_code_all_pass() {
        let results = vec![make_result(true)];
        let any_failed = results.iter().any(|r| !r.all_passed);
        assert_eq!(if any_failed { 1 } else { 0 }, 0);
    }

    #[test]
    fn test_exit_code_any_fail() {
        let results = vec![make_result(false)];
        let any_failed = results.iter().any(|r| !r.all_passed);
        assert_eq!(if any_failed { 1 } else { 0 }, 1);
    }

    #[test]
    fn test_exit_code_no_checks() {
        let target_nodes: Vec<&agm_core::model::node::Node> = vec![];
        let code = if target_nodes.is_empty() { 2 } else { 0 };
        assert_eq!(code, 2);
    }
}
