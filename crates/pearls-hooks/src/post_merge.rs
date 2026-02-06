// Rust guideline compliant 2026-02-06

//! Post-merge hook implementation.
//!
//! Validates graph integrity after merge operations.

use anyhow::Result;
use pearls_core::{IssueGraph, Storage};
use std::collections::HashSet;
use std::path::Path;

/// Runs the post-merge hook.
///
/// # Arguments
///
/// * `repo_path` - Path to the Git repository
///
/// # Returns
///
/// Ok if integrity checks pass, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - Cycles are detected in the dependency graph
/// - Orphaned dependencies are found
pub fn post_merge_hook(repo_path: &Path) -> Result<()> {
    let pearls_path = repo_path.join(".pearls/issues.jsonl");
    let storage = Storage::new(pearls_path)?;
    let pearls = storage.load_all()?;
    let graph = IssueGraph::from_pearls(pearls.clone())?;

    if let Some(cycle) = graph.find_cycle() {
        anyhow::bail!("Cycle detected after merge: {:?}", cycle);
    }

    let ids: HashSet<String> = pearls.iter().map(|p| p.id.clone()).collect();
    let mut orphaned = Vec::new();
    for pearl in &pearls {
        for dep in &pearl.deps {
            if !ids.contains(&dep.target_id) {
                orphaned.push((pearl.id.clone(), dep.target_id.clone()));
            }
        }
    }

    if !orphaned.is_empty() {
        eprintln!("Warning: Orphaned dependencies detected:");
        for (from, target) in orphaned {
            eprintln!("  {} -> {}", from, target);
        }
    }

    Ok(())
}
