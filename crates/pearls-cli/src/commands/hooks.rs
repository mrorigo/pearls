// Rust guideline compliant 2026-02-06

//! Hook command wrappers for invoking Pearls Git hooks from the CLI.

use anyhow::Result;

/// Runs the requested hook action.
///
/// # Arguments
///
/// * `action` - Hook action name ("pre-commit" or "post-merge")
///
/// # Returns
///
/// Ok if the hook succeeds, Err otherwise.
///
/// # Errors
///
/// Returns an error if the action is invalid or the hook fails.
pub fn execute(action: HookAction) -> Result<()> {
    let repo_path = std::env::current_dir()?;
    match action {
        HookAction::PreCommit => pearls_hooks::pre_commit_hook(&repo_path),
        HookAction::PostMerge => pearls_hooks::post_merge_hook(&repo_path),
    }
}

/// Supported hook actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::Subcommand)]
pub enum HookAction {
    /// Run the pre-commit hook
    PreCommit,
    /// Run the post-merge hook
    PostMerge,
}
