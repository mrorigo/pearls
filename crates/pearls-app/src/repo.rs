// Rust guideline compliant 2026-02-09

//! Repository discovery and path management utilities.

use crate::error::{AppError, Result};
use pearls_core::{Config, Storage};
use std::path::{Path, PathBuf};

/// Repository path metadata for a Pearls workspace.
#[derive(Debug, Clone)]
pub struct RepoContext {
    root: PathBuf,
    pearls_dir: PathBuf,
    issues_path: PathBuf,
    archive_path: PathBuf,
    config_path: PathBuf,
}

impl RepoContext {
    /// Discovers a Pearls repository starting from an optional root.
    ///
    /// # Arguments
    ///
    /// * `repo_root` - Optional repository root to pin discovery
    ///
    /// # Returns
    ///
    /// A `RepoContext` with resolved paths for the repository.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The repository root cannot be resolved
    /// - The `.pearls` directory is missing
    pub fn discover(repo_root: Option<&Path>) -> Result<Self> {
        let root = match repo_root {
            Some(root) => root.to_path_buf(),
            None => std::env::current_dir()?,
        };
        let pearls_dir = root.join(".pearls");
        if !pearls_dir.exists() {
            return Err(AppError::RepoNotInitialized {
                path: pearls_dir.clone(),
            });
        }

        Ok(Self {
            root,
            issues_path: pearls_dir.join("issues.jsonl"),
            archive_path: pearls_dir.join("archive.jsonl"),
            config_path: pearls_dir.join("config.toml"),
            pearls_dir,
        })
    }

    /// Returns the repository root path.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    /// Returns the `.pearls` directory path.
    #[must_use]
    pub fn pearls_dir(&self) -> &Path {
        self.pearls_dir.as_path()
    }

    /// Returns the issues JSONL path.
    #[must_use]
    pub fn issues_path(&self) -> &Path {
        self.issues_path.as_path()
    }

    /// Returns the archive JSONL path.
    #[must_use]
    pub fn archive_path(&self) -> &Path {
        self.archive_path.as_path()
    }

    /// Returns the config TOML path.
    #[must_use]
    pub fn config_path(&self) -> &Path {
        self.config_path.as_path()
    }

    /// Opens storage for the active issues file.
    ///
    /// # Returns
    ///
    /// A `Storage` instance for `issues.jsonl`.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage cannot be initialized.
    pub fn open_storage(&self) -> Result<Storage> {
        Ok(Storage::new(self.issues_path.clone())?)
    }

    /// Opens storage for the archive file if it exists.
    ///
    /// # Returns
    ///
    /// `Ok(Some(Storage))` if the archive file exists, `Ok(None)` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the archive file exists but cannot be opened.
    pub fn open_archive_storage(&self) -> Result<Option<Storage>> {
        if self.archive_path.exists() {
            return Ok(Some(Storage::new(self.archive_path.clone())?));
        }
        Ok(None)
    }

    /// Loads repository configuration.
    ///
    /// # Returns
    ///
    /// The loaded `Config`.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration cannot be loaded.
    pub fn load_config(&self) -> Result<Config> {
        Ok(Config::load(self.pearls_dir())?)
    }
}
