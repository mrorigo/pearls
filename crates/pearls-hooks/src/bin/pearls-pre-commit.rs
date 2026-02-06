// Rust guideline compliant 2026-02-06

//! CLI entry point for Pearls pre-commit hook.

fn main() -> anyhow::Result<()> {
    let repo_path = std::env::current_dir()?;
    pearls_hooks::pre_commit_hook(&repo_path)?;
    Ok(())
}
