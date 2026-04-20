//! `schema` command: emit JSON Schema for a built-in AGM node type.
//!
//! Exit codes:
//!   0 — success.
//!   1 — unknown or custom type.
//!   2 — invalid flag combination or I/O error.

use std::path::{Path, PathBuf};

use agm_core::model::fields::NodeType;
use agm_core::schemas::{SchemaDialect, SchemaGenError, SchemaOptions, schema_for};

// ---------------------------------------------------------------------------
// Exit codes
// ---------------------------------------------------------------------------

const EXIT_OK: i32 = 0;
const EXIT_UNKNOWN_TYPE: i32 = 1;
const EXIT_INVALID_FLAGS: i32 = 2;

// ---------------------------------------------------------------------------
// Arg enums (mirrors clap ValueEnum in main.rs)
// ---------------------------------------------------------------------------

/// `--format` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFormatArg {
    /// JSON Schema (default)
    JsonSchema,
    /// YAML output
    Yaml,
}

/// `--for` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectArg {
    /// Raw JSON Schema (default)
    Vanilla,
    /// Anthropic tool-use shape
    AnthropicToolUse,
    /// OpenAI function-calling shape
    OpenAiTool,
}

impl DialectArg {
    fn to_core(self) -> SchemaDialect {
        match self {
            Self::Vanilla => SchemaDialect::JsonSchema,
            Self::AnthropicToolUse => SchemaDialect::AnthropicToolUse,
            Self::OpenAiTool => SchemaDialect::OpenAiTool,
        }
    }
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

/// Entry point called from `main.rs`.
///
/// `type_arg` is the string the user provided as `TYPE` (e.g. `"ticket"` or `"all"`).
#[allow(clippy::too_many_arguments)]
pub fn run(
    type_arg: &str,
    format: SchemaFormatArg,
    dialect: DialectArg,
    include_enums: bool,
    strict: bool,
    tool_name: Option<&str>,
    tool_description: Option<&str>,
    output: Option<&Path>,
    pretty: bool,
) -> i32 {
    let opts = SchemaOptions {
        dialect: dialect.to_core(),
        include_enums,
        strict,
        tool_name: tool_name.map(str::to_owned),
        tool_description: tool_description.map(str::to_owned),
    };

    if type_arg == "all" {
        run_all(format, &opts, output, pretty)
    } else {
        run_single(type_arg, format, &opts, output, pretty)
    }
}

// ---------------------------------------------------------------------------
// Single type
// ---------------------------------------------------------------------------

fn run_single(
    type_arg: &str,
    format: SchemaFormatArg,
    opts: &SchemaOptions,
    output: Option<&Path>,
    pretty: bool,
) -> i32 {
    let node_type = parse_node_type(type_arg);

    match schema_for(&node_type, opts) {
        Ok(schema) => {
            let rendered = match render_schema(&schema, format, pretty) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error: failed to serialize schema: {e}");
                    return EXIT_INVALID_FLAGS;
                }
            };
            write_output(&rendered, output)
        }
        Err(SchemaGenError::CustomType(s)) => {
            eprintln!("error: no schema defined for custom node type: `{s}`");
            eprintln!(
                "  hint: schemas are only available for built-in node types; pass one of:\n\
                         \t facts, rules, workflow, entity, decision, exception, example,\n\
                         \t glossary, anti_pattern, orchestration, ticket"
            );
            EXIT_UNKNOWN_TYPE
        }
        Err(SchemaGenError::InvalidToolName(name)) => {
            eprintln!(
                "error: --tool-name contains invalid characters (must match [a-z][a-z0-9_]*): `{name}`"
            );
            EXIT_INVALID_FLAGS
        }
    }
}

// ---------------------------------------------------------------------------
// `all` — emit one file per type
// ---------------------------------------------------------------------------

const ALL_TYPES: &[(&str, NodeType)] = &[
    ("facts", NodeType::Facts),
    ("rules", NodeType::Rules),
    ("workflow", NodeType::Workflow),
    ("entity", NodeType::Entity),
    ("decision", NodeType::Decision),
    ("exception", NodeType::Exception),
    ("example", NodeType::Example),
    ("glossary", NodeType::Glossary),
    ("anti_pattern", NodeType::AntiPattern),
    ("orchestration", NodeType::Orchestration),
    ("ticket", NodeType::Ticket),
];

fn run_all(
    format: SchemaFormatArg,
    opts: &SchemaOptions,
    output: Option<&Path>,
    pretty: bool,
) -> i32 {
    // Determine output directory.
    let out_dir: PathBuf = match output {
        Some(p) => p.to_owned(),
        None => {
            eprintln!("error: --output must be a directory when TYPE is \"all\"");
            return EXIT_INVALID_FLAGS;
        }
    };

    // Create the directory if needed.
    let base_dir = if opts.dialect != SchemaDialect::JsonSchema {
        // Dialect subdirectory, e.g. schemas/anthropic_tool_use/
        let sub = match opts.dialect {
            SchemaDialect::AnthropicToolUse => "anthropic_tool_use",
            SchemaDialect::OpenAiTool => "openai_tool",
            SchemaDialect::JsonSchema => "",
        };
        out_dir.join(sub)
    } else {
        out_dir.clone()
    };

    if let Err(e) = std::fs::create_dir_all(&base_dir) {
        eprintln!(
            "error: cannot create output directory {}: {e}",
            base_dir.display()
        );
        return EXIT_INVALID_FLAGS;
    }

    let ext = match format {
        SchemaFormatArg::Yaml => "yaml",
        SchemaFormatArg::JsonSchema => "json",
    };

    for (type_name, node_type) in ALL_TYPES {
        // Build per-type opts (inherit everything, override tool_name from defaults if needed)
        let schema = match schema_for(node_type, opts) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {e}");
                return EXIT_UNKNOWN_TYPE;
            }
        };

        let rendered = match render_schema(&schema, format, pretty) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: failed to serialize schema for {type_name}: {e}");
                return EXIT_INVALID_FLAGS;
            }
        };

        let file_path = base_dir.join(format!("{type_name}.{ext}"));
        if let Err(e) = std::fs::write(&file_path, rendered.as_bytes()) {
            eprintln!("error: cannot write {}: {e}", file_path.display());
            return EXIT_INVALID_FLAGS;
        }
    }

    EXIT_OK
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_node_type(s: &str) -> NodeType {
    s.parse().unwrap_or_else(|_| NodeType::Custom(s.to_owned()))
}

fn render_schema(
    schema: &serde_json::Value,
    format: SchemaFormatArg,
    pretty: bool,
) -> anyhow::Result<String> {
    match format {
        SchemaFormatArg::JsonSchema => {
            if pretty {
                Ok(serde_json::to_string_pretty(schema)?)
            } else {
                Ok(serde_json::to_string(schema)?)
            }
        }
        SchemaFormatArg::Yaml => Ok(serde_yaml::to_string(schema)?),
    }
}

fn write_output(content: &str, output: Option<&Path>) -> i32 {
    match output {
        Some(path) => match std::fs::write(path, content.as_bytes()) {
            Ok(()) => EXIT_OK,
            Err(e) => {
                eprintln!("error: cannot write {}: {e}", path.display());
                EXIT_INVALID_FLAGS
            }
        },
        None => {
            println!("{content}");
            EXIT_OK
        }
    }
}
