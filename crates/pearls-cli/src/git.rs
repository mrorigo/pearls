// Rust guideline compliant 2026-02-12

//! Shared Git helpers for CLI commands.

use anyhow::Result;
use git2::{Repository, StatusOptions};

/// Returns whether the repository working tree is clean.
///
/// A clean working tree means there are no staged, unstaged, or untracked changes.
///
/// # Arguments
///
/// * `repo` - The Git repository to inspect
///
/// # Returns
///
/// `true` when no changes are detected, otherwise `false`.
///
/// # Errors
///
/// Returns an error if repository status cannot be read.
pub(crate) fn working_tree_is_clean(repo: &Repository) -> Result<bool> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut options))?;
    Ok(statuses.is_empty())
}

/// Ensures the repository has no local changes before a sync operation.
///
/// # Arguments
///
/// * `repo` - The Git repository to inspect
///
/// # Returns
///
/// Ok when the repository is clean.
///
/// # Errors
///
/// Returns an error when the working tree contains local changes.
pub(crate) fn ensure_clean_working_tree_for_sync(repo: &Repository) -> Result<()> {
    if !working_tree_is_clean(repo)? {
        anyhow::bail!("Working directory has changes. Commit or stash before sync.");
    }
    Ok(())
}
