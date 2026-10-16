// Rust guideline compliant 2026-02-06

//! Implementation of the `prl update` command.
//!
//! Updates an existing Pearl with new field values, validates the changes,
//! and persists them to the JSONL file.

use crate::output_mode::is_json_output;
use pearls_app::service::{resolve_active_task, GraphService, Mutation, TaskRef};

use anyhow::Result;
use std::path::Path;

/// Updates a Pearl with the specified field changes.
///
/// # Arguments
///
/// * `id` - The Pearl ID (full or partial)
/// * `title` - Optional new title
/// * `description` - Optional new description
/// * `priority` - Optional new priority (0-max_priority)
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
#[allow(clippy::too_many_arguments)]
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

    let mut provided_description = description;
    if let Some(path) = description_file {
        provided_description = Some(read_description_from_path(&path)?);
    }
    let status = status
        .as_deref()
        .map(pearls_app::parse_status)
        .transpose()?;
    let mut full_id = String::new();
    let result = GraphService::open(".")?.execute(|snapshot| {
        let pearl = resolve_active_task(&snapshot.data, &id)?;
        full_id = pearl.id.clone();
        let mut labels = pearl.labels.clone();
        for label in &add_labels {
            if !labels.contains(label) {
                labels.push(label.clone());
            }
        }
        labels.retain(|label| !remove_labels.contains(label));
        super::utils::suggest_labels(&snapshot.data.active, &add_labels);
        let mut mutations = vec![Mutation::Update {
            task: TaskRef::Id(full_id.clone()),
            patch: pearls_app::service::TaskPatch {
                title,
                description: provided_description,
                priority,
                labels: Some(labels),
            },
        }];
        if let Some(status) = status {
            mutations.push(Mutation::SetStatus {
                task: TaskRef::Id(full_id.clone()),
                status,
            });
        }
        Ok(mutations)
    })?;
    let pearl = result.task(&full_id)?;

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
        if !pearl.description.is_empty() {
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
