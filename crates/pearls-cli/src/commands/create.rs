// Rust guideline compliant 2026-02-06

//! Implementation of the `prl create` command.
//!
//! Creates a new Pearl with the specified title and optional fields,
//! generates a hash-based ID, and appends it to the JSONL file.

use crate::output_mode::is_json_output;
use anyhow::Result;
use pearls_core::{Config, Pearl, Storage};
use std::path::Path;

/// Creates a new Pearl with the specified parameters.
///
/// # Arguments
///
/// * `title` - The Pearl title
/// * `description` - Optional description
/// * `priority` - Optional priority (0-4)
/// * `labels` - Optional labels
/// * `author` - Optional author (defaults to Git config or system username)
///
/// # Returns
///
/// Ok if the Pearl was created successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - The `issues.jsonl` file cannot be accessed
/// - The Pearl fails validation
/// - The file cannot be written
pub fn execute(
    title: String,
    description: Option<String>,
    description_file: Option<String>,
    priority: Option<u8>,
    labels: Vec<String>,
    author: Option<String>,
) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    // Verify .pearls directory exists
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    // Determine author
    let author = author
        .or_else(get_default_author)
        .unwrap_or_else(|| "unknown".to_string());

    let config = Config::load(pearls_dir)?;

    // Create new Pearl
    let mut pearl = Pearl::new(title, author);

    // Apply optional fields
    let mut provided_description = description;
    if let Some(desc_path) = description_file {
        provided_description = Some(read_description_from_path(&desc_path)?);
    }

    if let Some(desc) = provided_description {
        enforce_description_limit(&desc)?;
        pearl.description = desc;
    }

    if let Some(p) = priority {
        if p > 4 {
            anyhow::bail!("Priority must be 0-4, got {}", p);
        }
        pearl.priority = p;
    } else {
        pearl.priority = config.default_priority;
    }

    if !labels.is_empty() {
        pearl.labels = labels.clone();
    }

    // Validate Pearl
    pearl.validate()?;

    // Save to storage
    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    if !labels.is_empty() {
        suggest_labels(&storage, &labels)?;
    }
    storage.save(&pearl)?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "create",
                "pearl": pearl
            }))?
        );
    } else {
        println!("✓ Created Pearl: {}", pearl.id);
        println!("  Title: {}", pearl.title);
        if !pearl.description.is_empty() {
            println!("  Description: {}", pearl.description);
        }
        if pearl.priority != 2 {
            println!("  Priority: {}", pearl.priority);
        }
        if !pearl.labels.is_empty() {
            println!("  Labels: {}", pearl.labels.join(", "));
        }
    }

    Ok(())
}

/// Gets the default author from Git config or system username.
///
/// # Returns
///
/// The author name if available, None otherwise.
fn get_default_author() -> Option<String> {
    // Try to get from Git config
    if let Ok(output) = std::process::Command::new("git")
        .args(&["config", "user.name"])
        .output()
    {
        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    // Fall back to system username
    std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
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
