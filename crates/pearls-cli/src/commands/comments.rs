// Rust guideline compliant 2026-02-07

//! Implementation of `prl comments` commands.
//!
//! Supports adding, listing, and deleting comments attached to a Pearl.

use crate::output_mode::is_json_output;
use pearls_app::service::{resolve_active_task, GraphService, Mutation, TaskRef};

use anyhow::Result;
use pearls_core::Comment;
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

    let author = author
        .or_else(default_author)
        .unwrap_or_else(|| "unknown".into());
    let mut pearl_id = String::new();
    let result = GraphService::open(".")?.execute(|snapshot| {
        pearl_id = resolve_active_task(&snapshot.data, &id)?.id.clone();
        Ok(vec![Mutation::AddComment {
            task: TaskRef::Id(pearl_id.clone()),
            author,
            body,
        }])
    })?;
    let pearl = result.task(&pearl_id)?;
    let comment_id = pearl
        .comments
        .last()
        .ok_or_else(|| anyhow::anyhow!("Added comment is missing"))?
        .id
        .clone();

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "comment_add",
                "id": pearl_id,
                "comment_id": comment_id
            }))?
        );
    } else {
        println!("✓ Added comment {} to {}", comment_id, pearl_id);
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

    let snapshot = GraphService::open(".")?.snapshot()?;
    let pearl = resolve_active_task(&snapshot.data, &id)?;

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

    let mut pearl_id = String::new();
    let mut resolved_comment_id = String::new();
    GraphService::open(".")?.execute(|snapshot| {
        let pearl = resolve_active_task(&snapshot.data, &id)?;
        pearl_id = pearl.id.clone();
        resolved_comment_id = resolve_comment_id(&comment_id, &pearl.comments)
            .map_err(|error| pearls_app::AppError::InvalidInput(error.to_string()))?;
        Ok(vec![Mutation::DeleteComment {
            task: TaskRef::Id(pearl_id.clone()),
            comment_id: resolved_comment_id.clone(),
        }])
    })?;
    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "comment_delete",
                "id": pearl_id,
                "comment_id": resolved_comment_id
            }))?
        );
    } else {
        println!(
            "✓ Deleted comment {} from {}",
            resolved_comment_id, pearl_id
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
