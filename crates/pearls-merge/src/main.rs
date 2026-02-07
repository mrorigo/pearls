// Rust guideline compliant 2026-02-06

//! Pearls Git Merge Driver
//!
//! Custom merge driver for JSONL files that performs semantic three-way merging.

use clap::Parser;
use pearls_merge::driver::merge_files;

/// Pearls merge driver for Git
#[derive(Parser, Debug)]
#[command(name = "pearls-merge")]
#[command(version, about = "Git merge driver for Pearls JSONL files")]
struct Cli {
    /// Path to ancestor version
    ancestor: String,

    /// Path to current version (ours)
    current: String,

    /// Path to other version (theirs)
    other: String,

    /// Path to output file
    #[arg(short, long)]
    output: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    merge_files(
        &cli.ancestor,
        &cli.current,
        &cli.other,
        cli.output.as_deref(),
    )?;
    Ok(())
}
