// Rust guideline compliant 2026-02-06

//! Implementation of the `prl unlink` command.
//!
//! Removes a dependency relationship between two Pearls.

use anyhow::Result;
use pearls_core::{identity, IssueGraph, Storage};
use std::path::Path;

/// Removes a dependency link between two Pearls.
///
/// # Arguments
///
/// * `from` - The source Pearl ID (full or partial)
/// * `to` - The target Pearl ID (full or partial)
///
/// # Returns
///
/// Ok if the dependency was removed successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - Either Pearl ID is not found
/// - The dependency does not exist
/// - The file cannot be written
pub fn execute(from: String, to: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let mut pearls = storage.load_all()?;

    let from_id = resolve_id(&from, &pearls)?;
    let to_id = resolve_id(&to, &pearls)?;

    let from_index = pearls
        .iter()
        .position(|pearl| pearl.id == from_id)
        .ok_or_else(|| anyhow::anyhow!("Pearl '{}' not found", from_id))?;

    let mut updated = pearls[from_index].clone();
    let initial_len = updated.deps.len();
    updated.deps.retain(|dep| dep.target_id != to_id);

    if updated.deps.len() == initial_len {
        anyhow::bail!("No dependency found between {} and {}", from_id, to_id);
    }

    pearls[from_index] = updated.clone();
    IssueGraph::from_pearls(pearls.clone())?;

    updated.validate()?;
    storage.save(&updated)?;

    println!("✓ Unlinked Pearl: {} -> {}", from_id, to_id);

    Ok(())
}

fn resolve_id(id: &str, pearls: &[pearls_core::Pearl]) -> Result<String> {
    if id.starts_with("prl-") && (id.len() == 10 || id.len() == 12) {
        if identity::validate_id_format(id).is_ok() {
            return Ok(id.to_string());
        }
    }

    identity::resolve_partial_id(id, pearls).map_err(|e| anyhow::anyhow!("{}", e))
}
