// Rust guideline compliant 2026-02-06

//! Implementation of the `prl close` command.
//!
//! Closes a Pearl by transitioning it to the closed status, with validation
//! to ensure no blocking dependencies prevent the transition.

use anyhow::Result;
use pearls_core::{Status, Storage};
use std::path::Path;

/// Closes a Pearl by transitioning it to closed status.
///
/// # Arguments
///
/// * `id` - The Pearl ID (full or partial)
///
/// # Returns
///
/// Ok if the Pearl was closed successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - The Pearl is not found
/// - The Pearl has open blocking dependencies
/// - The file cannot be written
pub fn execute(id: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    // Verify .pearls directory exists
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    // Load all Pearls to resolve partial ID and build graph
    let storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let all_pearls = storage.load_all()?;

    // Resolve partial ID
    let full_id = pearls_core::identity::resolve_partial_id(&id, &all_pearls)?;

    // Load the specific Pearl
    let mut pearl = storage.load_by_id(&full_id)?;

    // Build the dependency graph
    let graph = pearls_core::graph::IssueGraph::from_pearls(all_pearls)?;

    // Validate the transition to closed
    pearls_core::fsm::validate_transition(&pearl, Status::Closed, &graph)?;

    // Update status and timestamp
    pearl.status = Status::Closed;
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    pearl.updated_at = now;

    // Validate Pearl
    pearl.validate()?;

    // Save to storage
    storage.save(&pearl)?;

    println!("✓ Closed Pearl: {}", pearl.id);
    println!("  Title: {}", pearl.title);
    println!("  Status: {:?}", pearl.status);

    Ok(())
}
