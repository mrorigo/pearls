// Rust guideline compliant 2026-02-06

//! Pre-commit hook implementation.
//!
//! Validates JSONL syntax, schema compliance, and handles auto-close on commit messages.

use anyhow::Result;
use pearls_app::service::{GraphService, Mutation, TaskRef};
use pearls_core::Status;
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
    let service = GraphService::open(repo_path)?;
    let snapshot = service.snapshot()?;
    let Some(ids) = extract_fix_ids(&repo_path.join(".git/COMMIT_EDITMSG"))? else {
        return Ok(());
    };
    let mut requested = std::collections::HashSet::new();
    for id in ids {
        let task = snapshot
            .data
            .active
            .iter()
            .find(|task| task.id == id)
            .ok_or_else(|| pearls_app::AppError::TaskNotFound { id: id.clone() })?;
        if task.status != Status::Closed {
            requested.insert(id);
        }
    }
    if requested.is_empty() {
        return Ok(());
    }
    let graph = pearls_core::IssueGraph::from_pearls_strict(
        snapshot
            .data
            .active
            .into_iter()
            .chain(snapshot.data.archived)
            .collect(),
    )?;
    let mutations = graph
        .topological_sort()?
        .into_iter()
        .filter(|id| requested.contains(id))
        .map(|id| Mutation::SetStatus {
            task: TaskRef::Id(id),
            status: Status::Closed,
        })
        .collect();
    service.execute(|_| Ok(mutations))?;
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
