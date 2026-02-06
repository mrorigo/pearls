// Rust guideline compliant 2026-02-06

//! Pearls CLI Application
//!
//! Command-line interface for the Pearls issue tracking system.

use clap::Parser;

pub mod commands;
pub mod output;
pub mod terminal;

pub use output::{create_formatter, OutputFormatter};
pub use terminal::{get_terminal_width, should_use_color, wrap_text};

/// Pearls CLI - Git-native distributed issue tracking
#[derive(Parser, Debug)]
#[command(name = "prl")]
#[command(version, about, long_about = None)]
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

        /// Priority level (0-4, default 2)
        #[arg(long)]
        priority: Option<u8>,

        /// Labels to assign
        #[arg(long)]
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
        #[arg(long)]
        label: Vec<String>,

        /// Filter by author
        #[arg(long)]
        author: Option<String>,

        /// Include archived Pearls
        #[arg(long)]
        include_archived: bool,

        /// Sort by field
        #[arg(long)]
        sort: Option<String>,
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

        /// New priority
        #[arg(long)]
        priority: Option<u8>,

        /// New status
        #[arg(long)]
        status: Option<String>,

        /// Add labels
        #[arg(long)]
        add_label: Vec<String>,

        /// Remove labels
        #[arg(long)]
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Determine output format and color usage
    let use_color = !cli.no_color && should_use_color();
    let format = match cli.format {
        Some(OutputFormat::Json) => "json",
        Some(OutputFormat::Table) => "table",
        Some(OutputFormat::Plain) => "plain",
        None => "table",
    };
    let formatter = create_formatter(format, use_color);

    match cli.command {
        Some(Commands::Init) => {
            commands::init::execute()?;
        }
        Some(Commands::Create {
            title,
            description,
            priority,
            label,
            author,
        }) => {
            commands::create::execute(title, description, priority, label, author)?;
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
            include_archived,
            sort,
        }) => {
            commands::list::execute(
                status,
                priority,
                label,
                author,
                include_archived,
                sort,
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
            priority,
            status,
            add_label,
            remove_label,
        }) => {
            commands::update::execute(id, title, description, priority, status, add_label, remove_label)?;
        }
        Some(Commands::Close { id }) => {
            commands::close::execute(id)?;
        }
        Some(Commands::Link { from, to, dep_type }) => {
            println!("Linking {} -> {} ({})", from, to, dep_type);
        }
        Some(Commands::Unlink { from, to }) => {
            println!("Unlinking {} -> {}", from, to);
        }
        Some(Commands::Status { detailed }) => {
            println!("Checking project status...");
            if detailed {
                println!("  (detailed checklist)");
            }
        }
        Some(Commands::Sync { dry_run }) => {
            println!("Syncing with remote...");
            if dry_run {
                println!("  (dry-run mode)");
            }
        }
        Some(Commands::Compact {
            threshold_days,
            dry_run,
        }) => {
            println!("Compacting old Pearls...");
            if let Some(t) = threshold_days {
                println!("  Threshold: {} days", t);
            }
            if dry_run {
                println!("  (dry-run mode)");
            }
        }
        Some(Commands::Doctor { fix }) => {
            println!("Running integrity checks...");
            if fix {
                println!("  (auto-fix mode)");
            }
        }
        Some(Commands::Import { source }) => match source {
            ImportSource::Beads { path } => {
                println!("Importing from Beads: {}", path);
            }
        },
        Some(Commands::Meta { action }) => match action {
            MetaAction::Get { id, key } => {
                println!("Getting metadata: {} -> {}", id, key);
            }
            MetaAction::Set { id, key, value } => {
                println!("Setting metadata: {} -> {} = {}", id, key, value);
            }
        },
        None => {
            println!("Use --help for usage information");
        }
    }

    Ok(())
}
