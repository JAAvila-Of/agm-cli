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
