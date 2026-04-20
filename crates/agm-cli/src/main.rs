mod commands;
mod runtime;

use std::path::PathBuf;

use agm_core::error::output::ErrorOutputFormat;
use agm_core::model::schema::EnforcementLevel;
use agm_core::renderer::RenderFormat;
use clap::{Parser, Subcommand, ValueEnum};

/// agm -- CLI tool for AGM files (Agent Graph Memory)
#[derive(Parser, Debug)]
#[command(name = "agm", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Validate an AGM file against the specification
    Validate {
        /// Path to the .agm file
        file: PathBuf,

        /// Type schema enforcement level
        #[arg(long, value_enum, default_value_t = EnforcementArg::Standard)]
        enforcement: EnforcementArg,

        /// Error output format
        #[arg(long = "errors-format", value_enum, default_value_t = ErrorsFormatArg::Text)]
        errors_format: ErrorsFormatArg,
    },

    /// Run extended quality checks (validate + lint heuristics)
    Lint {
        /// Path to the .agm file
        file: PathBuf,

        /// Type schema enforcement level
        #[arg(long, value_enum, default_value_t = EnforcementArg::Standard)]
        enforcement: EnforcementArg,

        /// Error output format
        #[arg(long = "errors-format", value_enum, default_value_t = ErrorsFormatArg::Text)]
        errors_format: ErrorsFormatArg,
    },

    /// Load nodes at a specific expansion level (output as JSON)
    Load {
        /// Path to the .agm file
        file: PathBuf,

        /// Loading mode: summary, operational, executable, full
        #[arg(long, default_value = "summary")]
        mode: String,

        /// Named load profile (overrides --mode)
        #[arg(long)]
        profile: Option<String>,

        /// Comma-separated list of node IDs to include
        #[arg(long)]
        nodes: Option<String>,
    },

    /// Render an AGM file to another format
    Render {
        /// Path to the .agm file
        file: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = RenderFormatArg::Markdown)]
        format: RenderFormatArg,
    },

    /// Output the dependency graph
    Graph {
        /// Path to the .agm file
        file: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = GraphFormatArg::Dot)]
        format: GraphFormatArg,

        /// Output topological order instead of graph format
        #[arg(long, default_value_t = false)]
        topo: bool,
    },

    /// Execute nodes in dependency order via an agent
    Run {
        /// Path to the .agm file
        file: PathBuf,

        /// Execute only this node (and its unfinished deps)
        #[arg(long)]
        node: Option<String>,

        /// Execute only this orchestration group
        #[arg(long)]
        group: Option<String>,

        /// Print execution plan without running
        #[arg(long, default_value_t = false)]
        dry_run: bool,

        /// Maximum concurrent node executions
        #[arg(long, default_value_t = 1)]
        concurrency: usize,

        /// Per-node timeout in seconds
        #[arg(long, default_value_t = 300)]
        timeout: u64,

        /// Agent backend to use
        #[arg(long, default_value = "shell")]
        agent: String,

        /// Working directory for file operations
        #[arg(long)]
        working_dir: Option<PathBuf>,

        /// Stop on first node failure
        #[arg(long, default_value_t = false)]
        fail_fast: bool,
    },

    /// Show execution status of an AGM file
    Status {
        /// Path to the .agm file
        file: PathBuf,

        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,

        /// Show status for a single node
        #[arg(long)]
        node: Option<String>,
    },

    /// Retry failed node(s)
    Retry {
        /// Path to the .agm file
        file: PathBuf,

        /// Node ID to retry
        #[arg(long)]
        node: Option<String>,

        /// Retry all failed nodes
        #[arg(long, default_value_t = false)]
        all_failed: bool,

        /// Working directory for file operations
        #[arg(long)]
        working_dir: Option<PathBuf>,

        /// Per-node timeout in seconds
        #[arg(long, default_value_t = 300)]
        timeout: u64,
    },

    /// Manage execution state
    State {
        #[command(subcommand)]
        action: StateAction,
    },

    /// Manage memory sidecars
    Mem {
        #[command(subcommand)]
        action: MemAction,
    },

    /// Build and display agent context for a node
    Context {
        /// Path to the .agm file
        file: PathBuf,

        /// Node ID to build context for
        #[arg(long)]
        node: String,

        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,

        /// Show token count and section breakdown
        #[arg(long, default_value_t = false)]
        token_count: bool,

        /// Working directory for file resolution
        #[arg(long)]
        working_dir: Option<PathBuf>,
    },

    /// Update agm to the latest version from GitHub Releases
    Update {
        /// Check for updates without installing
        #[arg(long, default_value_t = false)]
        check: bool,
    },

    /// Run verify checks on node(s)
    Verify {
        /// Path to the .agm file
        file: PathBuf,

        /// Node ID to verify
        #[arg(long)]
        node: Option<String>,

        /// Verify all nodes that have verify checks
        #[arg(long, default_value_t = false)]
        all: bool,

        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,

        /// Working directory for verify check resolution
        #[arg(long)]
        working_dir: Option<PathBuf>,

        /// Per-check timeout in seconds
        #[arg(long, default_value_t = 60)]
        timeout: u64,
    },

    /// Compare two AGM files at the semantic level
    Diff {
        /// Path to the left (old/base) AGM file
        left: PathBuf,

        /// Path to the right (new/changed) AGM file
        right: PathBuf,

        /// Output format
        #[arg(long, value_enum, default_value_t = DiffFormatArg::Text)]
        format: DiffFormatArg,

        /// Show only breaking changes
        #[arg(long, default_value_t = false)]
        breaking_only: bool,

        /// Quiet mode: exit code only (0=no changes, 1=changes, 2=breaking)
        #[arg(long, default_value_t = false)]
        quiet: bool,
    },

    /// Normalize non-canonical field and type names to their canonical forms
    Normalize {
        /// Path to the .agm file
        file: PathBuf,

        /// Additional or override rules YAML file
        #[arg(long)]
        rules: Option<PathBuf>,

        /// Write normalized output to this file (default: stdout)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Rewrite the file in place (conflicts with --output)
        #[arg(long, conflicts_with = "output")]
        in_place: bool,

        /// Print the normalize report after the output
        #[arg(long)]
        explain: bool,

        /// Report format when --explain is used
        #[arg(long = "report-format", value_enum, default_value_t = ReportFormatArg::Text)]
        report_format: ReportFormatArg,

        /// Skip type-level normalization
        #[arg(long)]
        no_types: bool,

        /// Skip field-level normalization
        #[arg(long)]
        no_fields: bool,

        /// Exit 0 if no rewrites needed, 1 if rewrites would be made (no output written)
        #[arg(long)]
        check: bool,
    },

    /// Emit JSON Schema for a built-in AGM node type
    Schema {
        /// Node type: facts, rules, workflow, entity, decision, exception, example,
        /// glossary, anti_pattern, orchestration, ticket, or "all"
        #[arg(value_name = "TYPE")]
        node_type: String,

        /// Output format: json-schema (default) or yaml
        #[arg(long = "format", value_enum, default_value_t = SchemaFormatArg::JsonSchema)]
        format: SchemaFormatArg,

        /// Dialect wrapper: vanilla (default), anthropic-tool-use, or openai-tool
        #[arg(long = "for", value_enum, default_value_t = DialectArg::Vanilla)]
        for_dialect: DialectArg,

        /// Emit enum constraints (default: true)
        #[arg(long = "include-enums", default_value_t = true)]
        include_enums: bool,

        /// Disable enum constraints
        #[arg(long = "no-include-enums", overrides_with = "include_enums")]
        no_include_enums: bool,

        /// Tighten schema: disallow non-allowed fields (additionalProperties: false)
        #[arg(long, default_value_t = false)]
        strict: bool,

        /// Override the tool name used in dialect wrapping
        #[arg(long)]
        tool_name: Option<String>,

        /// Override the tool description used in dialect wrapping
        #[arg(long)]
        tool_description: Option<String>,

        /// Write output to file or directory (default: stdout)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Pretty-print JSON (default: true)
        #[arg(long, default_value_t = true)]
        pretty: bool,
    },

    /// Ingest tool-call JSON args into canonical AGM text
    Ingest {
        /// Node type: facts, rules, workflow, entity, decision, exception,
        /// example, glossary, anti_pattern, orchestration, ticket
        #[arg(value_name = "TYPE")]
        type_: String,

        /// Package name for the generated AGM file header (required)
        #[arg(long)]
        package: String,

        /// Node id (single mode) or id prefix (batch mode)
        #[arg(long)]
        id: Option<String>,

        /// Read JSON from this file instead of stdin
        #[arg(long)]
        file: Option<PathBuf>,

        /// Skip field-name normalization
        #[arg(long, default_value_t = false)]
        no_normalize: bool,

        /// Skip JSON Schema pre-check
        #[arg(long, default_value_t = false)]
        no_schema_check: bool,

        /// Enforcement level for post-build validation
        #[arg(long, value_enum, default_value_t = EnforcementArg::Standard)]
        enforcement: EnforcementArg,

        /// Write AGM output to this file (default: stdout)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Package version in the generated file header
        #[arg(long, default_value = "0.1.0")]
        version: String,

        /// Optional title field in the generated file header
        #[arg(long)]
        header_title: Option<String>,
    },

    /// Compile a Markdown file into an AGM file
    Compile {
        /// Path to the input Markdown file
        input: PathBuf,

        /// Output file path [default: stdout]
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Package name for the generated AGM file
        #[arg(long)]
        package: String,

        /// Version for the generated AGM file
        #[arg(long, default_value = "0.1.0")]
        version: String,

        /// Prefix for generated node IDs
        #[arg(long)]
        id_prefix: Option<String>,

        /// Minimum confidence for node inclusion (0.0-1.0)
        #[arg(long, default_value_t = 0.5)]
        min_confidence: f32,

        /// Run validator on generated output
        #[arg(long, default_value_t = false)]
        validate: bool,

        /// Output as JSON instead of AGM
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum StateAction {
    /// List all node states
    List {
        /// Path to the .agm file
        file: PathBuf,
        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Get state for a specific node
    Get {
        /// Path to the .agm file
        file: PathBuf,
        /// Node ID
        #[arg(long)]
        node: String,
    },
    /// Export state to a file
    Export {
        /// Path to the .agm file
        file: PathBuf,
        /// Export format: json, agm
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Import state from a file
    Import {
        /// Path to the .agm file
        file: PathBuf,
        /// Path to the state file to import
        #[arg(long)]
        from: PathBuf,
    },
    /// Reset execution state
    Reset {
        /// Path to the .agm file
        file: PathBuf,
        /// Reset only this node
        #[arg(long)]
        node: Option<String>,
        /// Keep completed nodes (only reset non-completed)
        #[arg(long, default_value_t = false)]
        keep_completed: bool,
        /// Skip confirmation prompt
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum MemAction {
    /// List memory entries
    List {
        /// Path to the .agm file
        file: PathBuf,
        /// Filter by topic
        #[arg(long)]
        topic: Option<String>,
        /// Filter by scope (project, global)
        #[arg(long)]
        scope: Option<String>,
        /// Output as JSON
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Get a specific memory entry by key
    Get {
        /// Path to the .agm file
        file: PathBuf,
        /// Memory key
        #[arg(long)]
        key: String,
    },
    /// Export memory to a file
    Export {
        /// Path to the .agm file
        file: PathBuf,
        /// Export format: json, agm
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Import memory from a file
    Import {
        /// Path to the .agm file
        file: PathBuf,
        /// Path to the memory file to import
        #[arg(long)]
        from: PathBuf,
    },
    /// Garbage-collect expired memory entries
    Gc {
        /// Path to the .agm file
        file: PathBuf,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum EnforcementArg {
    Strict,
    Standard,
    Permissive,
}

impl EnforcementArg {
    fn to_core(&self) -> EnforcementLevel {
        match self {
            Self::Strict => EnforcementLevel::Strict,
            Self::Standard => EnforcementLevel::Standard,
            Self::Permissive => EnforcementLevel::Permissive,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum ErrorsFormatArg {
    Text,
    Json,
}

impl ErrorsFormatArg {
    fn to_core(&self) -> ErrorOutputFormat {
        match self {
            Self::Text => ErrorOutputFormat::Text,
            Self::Json => ErrorOutputFormat::Json,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum RenderFormatArg {
    Json,
    JsonCanonical,
    Markdown,
    Agm,
    Dot,
    Mermaid,
}

impl RenderFormatArg {
    fn to_core(&self) -> RenderFormat {
        match self {
            Self::Json => RenderFormat::Json,
            Self::JsonCanonical => RenderFormat::JsonCanonical,
            Self::Markdown => RenderFormat::Markdown,
            Self::Agm => RenderFormat::Canonical,
            Self::Dot => RenderFormat::Dot,
            Self::Mermaid => RenderFormat::Mermaid,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum GraphFormatArg {
    Dot,
    Mermaid,
}

#[derive(Debug, Clone, ValueEnum)]
enum DiffFormatArg {
    Text,
    Json,
    Markdown,
}

impl DiffFormatArg {
    fn to_core(&self) -> agm_core::diff::render::DiffFormat {
        match self {
            Self::Text => agm_core::diff::render::DiffFormat::Text,
            Self::Json => agm_core::diff::render::DiffFormat::Json,
            Self::Markdown => agm_core::diff::render::DiffFormat::Markdown,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum ReportFormatArg {
    Text,
    Json,
}

impl ReportFormatArg {
    fn to_core(&self) -> commands::normalize::ReportFormat {
        match self {
            Self::Text => commands::normalize::ReportFormat::Text,
            Self::Json => commands::normalize::ReportFormat::Json,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum SchemaFormatArg {
    /// JSON Schema (default)
    #[value(name = "json-schema")]
    JsonSchema,
    /// YAML output
    Yaml,
}

impl SchemaFormatArg {
    fn to_cmd(self) -> commands::schema_cmd::SchemaFormatArg {
        match self {
            Self::JsonSchema => commands::schema_cmd::SchemaFormatArg::JsonSchema,
            Self::Yaml => commands::schema_cmd::SchemaFormatArg::Yaml,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum DialectArg {
    /// Raw JSON Schema
    #[value(name = "vanilla")]
    Vanilla,
    /// Anthropic tool-use shape
    #[value(name = "anthropic-tool-use")]
    AnthropicToolUse,
    /// OpenAI function-calling shape
    #[value(name = "openai-tool")]
    OpenAiTool,
}

impl DialectArg {
    fn to_cmd(self) -> commands::schema_cmd::DialectArg {
        match self {
            Self::Vanilla => commands::schema_cmd::DialectArg::Vanilla,
            Self::AnthropicToolUse => commands::schema_cmd::DialectArg::AnthropicToolUse,
            Self::OpenAiTool => commands::schema_cmd::DialectArg::OpenAiTool,
        }
    }
}

fn main() -> anyhow::Result<()> {
    // Configure miette for fancy terminal output
    miette::set_hook(Box::new(|_| {
        Box::new(
            miette::MietteHandlerOpts::new()
                .terminal_links(true)
                .unicode(true)
                .context_lines(2)
                .build(),
        )
    }))?;

    let cli = Cli::parse();

    let exit_code = match cli.command {
        Commands::Validate {
            file,
            enforcement,
            errors_format,
        } => commands::validate::run(&file, enforcement.to_core(), errors_format.to_core()),

        Commands::Lint {
            file,
            enforcement,
            errors_format,
        } => commands::lint::run(&file, enforcement.to_core(), errors_format.to_core()),

        Commands::Load {
            file,
            mode,
            profile,
            nodes,
        } => commands::load::run(&file, &mode, profile.as_deref(), nodes.as_deref()),

        Commands::Render { file, format } => commands::render::run(&file, format.to_core()),

        Commands::Graph { file, format, topo } => {
            let render_format = match format {
                GraphFormatArg::Dot => RenderFormat::Dot,
                GraphFormatArg::Mermaid => RenderFormat::Mermaid,
            };
            commands::graph::run(&file, render_format, topo)
        }

        Commands::Run {
            file,
            node,
            group,
            dry_run,
            concurrency,
            timeout,
            agent: _,
            working_dir,
            fail_fast,
        } => {
            let wd = working_dir
                .unwrap_or_else(|| std::env::current_dir().expect("cannot determine cwd"));
            commands::run::run(
                &file,
                node.as_deref(),
                group.as_deref(),
                dry_run,
                concurrency,
                timeout,
                &wd,
                fail_fast,
            )
        }

        Commands::Status { file, json, node } => {
            commands::status::run(&file, json, node.as_deref())
        }

        Commands::Retry {
            file,
            node,
            all_failed,
            working_dir,
            timeout,
        } => {
            let wd = working_dir
                .unwrap_or_else(|| std::env::current_dir().expect("cannot determine cwd"));
            commands::retry::run(&file, node.as_deref(), all_failed, &wd, timeout)
        }

        Commands::State { action } => match action {
            StateAction::List { file, json } => commands::state_cmd::list(&file, json),
            StateAction::Get { file, node } => commands::state_cmd::get(&file, &node),
            StateAction::Export { file, format } => commands::state_cmd::export(&file, &format),
            StateAction::Import { file, from } => commands::state_cmd::import(&file, &from),
            StateAction::Reset {
                file,
                node,
                keep_completed,
                yes,
            } => commands::state_cmd::reset(&file, node.as_deref(), keep_completed, yes),
        },

        Commands::Mem { action } => match action {
            MemAction::List {
                file,
                topic,
                scope,
                json,
            } => commands::mem_cmd::list(&file, topic.as_deref(), scope.as_deref(), json),
            MemAction::Get { file, key } => commands::mem_cmd::get(&file, &key),
            MemAction::Export { file, format } => commands::mem_cmd::export(&file, &format),
            MemAction::Import { file, from } => commands::mem_cmd::import(&file, &from),
            MemAction::Gc { file } => commands::mem_cmd::gc(&file),
        },

        Commands::Context {
            file,
            node,
            json,
            token_count,
            working_dir,
        } => {
            let wd = working_dir
                .unwrap_or_else(|| std::env::current_dir().expect("cannot determine cwd"));
            commands::context::run(&file, &node, json, token_count, &wd)
        }

        Commands::Update { check } => commands::update::run(check),

        Commands::Verify {
            file,
            node,
            all,
            json,
            working_dir,
            timeout,
        } => {
            let wd = working_dir
                .unwrap_or_else(|| std::env::current_dir().expect("cannot determine cwd"));
            commands::verify_cmd::run(&file, node.as_deref(), all, json, &wd, timeout)
        }

        Commands::Diff {
            left,
            right,
            format,
            breaking_only,
            quiet,
        } => commands::diff::run(&left, &right, format.to_core(), breaking_only, quiet),

        Commands::Normalize {
            file,
            rules,
            output,
            in_place,
            explain,
            report_format,
            no_types,
            no_fields,
            check,
        } => commands::normalize::run(
            &file,
            rules.as_deref(),
            output.as_deref(),
            in_place,
            explain,
            report_format.to_core(),
            no_types,
            no_fields,
            check,
        ),

        Commands::Schema {
            node_type,
            format,
            for_dialect,
            include_enums,
            no_include_enums,
            strict,
            tool_name,
            tool_description,
            output,
            pretty,
        } => commands::schema_cmd::run(
            &node_type,
            format.to_cmd(),
            for_dialect.to_cmd(),
            include_enums && !no_include_enums,
            strict,
            tool_name.as_deref(),
            tool_description.as_deref(),
            output.as_deref(),
            pretty,
        ),

        Commands::Ingest {
            type_,
            package,
            id,
            file,
            no_normalize,
            no_schema_check,
            enforcement,
            output,
            version,
            header_title,
        } => commands::ingest::run(
            &type_,
            &package,
            id.as_deref(),
            file.as_deref(),
            no_normalize,
            no_schema_check,
            enforcement.to_core(),
            output.as_deref(),
            &version,
            header_title.as_deref(),
        ),

        Commands::Compile {
            input,
            output,
            package,
            version,
            id_prefix,
            min_confidence,
            validate,
            json,
        } => commands::compile::run(
            &input,
            output.as_deref(),
            &package,
            &version,
            id_prefix.as_deref(),
            min_confidence,
            validate,
            json,
        ),
    };

    std::process::exit(exit_code);
}
