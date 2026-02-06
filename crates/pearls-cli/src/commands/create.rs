// Rust guideline compliant 2026-02-06

//! Implementation of the `prl create` command.
//!
//! Creates a new Pearl with the specified title and optional fields,
//! generates a hash-based ID, and appends it to the JSONL file.

use anyhow::Result;
use pearls_core::{Pearl, Storage};
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
    let author = author.or_else(get_default_author).unwrap_or_else(|| "unknown".to_string());

    // Create new Pearl
    let mut pearl = Pearl::new(title, author);

    // Apply optional fields
    if let Some(desc) = description {
        pearl.description = desc;
    }

    if let Some(p) = priority {
        if p > 4 {
            anyhow::bail!("Priority must be 0-4, got {}", p);
        }
        pearl.priority = p;
    }

    if !labels.is_empty() {
        pearl.labels = labels;
    }

    // Validate Pearl
    pearl.validate()?;

    // Save to storage
    let storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    storage.save(&pearl)?;

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
    std::env::var("USER").ok().or_else(|| std::env::var("USERNAME").ok())
}
