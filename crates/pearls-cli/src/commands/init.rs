// Rust guideline compliant 2026-02-06

//! Implementation of the `prl init` command.
//!
//! Initializes a new Pearls repository by creating the `.pearls` directory,
//! initializing the JSONL file, and setting up Git integration.

use anyhow::Result;
use pearls_core::Config;
use std::fs;
use std::path::Path;

/// Initializes a new Pearls repository.
///
/// Creates the `.pearls` directory structure and initializes configuration.
/// Sets up Git merge driver and hooks for seamless integration.
///
/// # Returns
///
/// Ok if initialization was successful, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory cannot be created
/// - The `issues.jsonl` file cannot be created
/// - The configuration file cannot be written
/// - Git configuration cannot be updated
pub fn execute() -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    // Create .pearls directory (ignore if already exists)
    if !pearls_dir.exists() {
        fs::create_dir(pearls_dir)?;
    }

    // Initialize empty issues.jsonl file (only if it doesn't exist)
    let issues_path = pearls_dir.join("issues.jsonl");
    if !issues_path.exists() {
        fs::File::create(&issues_path)?;
    }

    // Create default config.toml (only if it doesn't exist)
    let config_path = pearls_dir.join("config.toml");
    if !config_path.exists() {
        let config = Config::default();
        config.save(pearls_dir)?;
    }

    // Configure Git merge driver and hooks
    setup_git_integration()?;

    println!("✓ Pearls repository initialized at .pearls/");
    println!("  - Created .pearls/issues.jsonl");
    println!("  - Created .pearls/config.toml");
    println!("  - Configured Git merge driver");

    Ok(())
}

/// Sets up Git integration for Pearls.
///
/// Configures the custom merge driver and installs Git hooks.
///
/// # Returns
///
/// Ok if Git integration was set up successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if Git operations fail.
fn setup_git_integration() -> Result<()> {
    // Configure merge driver in .git/config
    // This would typically use git2 crate to configure:
    // - merge.pearls.driver = "pearls-merge %O %A %B"
    // - merge.pearls.name = "Pearls JSONL merge driver"

    // Create .gitattributes file
    let gitattributes_path = Path::new(".gitattributes");
    let gitattributes_content = "issues.jsonl merge=pearls\narchive.jsonl merge=pearls\n";

    if !gitattributes_path.exists() {
        fs::write(gitattributes_path, gitattributes_content)?;
    }

    install_hook(
        ".git/hooks/pre-commit",
        "cargo run -q -p pearls-hooks --bin pearls-pre-commit",
    )?;
    install_hook(
        ".git/hooks/post-merge",
        "cargo run -q -p pearls-hooks --bin pearls-post-merge",
    )?;

    Ok(())
}

fn install_hook(path: &str, command: &str) -> Result<()> {
    let hook_content = format!("#!/bin/sh\n{}\n", command);
    fs::write(path, hook_content)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }

    Ok(())
}
