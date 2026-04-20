//! `normalize` command: rewrite non-canonical field and type names in an AGM file.
//!
//! Exit codes:
//!   0 — normalized successfully (or `--check` confirmed input was canonical).
//!   1 — `--check` detected at least one rewrite would be made (non-canonical input).
//!   2 — parse errors in input or I/O error.
//!   3 — rule file error.

use std::path::Path;

use agm_core::normalize::rules::{RuleParseError, RuleSet};
use agm_core::normalize::{NormalizeConfig, NormalizeReport, normalize_text};

// ---------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------

const EXIT_OK: i32 = 0;
const EXIT_REWRITES_NEEDED: i32 = 1;
const EXIT_PARSE_OR_IO_ERROR: i32 = 2;
const EXIT_RULE_FILE_ERROR: i32 = 3;

// ---------------------------------------------------------------------------
// Report format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Json,
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Entry point called from `main.rs`.
///
/// # Returns
///
/// The process exit code (0–3).
#[allow(clippy::too_many_arguments)]
pub fn run(
    file: &Path,
    rules_path: Option<&Path>,
    output: Option<&Path>,
    in_place: bool,
    explain: bool,
    report_format: ReportFormat,
    no_types: bool,
    no_fields: bool,
    check: bool,
) -> i32 {
    // ---- Read input file ----
    let source = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {}: {}", file.display(), e);
            return EXIT_PARSE_OR_IO_ERROR;
        }
    };

    // ---- Build rule set ----
    let mut rules = RuleSet::builtin();

    // Merge project-local `.agm-normalize.yaml` if present (silent if absent).
    let local_rules_path = std::env::current_dir()
        .ok()
        .map(|d| d.join(".agm-normalize.yaml"));
    if let Some(ref local) = local_rules_path {
        if local.exists() {
            match load_rules_file(local) {
                Ok(extra) => rules.merge(extra),
                Err(e) => {
                    eprintln!("warning: could not load .agm-normalize.yaml: {e}");
                }
            }
        }
    }

    // Merge `--rules` override if supplied.
    if let Some(rules_file) = rules_path {
        match load_rules_file(rules_file) {
            Ok(extra) => rules.merge(extra),
            Err(e) => {
                eprintln!("error: {}", e);
                return EXIT_RULE_FILE_ERROR;
            }
        }
    }

    // ---- Build config ----
    let config = NormalizeConfig {
        rules,
        warn_on_collision: true,
        normalize_types: !no_types,
        normalize_fields: !no_fields,
    };

    // ---- Normalize ----
    let (normalized, report) = match normalize_text(&source, &config) {
        Ok(pair) => pair,
        Err(errors) => {
            // Parse errors — render to stderr.
            for e in &errors {
                eprintln!("parse error: {}", e.message);
            }
            return EXIT_PARSE_OR_IO_ERROR;
        }
    };

    // ---- --check mode ----
    if check {
        if report.rewrites.is_empty() {
            return EXIT_OK;
        } else {
            return EXIT_REWRITES_NEEDED;
        }
    }

    // ---- Write output ----
    if in_place {
        if let Err(e) = std::fs::write(file, &normalized) {
            eprintln!("error: cannot write {}: {}", file.display(), e);
            return EXIT_PARSE_OR_IO_ERROR;
        }
    } else if let Some(out) = output {
        if let Err(e) = std::fs::write(out, &normalized) {
            eprintln!("error: cannot write {}: {}", out.display(), e);
            return EXIT_PARSE_OR_IO_ERROR;
        }
    } else {
        print!("{normalized}");
    }

    // ---- --explain mode ----
    if explain {
        render_report(&report, report_format);
    }

    EXIT_OK
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_rules_file(path: &Path) -> Result<RuleSet, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read rules file {}: {}", path.display(), e))?;
    RuleSet::from_yaml_str(&text).map_err(|e| match e {
        RuleParseError::Yaml(msg) => {
            format!("invalid YAML in rules file {}: {}", path.display(), msg)
        }
        RuleParseError::Empty => {
            format!("rules file {} is empty", path.display())
        }
    })
}

fn render_report(report: &NormalizeReport, format: ReportFormat) {
    match format {
        ReportFormat::Text => {
            eprintln!("\nNormalize report:");
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
            for w in &report.warnings {
                eprintln!("  warning [{:?}]: {}", w.code, w.message);
            }
        }
        ReportFormat::Json => match serde_json::to_string_pretty(report) {
            Ok(json) => eprintln!("{json}"),
            Err(e) => eprintln!("error: failed to serialize report: {e}"),
        },
    }
}
