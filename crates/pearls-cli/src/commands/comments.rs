// Rust guideline compliant 2026-02-07

//! Implementation of `prl comments` commands.
//!
//! Supports adding, listing, and deleting comments attached to a Pearl.

use crate::output_mode::is_json_output;
use anyhow::Result;
use pearls_core::{identity, Comment, Storage};
use std::path::Path;

/// Adds a comment to a Pearl.
///
/// # Arguments
///
/// * `id` - Pearl ID (full or partial)
/// * `body` - Comment text
/// * `author` - Optional comment author override
///
/// # Returns
///
/// Ok if the comment was added.
pub fn add(id: String, body: String, author: Option<String>) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;
    let full_id = identity::resolve_partial_id(&id, &pearls)?;
    let mut pearl = storage.load_by_id(&full_id)?;

    let author = author
        .or_else(default_author)
        .unwrap_or_else(|| "unknown".to_string());
    let comment_id = pearl.add_comment(author, body)?;
    pearl.validate()?;
    storage.save(&pearl)?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "comment_add",
                "id": pearl.id,
                "comment_id": comment_id
            }))?
        );
    } else {
        println!("✓ Added comment {} to {}", comment_id, pearl.id);
    }
    Ok(())
}

/// Lists comments for a Pearl.
///
/// # Arguments
///
/// * `id` - Pearl ID (full or partial)
/// * `json_output` - Whether to emit JSON
///
/// # Returns
///
/// Ok if comments were listed.
pub fn list(id: String, json_output: bool) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;
    let full_id = identity::resolve_partial_id(&id, &pearls)?;
    let pearl = storage.load_by_id(&full_id)?;

    if json_output {
        let payload = serde_json::json!({
            "id": pearl.id,
            "comments": pearl.comments,
            "total": pearl.comments.len()
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("Comments for {} ({})", pearl.id, pearl.title);
    if pearl.comments.is_empty() {
        println!("No comments found.");
        return Ok(());
    }

    for comment in &pearl.comments {
        println!("- {} [{}] {}", comment.id, comment.author, comment.body);
    }

    Ok(())
}

/// Deletes a comment from a Pearl.
///
/// # Arguments
///
/// * `id` - Pearl ID (full or partial)
/// * `comment_id` - Comment ID (full or partial)
///
/// # Returns
///
/// Ok if the comment was deleted.
pub fn delete(id: String, comment_id: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;
    let full_id = identity::resolve_partial_id(&id, &pearls)?;
    let mut pearl = storage.load_by_id(&full_id)?;

    let resolved_comment_id = resolve_comment_id(&comment_id, &pearl.comments)?;
    if !pearl.delete_comment(&resolved_comment_id) {
        anyhow::bail!(
            "Comment '{}' not found for Pearl {}",
            resolved_comment_id,
            pearl.id
        );
    }

    pearl.validate()?;
    storage.save(&pearl)?;
    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "comment_delete",
                "id": pearl.id,
                "comment_id": resolved_comment_id
            }))?
        );
    } else {
        println!(
            "✓ Deleted comment {} from {}",
            resolved_comment_id, pearl.id
        );
    }

    Ok(())
}

fn resolve_comment_id(partial: &str, comments: &[Comment]) -> Result<String> {
    if partial.len() < 3 {
        anyhow::bail!("Comment ID must be at least 3 characters");
    }

    let matches: Vec<&str> = comments
        .iter()
        .filter(|comment| comment.id.starts_with(partial))
        .map(|comment| comment.id.as_str())
        .collect();

    match matches.len() {
        0 => anyhow::bail!("Comment '{}' not found", partial),
        1 => Ok(matches[0].to_string()),
        _ => anyhow::bail!(
            "Ambiguous comment ID '{}': matches {}",
            partial,
            matches.join(", ")
        ),
    }
}

fn default_author() -> Option<String> {
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

    std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
}
