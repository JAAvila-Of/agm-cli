//! `repair` command: apply text-level rewrite rules to an AGM file.
//!
//! Exit codes:
//!   0 — repair succeeded (or `--check` confirmed no rewrites needed).
//!   1 — `--check` detected pending rewrites.
//!   2 — parse-after-repair failure (safety net activated) OR unknown rule ID.
//!   3 — file I/O error OR `--confirm` used without an interactive terminal.

use std::io::IsTerminal as _;
use std::path::Path;

use agm_core::repair::RepairReport;
use agm_core::repair::{RepairConfig, builtin_rule_ids, repair_text};

// ---------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------

pub(crate) const EXIT_OK: i32 = 0;
pub(crate) const EXIT_REWRITES_NEEDED: i32 = 1;
pub(crate) const EXIT_PARSE_OR_UNKNOWN_RULE: i32 = 2;
pub(crate) const EXIT_IO_OR_CONFIRM: i32 = 3;

// ---------------------------------------------------------------------------
// ReportFormat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Json,
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Entry point called from `main.rs` for the `repair` subcommand.
///
/// # Returns
///
/// The process exit code (0–3).
#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    output: Option<&Path>,
    in_place: bool,
    confirm: bool,
    explain: bool,
    report_format: ReportFormat,
    disable_rules: &[String],
    enable_only: Option<&str>,
    no_safety_net: bool,
    check: bool,
) -> i32 {
    // ---- Validate rule IDs ----
    let (disabled_ids, only_ids) = match resolve_rule_ids(disable_rules, enable_only) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: {e}");
            return EXIT_PARSE_OR_UNKNOWN_RULE;
        }
    };

    // ---- Read input ----
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {e}", file.display());
            return EXIT_IO_OR_CONFIRM;
        }
    };

    // ---- Build config ----
    let config = RepairConfig {
        disabled_rules: disabled_ids,
        only_rules: only_ids,
        safety_net: !no_safety_net,
    };

    // ---- Repair ----
    let result = repair_text(&source, &config);

    // ---- --check mode ----
    if check {
        if result.report.is_empty() {
            return EXIT_OK;
        } else {
            return EXIT_REWRITES_NEEDED;
        }
    }

    // ---- Safety net rollback ----
    if result.rolled_back {
        eprintln!(
            "error: repair applied rules but the result still fails to parse; original preserved."
        );
        eprintln!(
            "  hint: run `agm repair {} --no-safety-net --explain` to see the attempted rewrite.",
            file.display()
        );
        if explain {
            render_report(&result.report, report_format);
        }
        return EXIT_PARSE_OR_UNKNOWN_RULE;
    }

    // ---- --confirm prompt ----
    if in_place && confirm {
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "error: --confirm requires an interactive terminal; omit --confirm for unattended runs"
            );
            return EXIT_IO_OR_CONFIRM;
        }
        eprintln!(
            "About to rewrite {} in place. Proceed? [y/N] ",
            file.display()
        );
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err()
            || !input.trim().eq_ignore_ascii_case("y")
        {
            eprintln!("Aborted.");
            return EXIT_OK;
        }
    }

    // ---- Write output ----
    if in_place {
        if let Err(e) = std::fs::write(file, &result.text) {
            eprintln!("error: cannot write {}: {e}", file.display());
            return EXIT_IO_OR_CONFIRM;
        }
    } else if let Some(out) = output {
        if let Err(e) = std::fs::write(out, &result.text) {
            eprintln!("error: cannot write {}: {e}", out.display());
            return EXIT_IO_OR_CONFIRM;
        }
    } else {
        print!("{}", result.text);
    }

    // ---- --explain ----
    if explain {
        render_report(&result.report, report_format);
    }

    EXIT_OK
}

// ---------------------------------------------------------------------------
// Rule ID resolution
// ---------------------------------------------------------------------------

/// Validate and resolve rule IDs from CLI args.
///
/// Returns `(disabled_ids, only_ids)`.
fn resolve_rule_ids(
    disable_rules: &[String],
    enable_only: Option<&str>,
) -> Result<(Vec<&'static str>, Option<Vec<&'static str>>), String> {
    let valid_ids: Vec<&'static str> = builtin_rule_ids();

    let valid_list = valid_ids.join(", ");

    // Validate disable_rules
    let mut disabled: Vec<&'static str> = Vec::new();
    for id in disable_rules {
        match valid_ids.iter().find(|&&v| v == id.as_str()) {
            Some(&static_id) => disabled.push(static_id),
            None => {
                return Err(format!("unknown rule id '{id}' (valid: {valid_list})"));
            }
        }
    }

    // Validate enable_only
    let only = if let Some(only_str) = enable_only {
        let mut ids: Vec<&'static str> = Vec::new();
        for id in only_str.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match valid_ids.iter().find(|&&v| v == id) {
                Some(&static_id) => ids.push(static_id),
                None => {
                    return Err(format!("unknown rule id '{id}' (valid: {valid_list})"));
                }
            }
        }
        Some(ids)
    } else {
        None
    };

    Ok((disabled, only))
}

// ---------------------------------------------------------------------------
// Report rendering
// ---------------------------------------------------------------------------

/// Render the repair report to stderr.
pub(crate) fn render_report(report: &RepairReport, format: ReportFormat) {
    match format {
        ReportFormat::Text => render_report_text(report),
        ReportFormat::Json => match serde_json::to_string_pretty(report) {
            Ok(json) => eprintln!("{json}"),
            Err(e) => eprintln!("error: failed to serialize repair report: {e}"),
        },
    }
}

fn render_report_text(report: &RepairReport) {
    eprintln!("\nRepair report:");
    eprintln!("  {}", report.summary());
    if report.rolled_back {
        eprintln!("  (rolled back — original preserved)");
    }

    // Group records by rule ID, keeping insertion order.
    let mut order: Vec<String> = Vec::new();
    let mut by_rule: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();

    for rec in &report.rewrites {
        if !by_rule.contains_key(&rec.rule_id) {
            order.push(rec.rule_id.clone());
        }
        by_rule
            .entry(rec.rule_id.clone())
            .or_default()
            .push(rec.line);
    }

    for rule_id in &order {
        let lines = by_rule.get(rule_id).unwrap();
        let count = lines.len();
        // Deduplicate and sort lines for display.
        let mut unique_lines = lines.clone();
        unique_lines.sort_unstable();
        unique_lines.dedup();

        let lines_str = if rule_id == "R-CRLF" {
            "(whole file)".to_owned()
        } else {
            let line_list: Vec<String> = unique_lines.iter().map(|n| n.to_string()).collect();
            format!("lines {}", line_list.join(", "))
        };

        eprintln!("  {:<22} \u{00d7}{count} ({lines_str})", rule_id);
    }
}

/// Validate and return rule IDs — public for fix.rs to reuse.
pub(crate) fn validate_rule_ids(
    disable_rules: &[String],
    enable_only: Option<&str>,
) -> Result<(Vec<&'static str>, Option<Vec<&'static str>>), String> {
    resolve_rule_ids(disable_rules, enable_only)
}
