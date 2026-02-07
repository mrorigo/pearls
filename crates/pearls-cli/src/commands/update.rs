// Rust guideline compliant 2026-02-06

//! Implementation of the `prl update` command.
//!
//! Updates an existing Pearl with new field values, validates the changes,
//! and persists them to the JSONL file.

use crate::output_mode::is_json_output;
use anyhow::Result;
use pearls_core::Storage;
use std::path::Path;

/// Updates a Pearl with the specified field changes.
///
/// # Arguments
///
/// * `id` - The Pearl ID (full or partial)
/// * `title` - Optional new title
/// * `description` - Optional new description
/// * `priority` - Optional new priority (0-4)
/// * `status` - Optional new status
/// * `add_labels` - Labels to add
/// * `remove_labels` - Labels to remove
///
/// # Returns
///
/// Ok if the Pearl was updated successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - The Pearl is not found
/// - The new values fail validation
/// - The file cannot be written
pub fn execute(
    id: String,
    title: Option<String>,
    description: Option<String>,
    description_file: Option<String>,
    priority: Option<u8>,
    status: Option<String>,
    add_labels: Vec<String>,
    remove_labels: Vec<String>,
) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    // Verify .pearls directory exists
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    // Load all Pearls to resolve partial ID
    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let all_pearls = storage.load_all()?;

    // Resolve partial ID
    let full_id = pearls_core::identity::resolve_partial_id(&id, &all_pearls)?;

    // Load the specific Pearl
    let mut pearl = storage.load_by_id(&full_id)?;

    // Apply updates
    if let Some(new_title) = title {
        pearl.title = new_title;
    }

    let mut provided_description = description;
    if let Some(desc_path) = description_file {
        provided_description = Some(read_description_from_path(&desc_path)?);
    }

    if let Some(new_description) = provided_description {
        enforce_description_limit(&new_description)?;
        pearl.description = new_description;
    }

    if let Some(new_priority) = priority {
        if new_priority > 4 {
            anyhow::bail!("Priority must be 0-4, got {}", new_priority);
        }
        pearl.priority = new_priority;
    }

    if let Some(new_status) = status {
        let new_status_enum = match new_status.to_lowercase().as_str() {
            "open" => pearls_core::Status::Open,
            "in_progress" | "in-progress" => pearls_core::Status::InProgress,
            "blocked" => pearls_core::Status::Blocked,
            "deferred" => pearls_core::Status::Deferred,
            "closed" => pearls_core::Status::Closed,
            _ => anyhow::bail!("Invalid status: {}", new_status),
        };
        let graph = pearls_core::graph::IssueGraph::from_pearls(all_pearls)?;
        pearls_core::fsm::validate_transition(&pearl, new_status_enum, &graph)?;
        pearl.status = new_status_enum;
    }

    // Handle label updates
    if !add_labels.is_empty() {
        for label in &add_labels {
            if !pearl.labels.contains(label) {
                pearl.labels.push(label.clone());
            }
        }
    }

    if !remove_labels.is_empty() {
        for label in &remove_labels {
            pearl.labels.retain(|l| l != label);
        }
    }

    // Update the updated_at timestamp
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    pearl.updated_at = now;

    // Validate Pearl
    pearl.validate()?;

    // Save to storage
    if !add_labels.is_empty() {
        suggest_labels(&storage, &add_labels)?;
    }
    storage.save(&pearl)?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "update",
                "pearl": pearl
            }))?
        );
    } else {
        println!("✓ Updated Pearl: {}", pearl.id);
        println!("  Title: {}", pearl.title);
        if pearl.description.len() > 0 {
            println!("  Description: {}", pearl.description);
        }
        println!("  Priority: {}", pearl.priority);
        println!("  Status: {:?}", pearl.status);
        if !pearl.labels.is_empty() {
            println!("  Labels: {}", pearl.labels.join(", "));
        }
    }

    Ok(())
}

fn read_description_from_path(path: &str) -> Result<String> {
    if path == "-" {
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        return Ok(buffer);
    }

    Ok(std::fs::read_to_string(path)?)
}

fn enforce_description_limit(description: &str) -> Result<()> {
    const MAX_BYTES: usize = 64 * 1024;
    if description.as_bytes().len() > MAX_BYTES {
        anyhow::bail!("Description exceeds 64KB limit");
    }
    Ok(())
}

fn suggest_labels(storage: &Storage, labels: &[String]) -> Result<()> {
    let existing = storage.load_all().unwrap_or_default();
    if existing.is_empty() {
        return Ok(());
    }
    let existing_labels: Vec<String> = existing
        .iter()
        .flat_map(|pearl| pearl.labels.clone())
        .collect();
    if existing_labels.is_empty() {
        return Ok(());
    }
    let lower_existing: std::collections::HashSet<String> = existing_labels
        .iter()
        .map(|label| label.to_lowercase())
        .collect();
    let mut missing = Vec::new();
    for label in labels {
        if !lower_existing.contains(&label.to_lowercase()) {
            missing.push(label.clone());
        }
    }
    if !missing.is_empty() {
        eprintln!(
            "Label suggestions: existing labels include {}",
            existing_labels.join(", ")
        );
    }
    Ok(())
}
