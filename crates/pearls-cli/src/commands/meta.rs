// Rust guideline compliant 2026-02-06

//! Implementation of metadata commands.
//!
//! Provides `prl meta get` and `prl meta set` for Pearl metadata.

use crate::output_mode::is_json_output;
use pearls_app::service::{resolve_active_task, GraphService, Mutation, TaskRef};

use anyhow::Result;
use std::path::Path;

/// Gets a metadata value for a Pearl.
///
/// # Arguments
///
/// * `id` - The Pearl ID (full or partial)
/// * `key` - Metadata key
///
/// # Returns
///
/// Ok if the value is printed, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - Repository is not initialized
/// - Pearl is not found
/// - Metadata key does not exist
pub fn get(id: String, key: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let snapshot = GraphService::open(".")?.snapshot()?;
    let pearl = resolve_active_task(&snapshot.data, &id)?;

    match pearl.metadata.get(&key) {
        Some(value) => {
            if is_json_output() {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "status": "ok",
                        "action": "meta_get",
                        "id": pearl.id,
                        "key": key,
                        "value": value
                    }))?
                );
            } else {
                println!("{}", value);
            }
            Ok(())
        }
        None => anyhow::bail!("Metadata key '{}' not found for {}", key, pearl.id),
    }
}

/// Sets a metadata value for a Pearl.
///
/// # Arguments
///
/// * `id` - The Pearl ID (full or partial)
/// * `key` - Metadata key
/// * `value` - Metadata value as JSON string
///
/// # Returns
///
/// Ok if the value was set, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - Repository is not initialized
/// - Pearl is not found
/// - Metadata value is not valid JSON
pub fn set(id: String, key: String, value: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let parsed: serde_json::Value = serde_json::from_str(&value)
        .map_err(|e| anyhow::anyhow!("Metadata value must be valid JSON: {}", e))?;
    let mut pearl_id = String::new();
    GraphService::open(".")?.execute(|snapshot| {
        pearl_id = resolve_active_task(&snapshot.data, &id)?.id.clone();
        Ok(vec![Mutation::SetMetadata {
            task: TaskRef::Id(pearl_id.clone()),
            key: key.clone(),
            value: parsed,
        }])
    })?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "meta_set",
                "id": pearl_id,
                "key": key
            }))?
        );
    } else {
        println!("✓ Updated metadata for {}", pearl_id);
    }
    Ok(())
}
