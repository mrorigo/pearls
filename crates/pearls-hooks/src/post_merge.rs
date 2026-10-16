// Rust guideline compliant 2026-02-06

//! Post-merge hook implementation.
//!
//! Validates graph integrity after merge operations.

use anyhow::Result;
use pearls_app::service::GraphService;
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
    GraphService::open(repo_path)?.snapshot()?;
    Ok(())
}
