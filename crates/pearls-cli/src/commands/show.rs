// Rust guideline compliant 2026-02-06

//! Implementation of the `prl show` command.
//!
//! Displays detailed information about a specific Pearl,
//! supporting both full and partial ID resolution.

use crate::output_mode::is_json_output;
use crate::OutputFormatter;
use anyhow::Result;
use pearls_app::service::GraphService;
use pearls_core::identity;
use std::path::Path;

/// Shows details of a Pearl by ID.
///
/// Supports partial ID resolution (minimum 3 characters).
/// Searches both active and archived Pearls if requested.
///
/// # Arguments
///
/// * `id` - The Pearl ID (full or partial)
/// * `include_archived` - Whether to search archived Pearls
/// * `formatter` - The output formatter to use
///
/// # Returns
///
/// Ok if the Pearl was found and displayed, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - The Pearl ID is not found
/// - The Pearl ID is ambiguous (matches multiple Pearls)
/// - The file cannot be read
pub fn execute(id: String, include_archived: bool, formatter: &dyn OutputFormatter) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    // Verify .pearls directory exists
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let snapshot = GraphService::open(".")?.snapshot()?;
    let mut pearls = snapshot.data.active;
    if include_archived {
        pearls.extend(snapshot.data.archived);
    }
    let full_id = identity::resolve_partial_id(&id, &pearls)?;
    let pearl = pearls
        .into_iter()
        .find(|task| task.id == full_id)
        .ok_or_else(|| anyhow::anyhow!("Pearl not found: {full_id}"))?;

    display_pearl(pearl, formatter)
}

fn format_dep_type(dep_type: pearls_core::DepType) -> &'static str {
    match dep_type {
        pearls_core::DepType::Blocks => "blocks",
        pearls_core::DepType::ParentChild => "parent_child",
        pearls_core::DepType::Related => "related",
        pearls_core::DepType::DiscoveredFrom => "discovered_from",
    }
}

fn display_pearl(pearl: pearls_core::Pearl, formatter: &dyn OutputFormatter) -> Result<()> {
    let mut output = formatter.format_pearl(&pearl);
    if !is_json_output() {
        if !pearl.deps.is_empty() {
            output.push_str("\nDependencies:\n");
            for dep in &pearl.deps {
                output.push_str(&format!(
                    "  - {} ({})\n",
                    dep.target_id,
                    format_dep_type(dep.dep_type)
                ));
            }
        }
        if !pearl.comments.is_empty() {
            output.push_str("\nComments:\n");
            for comment in &pearl.comments {
                output.push_str(&format!(
                    "  - {} [{}] {}\n",
                    comment.id, comment.author, comment.body
                ));
            }
        }
    }
    println!("{}", output);

    Ok(())
}
