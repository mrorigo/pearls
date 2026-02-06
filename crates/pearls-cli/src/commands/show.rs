// Rust guideline compliant 2026-02-06

//! Implementation of the `prl show` command.
//!
//! Displays detailed information about a specific Pearl,
//! supporting both full and partial ID resolution.

use crate::OutputFormatter;
use anyhow::Result;
use pearls_core::{identity, Storage};
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

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;

    // Try to resolve partial ID
    let full_id = resolve_id(&id, &storage, include_archived)?;

    // Load the Pearl
    let pearl = storage.load_by_id(&full_id)?;

    // Display the Pearl
    let mut output = formatter.format_pearl(&pearl);
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
    println!("{}", output);

    Ok(())
}

fn format_dep_type(dep_type: pearls_core::DepType) -> &'static str {
    match dep_type {
        pearls_core::DepType::Blocks => "blocks",
        pearls_core::DepType::ParentChild => "parent_child",
        pearls_core::DepType::Related => "related",
        pearls_core::DepType::DiscoveredFrom => "discovered_from",
    }
}

/// Resolves a partial or full Pearl ID.
///
/// # Arguments
///
/// * `id` - The ID to resolve (full or partial)
/// * `storage` - The storage instance
/// * `include_archived` - Whether to search archived Pearls
///
/// # Returns
///
/// The full Pearl ID if found.
///
/// # Errors
///
/// Returns an error if the ID cannot be resolved.
fn resolve_id(id: &str, storage: &Storage, include_archived: bool) -> Result<String> {
    // If ID is already in full format, validate and return it
    if id.starts_with("prl-") && (id.len() == 10 || id.len() == 12) {
        if identity::validate_id_format(id).is_ok() {
            return Ok(id.to_string());
        }
    }

    // Load all Pearls for partial ID resolution
    let mut pearls = storage.load_all()?;

    // Load archived Pearls if requested
    if include_archived {
        let archive_path = storage.path().parent().unwrap().join("archive.jsonl");
        if archive_path.exists() {
            let archive_storage = Storage::new(archive_path)?;
            if let Ok(archived) = archive_storage.load_all() {
                pearls.extend(archived);
            }
        }
    }

    // Try to resolve partial ID
    identity::resolve_partial_id(id, &pearls).map_err(|e| anyhow::anyhow!("{}", e))
}
