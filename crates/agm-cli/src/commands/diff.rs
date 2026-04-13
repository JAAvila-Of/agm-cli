//! CLI `agm diff` command implementation.

use std::path::Path;

use agm_core::error::output::ErrorOutputFormat;

use super::helpers::{EXIT_SUCCESS, read_file};
use agm_core::diff::render::{DiffFormat, render_diff};

/// Exit code when non-breaking differences are found.
const EXIT_CHANGES: i32 = 1;
/// Exit code when breaking changes are found.
const EXIT_BREAKING: i32 = 2;

/// Runs the `diff` command.
///
/// Compares `left_path` (old/base) against `right_path` (new/changed) at
/// the semantic level and reports the result in the requested format.
///
/// # Exit codes
/// - `0` — no semantic differences (or no breaking differences with `--breaking-only`)
/// - `1` — non-breaking differences found
/// - `2` — breaking changes found (also used for I/O errors)
pub fn run(
    left_path: &Path,
    right_path: &Path,
    format: DiffFormat,
    breaking_only: bool,
    quiet: bool,
) -> i32 {
    // Read both files
    let left_source = read_file(left_path);
    let right_source = read_file(right_path);

    // Parse both files
    let left_name = left_path.display().to_string();
    let right_name = right_path.display().to_string();

    let left_file =
        super::helpers::parse_or_exit(&left_source, &left_name, ErrorOutputFormat::Text);
    let right_file =
        super::helpers::parse_or_exit(&right_source, &right_name, ErrorOutputFormat::Text);

    // Compute semantic diff
    let mut report = agm_core::diff::diff(&left_file, &right_file);

    // Filter to breaking only if requested
    if breaking_only {
        report = report.breaking_only();
    }

    // Determine exit code
    let exit_code = if report.is_empty() {
        EXIT_SUCCESS
    } else if report.has_breaking_changes() {
        EXIT_BREAKING
    } else {
        EXIT_CHANGES
    };

    // Render and print unless quiet mode
    if !quiet {
        let output = render_diff(&report, format);
        print!("{output}");
    }

    exit_code
}
