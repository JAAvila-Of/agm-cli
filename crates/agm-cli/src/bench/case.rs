//! Bench case loading and data types.

use std::path::Path;

use agm_core::model::fields::NodeType;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single benchmark prompt + expectation pair.
#[derive(Debug, Clone)]
pub struct BenchCase {
    /// Case identifier, e.g. `"ticket/a"`.
    pub id: String,
    /// AGM node type expected in the response.
    pub node_type: NodeType,
    /// The prompt text sent to the provider.
    pub prompt: String,
    /// Criteria used to judge the provider response.
    pub expectation: BenchExpectation,
}

/// Criteria used to judge whether a provider response is compliant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchExpectation {
    /// Fields that MUST be present in the parsed node (`(field, optional_value)`).
    #[serde(default)]
    pub required_fields: Vec<(String, Option<String>)>,
    /// Expected node type (as a string, e.g. `"ticket"`).
    #[serde(default)]
    pub expected_type: Option<String>,
    /// Whether the output must pass `agm validate` under Standard enforcement.
    #[serde(default)]
    pub must_validate: bool,
    /// Whether the output must match the type's JSON Schema.
    #[serde(default)]
    pub must_match_schema: bool,
}

// ---------------------------------------------------------------------------
// YAML shape used on disk
// ---------------------------------------------------------------------------

/// On-disk YAML shape for an `expected.yaml` file.
#[derive(Debug, Deserialize)]
struct ExpectedYaml {
    #[serde(default)]
    required_fields: Vec<RequiredFieldYaml>,
    #[serde(default)]
    expected_type: Option<String>,
    #[serde(default)]
    must_validate: bool,
    #[serde(default)]
    must_match_schema: bool,
}

#[derive(Debug, Deserialize)]
struct RequiredFieldYaml {
    field: String,
    #[serde(default)]
    value: Option<String>,
}

impl From<ExpectedYaml> for BenchExpectation {
    fn from(y: ExpectedYaml) -> Self {
        Self {
            required_fields: y
                .required_fields
                .into_iter()
                .map(|r| (r.field, r.value))
                .collect(),
            expected_type: y.expected_type,
            must_validate: y.must_validate,
            must_match_schema: y.must_match_schema,
        }
    }
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Load all bench cases from a fixture root directory.
///
/// Expected layout:
/// ```text
/// <root>/
///   <node_type>/
///     <case_id>/
///       prompt.txt
///       expected.yaml
/// ```
///
/// Case IDs are formed as `"<node_type>/<case_id>"`.
///
/// # Errors
///
/// Returns an error if any `prompt.txt` or `expected.yaml` cannot be read or
/// parsed.
pub fn load_from_dir(root: &Path) -> anyhow::Result<Vec<BenchCase>> {
    use anyhow::Context as _;
    let mut cases = Vec::new();

    for type_entry in read_sorted_dirs(root)? {
        let type_name = type_entry.file_name().to_string_lossy().into_owned();
        let node_type: NodeType = type_name.parse().unwrap();
        if let NodeType::Custom(_) = &node_type {
            return Err(anyhow::anyhow!("unknown node type directory: {type_name}"));
        }

        for case_entry in read_sorted_dirs(&type_entry.path())? {
            let case_name = case_entry.file_name().to_string_lossy().into_owned();
            let case_id = format!("{type_name}/{case_name}");

            let prompt_path = case_entry.path().join("prompt.txt");
            let expected_path = case_entry.path().join("expected.yaml");

            let prompt = std::fs::read_to_string(&prompt_path)
                .with_context(|| format!("reading {}", prompt_path.display()))?;

            let expected_raw = std::fs::read_to_string(&expected_path)
                .with_context(|| format!("reading {}", expected_path.display()))?;

            let expected_yaml: ExpectedYaml = serde_yaml::from_str(&expected_raw)
                .with_context(|| format!("parsing {}", expected_path.display()))?;

            cases.push(BenchCase {
                id: case_id,
                node_type: node_type.clone(),
                prompt,
                expectation: expected_yaml.into(),
            });
        }
    }

    Ok(cases)
}

fn read_sorted_dirs(path: &Path) -> anyhow::Result<Vec<std::fs::DirEntry>> {
    use anyhow::Context as _;
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .with_context(|| format!("reading directory {}", path.display()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    Ok(entries)
}

// ---------------------------------------------------------------------------
// Built-in fixtures (embedded at compile time)
// ---------------------------------------------------------------------------

/// Load all 12 built-in bench cases (embedded in the binary at compile time).
///
/// Cases are: `ticket/a`..`ticket/d`, `workflow/a`..`workflow/d`,
/// `orchestration/a`..`orchestration/d`.
#[must_use]
pub fn load_builtin_fixtures() -> Vec<BenchCase> {
    macro_rules! fixture {
        ($type:literal, $id:literal) => {
            (
                $type,
                $id,
                include_str!(concat!("fixtures/", $type, "/", $id, "/prompt.txt")),
                include_str!(concat!("fixtures/", $type, "/", $id, "/expected.yaml")),
            )
        };
    }

    let raw: &[(&str, &str, &str, &str)] = &[
        fixture!("ticket", "a"),
        fixture!("ticket", "b"),
        fixture!("ticket", "c"),
        fixture!("ticket", "d"),
        fixture!("workflow", "a"),
        fixture!("workflow", "b"),
        fixture!("workflow", "c"),
        fixture!("workflow", "d"),
        fixture!("orchestration", "a"),
        fixture!("orchestration", "b"),
        fixture!("orchestration", "c"),
        fixture!("orchestration", "d"),
    ];

    raw.iter()
        .map(|(type_name, case_name, prompt, expected_yaml)| {
            let case_id = format!("{type_name}/{case_name}");
            let node_type: NodeType = type_name.parse().expect("built-in type is valid");
            let expected_yaml_val: ExpectedYaml =
                serde_yaml::from_str(expected_yaml).expect("built-in expected.yaml is valid");
            BenchCase {
                id: case_id,
                node_type,
                prompt: prompt.to_string(),
                expectation: expected_yaml_val.into(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_case(root: &Path, type_name: &str, case_name: &str, prompt: &str, expected: &str) {
        let dir = root.join(type_name).join(case_name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("prompt.txt"), prompt).unwrap();
        fs::write(dir.join("expected.yaml"), expected).unwrap();
    }

    #[test]
    fn test_load_from_dir_basic() {
        let tmp = TempDir::new().unwrap();
        make_case(
            tmp.path(),
            "ticket",
            "a",
            "Create a ticket for the login bug",
            "expected_type: ticket\nmust_validate: true\nmust_match_schema: false\n",
        );
        let cases = load_from_dir(tmp.path()).unwrap();
        assert_eq!(cases.len(), 1);
        assert_eq!(cases[0].id, "ticket/a");
        assert_eq!(cases[0].node_type, NodeType::Ticket);
        assert!(cases[0].prompt.contains("login bug"));
        assert_eq!(
            cases[0].expectation.expected_type.as_deref(),
            Some("ticket")
        );
        assert!(cases[0].expectation.must_validate);
    }

    #[test]
    fn test_expectation_yaml_roundtrip() {
        let yaml = r#"
expected_type: workflow
must_validate: true
must_match_schema: true
required_fields:
  - field: summary
  - field: status
    value: pending
"#;
        let parsed: ExpectedYaml = serde_yaml::from_str(yaml).unwrap();
        let exp: BenchExpectation = parsed.into();
        assert_eq!(exp.expected_type.as_deref(), Some("workflow"));
        assert!(exp.must_validate);
        assert!(exp.must_match_schema);
        assert_eq!(exp.required_fields.len(), 2);
        assert_eq!(exp.required_fields[0].0, "summary");
        assert_eq!(exp.required_fields[1].0, "status");
        assert_eq!(exp.required_fields[1].1.as_deref(), Some("pending"));
    }

    #[test]
    fn test_load_builtin_fixtures_returns_12_cases() {
        let cases = load_builtin_fixtures();
        assert_eq!(cases.len(), 12);
        // Verify each type has 4 cases
        let ticket_count = cases.iter().filter(|c| c.id.starts_with("ticket/")).count();
        let workflow_count = cases
            .iter()
            .filter(|c| c.id.starts_with("workflow/"))
            .count();
        let orch_count = cases
            .iter()
            .filter(|c| c.id.starts_with("orchestration/"))
            .count();
        assert_eq!(ticket_count, 4);
        assert_eq!(workflow_count, 4);
        assert_eq!(orch_count, 4);
    }

    #[test]
    fn test_load_from_dir_unknown_type_returns_err() {
        let tmp = TempDir::new().unwrap();
        make_case(
            tmp.path(),
            "unknowntype",
            "a",
            "prompt",
            "expected_type: unknowntype\n",
        );
        let result = load_from_dir(tmp.path());
        assert!(result.is_err());
    }
}
