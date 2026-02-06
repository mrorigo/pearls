// Rust guideline compliant 2026-02-06

//! Pearls Git Merge Driver
//!
//! Custom merge driver for JSONL files that performs semantic three-way merging.

use clap::Parser;
use pearls_merge::merge::{merge_with_conflicts, MergeConflict};

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

    let ancestor = read_jsonl(&cli.ancestor)?;
    let ours = read_jsonl(&cli.current)?;
    let theirs = read_jsonl(&cli.other)?;

    let (merged, conflicts) = merge_with_conflicts(ancestor, ours, theirs)?;
    let output_path = cli.output.as_deref().unwrap_or(&cli.current);

    if conflicts.is_empty() {
        write_jsonl(output_path, &merged)?;
        return Ok(());
    }

    write_conflicts(output_path, &merged, &conflicts)?;
    anyhow::bail!(
        "Merge conflicts detected. Resolve conflicts in {}",
        output_path
    );
}

fn read_jsonl(path: &str) -> anyhow::Result<Vec<pearls_core::Pearl>> {
    let content = std::fs::read_to_string(path)?;
    let mut pearls = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let pearl: pearls_core::Pearl = serde_json::from_str(line)?;
        pearls.push(pearl);
    }
    Ok(pearls)
}

fn write_jsonl(path: &str, pearls: &[pearls_core::Pearl]) -> anyhow::Result<()> {
    let mut out = String::new();
    for pearl in pearls {
        let json = serde_json::to_string(pearl)?;
        out.push_str(&json);
        out.push('\n');
    }
    std::fs::write(path, out)?;
    Ok(())
}

fn write_conflicts(
    path: &str,
    merged: &[pearls_core::Pearl],
    conflicts: &[MergeConflict],
) -> anyhow::Result<()> {
    let mut out = String::new();
    for pearl in merged {
        let json = serde_json::to_string(pearl)?;
        out.push_str(&json);
        out.push('\n');
    }

    for conflict in conflicts {
        out.push_str(&format!("<<<<<<< ours {}\n", conflict.id));
        out.push_str(&serde_json::to_string(&conflict.ours)?);
        out.push('\n');
        out.push_str("=======\n");
        out.push_str(&serde_json::to_string(&conflict.theirs)?);
        out.push('\n');
        out.push_str(">>>>>>> theirs\n");
    }

    std::fs::write(path, out)?;
    Ok(())
}
