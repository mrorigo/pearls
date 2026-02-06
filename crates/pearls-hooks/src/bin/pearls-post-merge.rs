// Rust guideline compliant 2026-02-06

//! CLI entry point for Pearls post-merge hook.

fn main() -> anyhow::Result<()> {
    let repo_path = std::env::current_dir()?;
    pearls_hooks::post_merge_hook(&repo_path)?;
    Ok(())
}
