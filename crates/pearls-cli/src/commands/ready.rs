// Rust guideline compliant 2026-02-09

//! Implementation of the `prl ready` command.
//!
//! Displays the ready queue: Pearls that are unblocked and ready for work,
//! sorted by priority and recency.

use crate::output_mode::is_json_output;
use anyhow::Result;
use pearls_app::{ready_queue, RepoContext};

/// Displays the ready queue of unblocked Pearls.
///
/// # Arguments
///
/// * `limit` - Optional maximum number of items to display
///
/// # Returns
///
/// Ok if the ready queue was displayed successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - The JSONL file cannot be read
/// - The dependency graph contains cycles
pub fn execute(limit: Option<usize>) -> Result<()> {
    let repo = RepoContext::discover(None)?;
    let storage = repo.open_storage()?;
    let pearls = storage.load_all()?;

    if pearls.is_empty() {
        if is_json_output() {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ready": [],
                    "total": 0,
                    "message": "No Pearls found"
                }))?
            );
        } else {
            println!("No Pearls found. Create one with 'prl create <title>'");
        }
        return Ok(());
    }

    let ready = ready_queue(pearls)?;

    if ready.is_empty() {
        if is_json_output() {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "ready": [],
                    "total": 0,
                    "message": "No Pearls ready for work"
                }))?
            );
        } else {
            println!("No Pearls ready for work.");
            println!("All Pearls are either closed, deferred, or blocked by dependencies.");
        }
        return Ok(());
    }

    let display_ready: Vec<_> = ready.iter().take(limit.unwrap_or(usize::MAX)).collect();

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ready": display_ready,
                "total": ready.len(),
                "returned": display_ready.len()
            }))?
        );
        return Ok(());
    }

    println!("Ready Queue ({} items):", display_ready.len());
    println!();

    for pearl in display_ready {
        let priority_str = format!("P{}", pearl.priority);
        let status_str = format!("{:?}", pearl.status);
        println!(
            "  {} [{}] {} - {}",
            pearl.id, priority_str, status_str, pearl.title
        );
        if !pearl.description.is_empty() {
            let desc = if pearl.description.len() > 60 {
                format!("{}...", &pearl.description[..60])
            } else {
                pearl.description.clone()
            };
            println!("      {}", desc);
        }
        if !pearl.labels.is_empty() {
            println!("      Labels: {}", pearl.labels.join(", "));
        }
    }

    if let Some(l) = limit {
        if ready.len() > l {
            println!();
            println!("  ... and {} more", ready.len() - l);
        }
    }

    Ok(())
}
