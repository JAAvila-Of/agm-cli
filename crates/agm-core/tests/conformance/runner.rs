//! Conformance test runner: fixture discovery, `# expect:` header parsing,
//! and assertion helpers shared across all conformance test modules.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use agm_core::error::ErrorCode;
use agm_core::model::fields::Span;
use agm_core::model::file::AgmFile;
use agm_core::model::node::Node;
use agm_core::model::schema::EnforcementLevel;
use agm_core::parser::parse;
use agm_core::renderer::canonical::render_canonical;
use agm_core::renderer::json_canonical::{agm_to_json, json_to_agm};
use agm_core::validator::{ValidateOptions, ValidationScope, validate};

// ---------------------------------------------------------------------------
// ExpectDirective
// ---------------------------------------------------------------------------

/// Represents the expected outcome declared in a fixture's `# expect:` header.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpectDirective {
    /// File must parse without any error-severity diagnostics.
    ParseOk,
    /// File must parse AND validate without any error-severity diagnostics.
    ValidateOk,
    /// File must produce at least one diagnostic with this error code.
    Error(ErrorCode),
    /// File must produce at least one warning-severity diagnostic with this code.
    Warning(ErrorCode),
    /// AGM -> JSON must match companion `.json` file.
    JsonForwardOk,
    /// JSON -> AGM -> canonical text -> parse -> JSON must round-trip to the same JSON.
    JsonRoundtripOk,
}

// ---------------------------------------------------------------------------
// Header parsing
// ---------------------------------------------------------------------------

/// Parses the first line of fixture content for `# expect: <directive>`.
///
/// Returns `None` if the first line doesn't match the expected pattern.
pub fn parse_expect_header(content: &str) -> Option<ExpectDirective> {
    let first_line = content.lines().next()?;
    let directive_str = first_line.strip_prefix("# expect:")?.trim();

    match directive_str {
        "parse ok" => Some(ExpectDirective::ParseOk),
        "validate ok" => Some(ExpectDirective::ValidateOk),
        "json forward ok" => Some(ExpectDirective::JsonForwardOk),
        "json roundtrip ok" => Some(ExpectDirective::JsonRoundtripOk),
        other => {
            if let Some(code_str) = other.strip_prefix("error ") {
                let code = ErrorCode::from_str(code_str.trim())
                    .unwrap_or_else(|_| panic!("unknown error code in fixture header: {code_str}"));
                Some(ExpectDirective::Error(code))
            } else if let Some(code_str) = other.strip_prefix("warning ") {
                let code = ErrorCode::from_str(code_str.trim()).unwrap_or_else(|_| {
                    panic!("unknown warning code in fixture header: {code_str}")
                });
                Some(ExpectDirective::Warning(code))
            } else {
                None
            }
        }
    }
}

/// Extracts the `# description:` line from fixture content (second line).
///
/// Used in failure messages for additional context.
pub fn parse_description(content: &str) -> Option<&str> {
    content
        .lines()
        .nth(1)
        .and_then(|line| line.strip_prefix("# description:"))
        .map(str::trim)
}

// ---------------------------------------------------------------------------
// Fixture discovery
// ---------------------------------------------------------------------------

/// Returns the path to `tests/fixtures/{relative}` relative to the
/// `agm-core` crate manifest directory.
pub fn fixtures_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
        .join(relative)
}

/// Walks `dir` and returns sorted paths of all files with the given extension.
pub fn discover_fixtures(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == extension {
                        paths.push(path);
                    }
                }
            }
        }
    }
    paths.sort();
    paths
}

// ---------------------------------------------------------------------------
// Enforcement level detection
// ---------------------------------------------------------------------------

/// Detects whether a fixture requires strict enforcement from its content.
///
/// Returns `Strict` if the first few lines contain `# enforcement: strict`,
/// otherwise returns `Standard`.
fn detect_enforcement(content: &str) -> EnforcementLevel {
    for line in content.lines().take(5) {
        if line.trim() == "# enforcement: strict" {
            return EnforcementLevel::Strict;
        }
    }
    EnforcementLevel::Standard
}

// ---------------------------------------------------------------------------
// Assertion helpers
// ---------------------------------------------------------------------------

/// Runs the parse pipeline and asserts based on the directive.
///
/// Used for `ParseOk` and `Error(P-code)` directives.
pub fn assert_parse_conformance(path: &Path, content: &str, directive: &ExpectDirective) {
    let desc = parse_description(content).unwrap_or("");
    let file_name = path.display().to_string();

    match directive {
        ExpectDirective::ParseOk => {
            match parse(content) {
                Ok(_) => {} // pass
                Err(errors) => panic!(
                    "FAIL [parse ok] {file_name}\n  desc: {desc}\n  got {} parse error(s): {errors:?}",
                    errors.len()
                ),
            }
        }
        ExpectDirective::Error(expected_code) => {
            // For P-codes: check only parse errors.
            // For V-codes: parse (allow parse errors) then validate.
            let parse_result = parse(content);
            let parse_errors = match &parse_result {
                Ok(_) => vec![],
                Err(errs) => errs.clone(),
            };

            let all_codes: Vec<ErrorCode> =
                if expected_code.category() == agm_core::error::codes::ErrorCategory::Parse {
                    parse_errors.iter().map(|e| e.code).collect()
                } else {
                    // V-code: validate regardless of parse result
                    let validate_codes = match &parse_result {
                        Ok(file) => {
                            let enforcement = detect_enforcement(content);
                            let opts = ValidateOptions {
                                enforcement_level: enforcement,
                                import_resolver: None,
                                scope: ValidationScope::File,
                            };
                            let diags = validate(file, content, &file_name, &opts);
                            diags
                                .diagnostics()
                                .iter()
                                .map(|d| d.code)
                                .collect::<Vec<_>>()
                        }
                        Err(_) => vec![],
                    };
                    let mut all = parse_errors.iter().map(|e| e.code).collect::<Vec<_>>();
                    all.extend(validate_codes);
                    all
                };

            assert!(
                all_codes.contains(expected_code),
                "FAIL [error {expected_code}] {file_name}\n  desc: {desc}\n  got codes: {all_codes:?}"
            );
        }
        other => panic!("assert_parse_conformance called with unexpected directive: {other:?}"),
    }
}

/// Runs the parse + validate pipeline and asserts based on the directive.
///
/// Used for `ValidateOk`, `Error(V-code)`, and `Warning` directives.
pub fn assert_validate_conformance(path: &Path, content: &str, directive: &ExpectDirective) {
    let desc = parse_description(content).unwrap_or("");
    let file_name = path.display().to_string();
    let enforcement = detect_enforcement(content);
    let opts = ValidateOptions {
        enforcement_level: enforcement,
        import_resolver: None,
        scope: ValidationScope::File,
    };

    match directive {
        ExpectDirective::ValidateOk => {
            let file = parse(content).unwrap_or_else(|errs| {
                panic!(
                    "FAIL [validate ok - parse step] {file_name}\n  desc: {desc}\n  parse errors: {errs:?}"
                )
            });
            let diags = validate(&file, content, &file_name, &opts);
            if diags.has_errors() {
                let errors: Vec<_> = diags
                    .diagnostics()
                    .iter()
                    .filter(|d| d.is_error())
                    .collect();
                panic!(
                    "FAIL [validate ok] {file_name}\n  desc: {desc}\n  got {} error(s): {errors:?}",
                    errors.len()
                );
            }
        }
        ExpectDirective::Error(expected_code) => {
            let parse_result = parse(content);
            let parse_errors = match &parse_result {
                Ok(_) => vec![],
                Err(errs) => errs.clone(),
            };

            let validate_codes = match &parse_result {
                Ok(file) => {
                    let diags = validate(file, content, &file_name, &opts);
                    diags
                        .diagnostics()
                        .iter()
                        .map(|d| d.code)
                        .collect::<Vec<_>>()
                }
                Err(_) => vec![],
            };

            let mut all_codes: Vec<ErrorCode> = parse_errors.iter().map(|e| e.code).collect();
            all_codes.extend(validate_codes);

            assert!(
                all_codes.contains(expected_code),
                "FAIL [error {expected_code}] {file_name}\n  desc: {desc}\n  got codes: {all_codes:?}"
            );
        }
        ExpectDirective::Warning(expected_code) => {
            let file = match parse(content) {
                Ok(f) => f,
                Err(errs) => {
                    // Check if the warning is in parse errors
                    let found = errs
                        .iter()
                        .any(|e| e.code == *expected_code && e.is_warning());
                    assert!(
                        found,
                        "FAIL [warning {expected_code}] {file_name}\n  desc: {desc}\n  parse failed without expected warning"
                    );
                    return;
                }
            };
            let diags = validate(&file, content, &file_name, &opts);
            let warning_codes: Vec<ErrorCode> = diags
                .diagnostics()
                .iter()
                .filter(|d| d.is_warning())
                .map(|d| d.code)
                .collect();
            assert!(
                warning_codes.contains(expected_code),
                "FAIL [warning {expected_code}] {file_name}\n  desc: {desc}\n  got warning codes: {warning_codes:?}"
            );
        }
        other => {
            panic!("assert_validate_conformance called with unexpected directive: {other:?}")
        }
    }
}

// ---------------------------------------------------------------------------
// Span-stripping helpers (spans change between parse and re-parse)
// ---------------------------------------------------------------------------

fn strip_spans(file: &AgmFile) -> AgmFile {
    AgmFile {
        header: file.header.clone(),
        nodes: file.nodes.iter().map(strip_node_span).collect(),
    }
}

fn strip_node_span(node: &Node) -> Node {
    Node {
        span: Span::default(),
        ..node.clone()
    }
}

/// Runs the AGM parse round-trip: parse -> render_canonical -> parse again.
///
/// Asserts the two ASTs are semantically equal (spans excluded).
pub fn assert_parse_roundtrip(path: &Path, content: &str) {
    let desc = parse_description(content).unwrap_or("");
    let file_name = path.display().to_string();

    let ast1 = parse(content).unwrap_or_else(|errs| {
        panic!(
            "FAIL [parse roundtrip - initial parse] {file_name}\n  desc: {desc}\n  errors: {errs:?}"
        )
    });

    let canonical = render_canonical(&ast1);

    let ast2 = parse(&canonical).unwrap_or_else(|errs| {
        panic!(
            "FAIL [parse roundtrip - re-parse] {file_name}\n  desc: {desc}\n  canonical:\n{canonical}\n  errors: {errs:?}"
        )
    });

    let stripped1 = strip_spans(&ast1);
    let stripped2 = strip_spans(&ast2);

    assert_eq!(
        stripped1, stripped2,
        "FAIL [parse roundtrip - AST mismatch] {file_name}\n  desc: {desc}"
    );
}

/// Runs the JSON forward conformance test: AGM -> JSON compared to companion `.json` file.
pub fn assert_json_forward(agm_path: &Path) {
    let agm_content = std::fs::read_to_string(agm_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", agm_path.display()));

    let json_path = agm_path.with_extension("json");
    let json_content = std::fs::read_to_string(&json_path).unwrap_or_else(|e| {
        panic!(
            "cannot read companion JSON file {}: {e}",
            json_path.display()
        )
    });

    let expected: serde_json::Value = serde_json::from_str(&json_content)
        .unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", json_path.display()));

    let file = parse(&agm_content).unwrap_or_else(|errs| {
        panic!(
            "FAIL [json forward - parse] {}\n  errors: {errs:?}",
            agm_path.display()
        )
    });

    let actual = agm_to_json(&file);

    assert_eq!(
        actual,
        expected,
        "FAIL [json forward] {}\n  actual != expected",
        agm_path.display()
    );
}

/// Runs the JSON round-trip: JSON -> AGM -> canonical text -> parse -> JSON.
pub fn assert_json_roundtrip(json_path: &Path) {
    let json_content = std::fs::read_to_string(json_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", json_path.display()));

    let original: serde_json::Value = serde_json::from_str(&json_content)
        .unwrap_or_else(|e| panic!("invalid JSON in {}: {e}", json_path.display()));

    let file = json_to_agm(&original).unwrap_or_else(|e| {
        panic!(
            "FAIL [json roundtrip - json_to_agm] {}\n  error: {e}",
            json_path.display()
        )
    });

    let canonical_text = render_canonical(&file);

    let file2 = parse(&canonical_text).unwrap_or_else(|errs| {
        panic!(
            "FAIL [json roundtrip - re-parse] {}\n  canonical:\n{canonical_text}\n  errors: {errs:?}",
            json_path.display()
        )
    });

    let roundtripped = agm_to_json(&file2);

    assert_eq!(
        original,
        roundtripped,
        "FAIL [json roundtrip] {}\n  original != roundtripped",
        json_path.display()
    );
}

// ---------------------------------------------------------------------------
// Generic conformance runner
// ---------------------------------------------------------------------------

/// Runs a directory of fixtures against the given assertion function.
///
/// Collects all failures and panics at the end with a combined report.
/// Returns the number of fixtures tested.
pub fn run_fixture_dir<F>(dir: &Path, extension: &str, run_one: F) -> usize
where
    F: Fn(&Path, &str, &ExpectDirective) + std::panic::RefUnwindSafe,
{
    let fixtures = discover_fixtures(dir, extension);
    assert!(
        !fixtures.is_empty(),
        "No fixtures found in {}",
        dir.display()
    );

    let mut failures = Vec::new();
    let count = fixtures.len();

    for path in &fixtures {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

        let directive = parse_expect_header(&content).unwrap_or_else(|| {
            panic!(
                "missing or invalid '# expect:' header in {}",
                path.display()
            )
        });

        if let Err(msg) = std::panic::catch_unwind(|| {
            run_one(path, &content, &directive);
        }) {
            let msg_str = if let Some(s) = msg.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = msg.downcast_ref::<&str>() {
                s.to_string()
            } else {
                format!("panic in {}", path.display())
            };
            failures.push(msg_str);
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} conformance failure(s) in {}:\n\n{}",
            failures.len(),
            dir.display(),
            failures.join("\n\n")
        );
    }

    count
}
