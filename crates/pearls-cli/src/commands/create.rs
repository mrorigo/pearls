// Rust guideline compliant 2026-02-06

//! Implementation of the `prl create` command.
//!
//! Creates a new Pearl with the specified title and optional fields,
//! generates a hash-based ID, and appends it to the JSONL file.

use crate::output_mode::is_json_output;
use pearls_app::service::{GraphService, Mutation};

use anyhow::Result;
use pearls_core::Config;
use std::path::Path;

/// Creates a new Pearl with the specified parameters.
///
/// # Arguments
///
/// * `title` - The Pearl title
/// * `description` - Optional description
/// * `priority` - Optional priority (0-max_priority)
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

    let mut provided_description = description;
    if let Some(path) = description_file {
        provided_description = Some(read_description_from_path(&path)?);
    }
    let service = GraphService::open(".")?;
    let result = service.execute(|snapshot| {
        super::utils::suggest_labels(&snapshot.data.active, &labels);
        Ok(vec![Mutation::Create {
            local_ref: "task".into(),
            task: pearls_app::service::CreateTask {
                title,
                author,
                description: provided_description.unwrap_or_default(),
                priority,
                labels,
                metadata: Default::default(),
                external_key: None,
            },
        }])
    })?;
    let pearl = result.task(&result.receipt.created_ids["task"])?;

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
        if pearl.priority != config.default_priority {
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
        .args(["config", "user.name"])
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
