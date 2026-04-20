//! `fix` umbrella command: runs `repair` then `normalize` in sequence.
//!
//! Exit codes:
//!   0 — both stages succeeded (or `--check` confirmed no rewrites needed).
//!   1 — `--check` detected pending rewrites in either stage.
//!   2 — parse-after-repair failure (safety net) or normalize parse error, or unknown rule ID.
//!   3 — file I/O error or `--confirm` without TTY.

use std::io::IsTerminal as _;
use std::path::Path;

use agm_core::normalize::rules::{RuleParseError, RuleSet};
use agm_core::normalize::{NormalizeConfig, normalize_text};

use super::normalize::ReportFormat as NormReportFormat;
use super::repair::{
    EXIT_IO_OR_CONFIRM, EXIT_OK, EXIT_PARSE_OR_UNKNOWN_RULE, EXIT_REWRITES_NEEDED,
    ReportFormat as RepairReportFormat, render_report as render_repair_report, validate_rule_ids,
};

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Entry point called from `main.rs` for the `fix` subcommand.
#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    output: Option<&Path>,
    in_place: bool,
    confirm: bool,
    explain: bool,
    report_format: RepairReportFormat,
    disable_rules: &[String],
    enable_only: Option<&str>,
    no_safety_net: bool,
    check: bool,
    // Normalize-specific flags
    norm_rules_path: Option<&Path>,
    no_types: bool,
    no_fields: bool,
) -> i32 {
    // ---- Validate rule IDs ----
    let (disabled_ids, only_ids) = match validate_rule_ids(disable_rules, enable_only) {
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

    // ---- Build repair config ----
    let repair_config = agm_core::repair::RepairConfig {
        disabled_rules: disabled_ids,
        only_rules: only_ids,
        safety_net: !no_safety_net,
    };

    // ---- Run repair stage ----
    let repair_result = agm_core::repair::repair_text(&source, &repair_config);

    // ---- Build normalize rule set ----
    let mut norm_rules = RuleSet::builtin();
    if let Some(rules_path) = norm_rules_path {
        match load_rules_file(rules_path) {
            Ok(extra) => norm_rules.merge(extra),
            Err(e) => {
                eprintln!("error: {e}");
                return EXIT_PARSE_OR_UNKNOWN_RULE;
            }
        }
    }

    let norm_config = NormalizeConfig {
        rules: norm_rules,
        warn_on_collision: true,
        normalize_types: !no_types,
        normalize_fields: !no_fields,
    };

    // ---- Run normalize stage ----
    // Per D25: if repair rolled back, do not run normalize stage.
    let (final_text, norm_report) = if repair_result.rolled_back {
        eprintln!(
            "error: repair stage failed to parse result; original preserved. Normalize stage skipped."
        );
        eprintln!(
            "  hint: run `agm repair {} --no-safety-net --explain` to debug.",
            file.display()
        );
        if explain {
            render_repair_report(&repair_result.report, report_format);
        }
        return EXIT_PARSE_OR_UNKNOWN_RULE;
    } else {
        match normalize_text(&repair_result.text, &norm_config) {
            Ok(pair) => pair,
            Err(errors) => {
                for e in &errors {
                    eprintln!("normalize parse error: {}", e.message);
                }
                return EXIT_PARSE_OR_UNKNOWN_RULE;
            }
        }
    };

    // ---- --check mode ----
    if check {
        let repair_has_rewrites = !repair_result.report.is_empty();
        let norm_has_rewrites = !norm_report.rewrites.is_empty();
        if repair_has_rewrites || norm_has_rewrites {
            return EXIT_REWRITES_NEEDED;
        } else {
            return EXIT_OK;
        }
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
        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() || !line.trim().eq_ignore_ascii_case("y")
        {
            eprintln!("Aborted.");
            return EXIT_OK;
        }
    }

    // ---- Write output ----
    if in_place {
        if let Err(e) = std::fs::write(file, &final_text) {
            eprintln!("error: cannot write {}: {e}", file.display());
            return EXIT_IO_OR_CONFIRM;
        }
    } else if let Some(out) = output {
        if let Err(e) = std::fs::write(out, &final_text) {
            eprintln!("error: cannot write {}: {e}", out.display());
            return EXIT_IO_OR_CONFIRM;
        }
    } else {
        print!("{final_text}");
    }

    // ---- --explain ----
    if explain {
        let repair_empty = repair_result.report.is_empty();
        let norm_empty = norm_report.rewrites.is_empty();

        match report_format {
            RepairReportFormat::Text => {
                eprintln!("\nRepair stage:");
                if repair_empty {
                    eprintln!("  0 rewrites");
                } else {
                    render_repair_report(&repair_result.report, RepairReportFormat::Text);
                }
                eprintln!("\nNormalize stage:");
                if norm_empty {
                    eprintln!("  0 rewrites");
                } else {
                    render_normalize_report(&norm_report, NormReportFormat::Text);
                }
            }
            RepairReportFormat::Json => {
                let combined = serde_json::json!({
                    "repair": repair_result.report,
                    "normalize": norm_report,
                });
                match serde_json::to_string_pretty(&combined) {
                    Ok(json) => eprintln!("{json}"),
                    Err(e) => eprintln!("error: failed to serialize combined report: {e}"),
                }
            }
        }
    }

    EXIT_OK
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_rules_file(path: &Path) -> Result<RuleSet, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read rules file {}: {e}", path.display()))?;
    RuleSet::from_yaml_str(&text).map_err(|e| match e {
        RuleParseError::Yaml(msg) => {
            format!("invalid YAML in rules file {}: {msg}", path.display())
        }
        RuleParseError::Empty => {
            format!("rules file {} is empty", path.display())
        }
    })
}

fn render_normalize_report(
    report: &agm_core::normalize::NormalizeReport,
    format: NormReportFormat,
) {
    match format {
        NormReportFormat::Text => {
            eprintln!("  {}", report.summary());
            for r in &report.rewrites {
                let node = r.node_id.as_deref().unwrap_or("<unknown>");
                eprintln!(
                    "  - [{}] node {} (line {})",
                    r.rule_id, node, r.span.start_line
                );
                eprintln!("      - {}", r.before);
                eprintln!("      + {}", r.after);
            }
        }
        NormReportFormat::Json => {
            if let Ok(json) = serde_json::to_string_pretty(report) {
                eprintln!("{json}");
            }
        }
    }
}
