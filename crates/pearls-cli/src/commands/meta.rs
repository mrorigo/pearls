// Rust guideline compliant 2026-02-06

//! Implementation of metadata commands.
//!
//! Provides `prl meta get` and `prl meta set` for Pearl metadata.

use crate::output_mode::is_json_output;
use anyhow::Result;
use pearls_core::{identity, Storage};
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

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;
    let full_id = identity::resolve_partial_id(&id, &pearls)?;
    let pearl = storage.load_by_id(&full_id)?;

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

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;
    let full_id = identity::resolve_partial_id(&id, &pearls)?;
    let mut pearl = storage.load_by_id(&full_id)?;

    let parsed: serde_json::Value = serde_json::from_str(&value)
        .map_err(|e| anyhow::anyhow!("Metadata value must be valid JSON: {}", e))?;

    pearl.metadata.insert(key.clone(), parsed);

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    pearl.updated_at = now;

    pearl.validate()?;
    storage.save(&pearl)?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "meta_set",
                "id": pearl.id,
                "key": key
            }))?
        );
    } else {
        println!("✓ Updated metadata for {}", pearl.id);
    }
    Ok(())
}
