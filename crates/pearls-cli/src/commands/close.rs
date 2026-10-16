// Rust guideline compliant 2026-02-06

//! Implementation of the `prl close` command.
//!
//! Closes a Pearl by transitioning it to the closed status, with validation
//! to ensure no blocking dependencies prevent the transition.

use crate::output_mode::is_json_output;
use pearls_app::service::{resolve_active_task, GraphService, Mutation, TaskRef};

use anyhow::Result;
use pearls_core::Status;
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

    let mut full_id = String::new();
    let result = GraphService::open(".")?.execute(|snapshot| {
        full_id = resolve_active_task(&snapshot.data, &id)?.id.clone();
        Ok(vec![Mutation::SetStatus {
            task: TaskRef::Id(full_id.clone()),
            status: Status::Closed,
        }])
    })?;
    let pearl = result.task(&full_id)?;

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
