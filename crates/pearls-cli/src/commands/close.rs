// Rust guideline compliant 2026-02-06

//! Implementation of the `prl close` command.
//!
//! Closes a Pearl by transitioning it to the closed status, with validation
//! to ensure no blocking dependencies prevent the transition.

use crate::output_mode::is_json_output;
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

    // Resolve, validate, and save while holding the storage lock.
    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearl = storage.with_lock(|storage| -> Result<pearls_core::Pearl> {
        let all_pearls = storage.load_all()?;
        let full_id = pearls_core::identity::resolve_partial_id(&id, &all_pearls)?;
        let mut pearl = storage.load_by_id(&full_id)?;

        let graph = pearls_core::graph::IssueGraph::from_pearls(all_pearls)?;
        pearls_core::fsm::validate_transition(&pearl, Status::Closed, &graph)?;

        pearl.status = Status::Closed;
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        pearl.updated_at = now;
        pearl.validate()?;
        storage.save(&pearl)?;
        Ok(pearl)
    })?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "close",
                "pearl": pearl
            }))?
        );
    } else {
        println!("✓ Closed Pearl: {}", pearl.id);
        println!("  Title: {}", pearl.title);
        println!("  Status: {:?}", pearl.status);
    }

    Ok(())
}
