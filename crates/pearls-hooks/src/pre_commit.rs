// Rust guideline compliant 2026-02-06

//! Pre-commit hook implementation.
//!
//! Validates JSONL syntax, schema compliance, and handles auto-close on commit messages.

use anyhow::Result;
use pearls_core::{IssueGraph, Status, Storage};
use std::path::Path;

/// Runs the pre-commit hook.
///
/// # Arguments
///
/// * `repo_path` - Path to the Git repository
///
/// # Returns
///
/// Ok if validation passes, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - JSONL syntax is invalid
/// - Pearl schema validation fails
/// - Duplicate IDs are detected
pub fn pre_commit_hook(repo_path: &Path) -> Result<()> {
    let pearls_path = repo_path.join(".pearls/issues.jsonl");
    let mut storage = Storage::new(pearls_path)?;
    let pearls = storage.load_all()?;

    for pearl in &pearls {
        pearl.validate()?;
    }

    let mut seen = std::collections::HashSet::new();
    for pearl in &pearls {
        if !seen.insert(&pearl.id) {
            anyhow::bail!("Duplicate Pearl ID detected: {}", pearl.id);
        }
    }

    if let Some(fix_ids) = extract_fix_ids(&repo_path.join(".git/COMMIT_EDITMSG"))? {
        if !fix_ids.is_empty() {
            auto_close(&mut storage, &pearls, &fix_ids)?;
        }
    }

    Ok(())
}

fn extract_fix_ids(path: &Path) -> Result<Option<Vec<String>>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let mut ids = Vec::new();
    let needle = "Fixes (prl-";
    let mut rest = content.as_str();
    while let Some(idx) = rest.find(needle) {
        let start = idx + needle.len();
        if let Some(end) = rest[start..].find(')') {
            let id = &rest[start..start + end];
            ids.push(format!("prl-{}", id));
            rest = &rest[start + end + 1..];
        } else {
            break;
        }
    }
    Ok(Some(ids))
}

fn auto_close(storage: &mut Storage, pearls: &[pearls_core::Pearl], ids: &[String]) -> Result<()> {
    let graph = IssueGraph::from_pearls(pearls.to_vec())?;
    for id in ids {
        let mut pearl = storage
            .load_by_id(id)
            .map_err(|_| anyhow::anyhow!("Pearl '{}' not found for auto-close", id))?;
        if pearl.status == Status::Closed {
            continue;
        }
        pearls_core::fsm::validate_transition(&pearl, Status::Closed, &graph)?;
        pearl.status = Status::Closed;
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        pearl.updated_at = now;
        pearl.validate()?;
        storage.save(&pearl)?;
    }
    Ok(())
}
