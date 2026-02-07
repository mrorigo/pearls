// Rust guideline compliant 2026-02-06

//! Pearls CLI Application
//!
//! Command-line interface for the Pearls issue tracking system.

use clap::Parser;

pub mod commands;
pub mod output;
pub mod progress;
pub mod terminal;

pub use output::{create_formatter, OutputFormatter};
pub use terminal::{get_terminal_width, should_use_color, wrap_text};

#[derive(Parser, Debug)]
#[command(
    name = "prl",
    version,
    about = "Pearls: Git-native distributed issue tracking",
    long_about = "Pearls is a Git-native issue tracker designed for agentic workflows. It stores all issues in JSONL and integrates with Git merges and hooks.",
    after_help = "Examples:\n  prl init\n  prl create \"Add index rebuild\" --priority 1 --label storage,performance\n  prl list --status open --sort updated_at\n  prl show prl-abc123\n  prl link prl-abc123 prl-def456 blocks\n  prl compact --threshold-days 30\n"
)]
struct Cli {
    /// Enable JSON output
    #[arg(long, global = true)]
    json: bool,

    /// Output format
    #[arg(long, value_enum, global = true)]
    format: Option<OutputFormat>,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Display timestamps as absolute UTC times
    #[arg(long, global = true)]
    absolute_time: bool,

    /// Custom config file path
    #[arg(long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Table,
    Plain,
}

#[derive(Debug, clap::Subcommand)]
enum Commands {
    /// Initialize a new Pearls repository
    Init,

    /// Create a new Pearl
    Create {
        /// Title of the Pearl
        title: String,

        /// Description of the Pearl
        #[arg(long)]
        description: Option<String>,

        /// Description from file ('-' for stdin)
        #[arg(long)]
        description_file: Option<String>,

        /// Priority level (0-4, default 2)
        #[arg(long)]
        priority: Option<u8>,

        /// Labels to assign
        #[arg(long, value_delimiter = ',')]
        label: Vec<String>,

        /// Author of the Pearl
        #[arg(long)]
        author: Option<String>,
    },

    /// Show details of a Pearl
    Show {
        /// Pearl ID (full or partial)
        id: String,

        /// Include archived Pearls
        #[arg(long)]
        include_archived: bool,
    },

    /// List Pearls
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,

        /// Filter by priority
        #[arg(long)]
        priority: Option<u8>,

        /// Filter by label
        #[arg(long, value_delimiter = ',')]
        label: Vec<String>,

        /// Filter by author
        #[arg(long)]
        author: Option<String>,

        /// Filter by created_at >= timestamp
        #[arg(long)]
        created_after: Option<i64>,

        /// Filter by created_at <= timestamp
        #[arg(long)]
        created_before: Option<i64>,

        /// Filter by updated_at >= timestamp
        #[arg(long)]
        updated_after: Option<i64>,

        /// Filter by updated_at <= timestamp
        #[arg(long)]
        updated_before: Option<i64>,

        /// Include archived Pearls
        #[arg(long)]
        include_archived: bool,

        /// Sort by field
        #[arg(long)]
        sort: Option<String>,

        /// Filter by dependency type
        #[arg(long, value_parser = ["blocks", "parent_child", "related", "discovered_from"])]
        dep_type: Option<String>,
    },

    /// Show ready queue
    Ready {
        /// Maximum number of items to show
        #[arg(long)]
        limit: Option<usize>,
    },

    /// Update a Pearl
    Update {
        /// Pearl ID
        id: String,

        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New description from file ('-' for stdin)
        #[arg(long)]
        description_file: Option<String>,

        /// New priority
        #[arg(long)]
        priority: Option<u8>,

        /// New status
        #[arg(long)]
        status: Option<String>,

        /// Add labels
        #[arg(long, value_delimiter = ',')]
        add_label: Vec<String>,

        /// Remove labels
        #[arg(long, value_delimiter = ',')]
        remove_label: Vec<String>,
    },

    /// Close a Pearl
    Close {
        /// Pearl ID
        id: String,
    },

    /// Link two Pearls with a dependency
    Link {
        /// Source Pearl ID
        from: String,

        /// Target Pearl ID
        to: String,

        /// Dependency type (blocks, parent_child, related, discovered_from)
        #[arg(value_parser = ["blocks", "parent_child", "related", "discovered_from"])]
        dep_type: String,
    },

    /// Remove a dependency link
    Unlink {
        /// Source Pearl ID
        from: String,

        /// Target Pearl ID
        to: String,
    },

    /// Show project status
    Status {
        /// Show detailed checklist
        #[arg(long)]
        detailed: bool,
    },

    /// Sync with remote repository
    Sync {
        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Compact old closed Pearls
    Compact {
        /// Days threshold for archival
        #[arg(long)]
        threshold_days: Option<u32>,

        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },

    /// Run integrity checks
    Doctor {
        /// Automatically fix detected issues
        #[arg(long)]
        fix: bool,
    },

    /// Run Pearls Git hooks
    Hooks {
        #[command(subcommand)]
        action: commands::hooks::HookAction,
    },

    /// Run the Pearls merge driver
    Merge {
        /// Path to ancestor version
        ancestor: String,

        /// Path to current version (ours)
        current: String,

        /// Path to other version (theirs)
        other: String,

        /// Path to output file
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Import from other formats
    Import {
        #[command(subcommand)]
        source: ImportSource,
    },

    /// Manage metadata
    Meta {
        #[command(subcommand)]
        action: MetaAction,
    },

    /// Manage Pearl comments
    Comments {
        #[command(subcommand)]
        action: CommentAction,
    },
}

#[derive(Debug, clap::Subcommand)]
enum ImportSource {
    /// Import from Beads format
    Beads {
        /// Path to Beads JSONL file
        path: String,
    },
}

#[derive(Debug, clap::Subcommand)]
enum MetaAction {
    /// Get metadata value
    Get {
        /// Pearl ID
        id: String,

        /// Metadata key
        key: String,
    },

    /// Set metadata value
    Set {
        /// Pearl ID
        id: String,

        /// Metadata key
        key: String,

        /// Metadata value (JSON)
        value: String,
    },
}

#[derive(Debug, clap::Subcommand)]
enum CommentAction {
    /// Add a comment to a Pearl
    Add {
        /// Pearl ID (full or partial)
        id: String,

        /// Comment text
        body: String,

        /// Comment author
        #[arg(long)]
        author: Option<String>,
    },

    /// List comments for a Pearl
    List {
        /// Pearl ID (full or partial)
        id: String,
    },

    /// Delete a comment from a Pearl
    Delete {
        /// Pearl ID (full or partial)
        id: String,

        /// Comment ID (full or partial)
        comment_id: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Determine output format and color usage
    let use_color = !cli.no_color && should_use_color();
    let format = match cli.format {
        Some(OutputFormat::Json) => "json",
        Some(OutputFormat::Table) => "table",
        Some(OutputFormat::Plain) => "plain",
        None => {
            if cli.json {
                "json"
            } else {
                "table"
            }
        }
    };
    let formatter = create_formatter(format, use_color, cli.absolute_time);

    match cli.command {
        Some(Commands::Init) => {
            commands::init::execute()?;
        }
        Some(Commands::Create {
            title,
            description,
            description_file,
            priority,
            label,
            author,
        }) => {
            commands::create::execute(
                title,
                description,
                description_file,
                priority,
                label,
                author,
            )?;
        }
        Some(Commands::Show {
            id,
            include_archived,
        }) => {
            commands::show::execute(id, include_archived, formatter.as_ref())?;
        }
        Some(Commands::List {
            status,
            priority,
            label,
            author,
            created_after,
            created_before,
            updated_after,
            updated_before,
            include_archived,
            sort,
            dep_type,
        }) => {
            commands::list::execute(
                status,
                priority,
                label,
                author,
                include_archived,
                sort,
                dep_type,
                created_after,
                created_before,
                updated_after,
                updated_before,
                formatter.as_ref(),
            )?;
        }
        Some(Commands::Ready { limit }) => {
            commands::ready::execute(limit)?;
        }
        Some(Commands::Update {
            id,
            title,
            description,
            description_file,
            priority,
            status,
            add_label,
            remove_label,
        }) => {
            commands::update::execute(
                id,
                title,
                description,
                description_file,
                priority,
                status,
                add_label,
                remove_label,
            )?;
        }
        Some(Commands::Close { id }) => {
            commands::close::execute(id)?;
        }
        Some(Commands::Link { from, to, dep_type }) => {
            commands::link::execute(from, to, dep_type)?;
        }
        Some(Commands::Unlink { from, to }) => {
            commands::unlink::execute(from, to)?;
        }
        Some(Commands::Status { detailed }) => {
            commands::status::execute(detailed)?;
        }
        Some(Commands::Sync { dry_run }) => {
            commands::sync::execute(dry_run)?;
        }
        Some(Commands::Compact {
            threshold_days,
            dry_run,
        }) => {
            commands::compact::execute(threshold_days, dry_run)?;
        }
        Some(Commands::Doctor { fix }) => {
            commands::doctor::execute(fix)?;
        }
        Some(Commands::Hooks { action }) => {
            commands::hooks::execute(action)?;
        }
        Some(Commands::Merge {
            ancestor,
            current,
            other,
            output,
        }) => {
            commands::merge::execute(ancestor, current, other, output)?;
        }
        Some(Commands::Import { source }) => match source {
            ImportSource::Beads { path } => {
                commands::import::import_beads(path)?;
            }
        },
        Some(Commands::Meta { action }) => match action {
            MetaAction::Get { id, key } => {
                commands::meta::get(id, key)?;
            }
            MetaAction::Set { id, key, value } => {
                commands::meta::set(id, key, value)?;
            }
        },
        Some(Commands::Comments { action }) => match action {
            CommentAction::Add { id, body, author } => {
                commands::comments::add(id, body, author)?;
            }
            CommentAction::List { id } => {
                commands::comments::list(id, format == "json")?;
            }
            CommentAction::Delete { id, comment_id } => {
                commands::comments::delete(id, comment_id)?;
            }
        },
        None => {
            println!("Use --help for usage information");
        }
    }

    Ok(())
}
