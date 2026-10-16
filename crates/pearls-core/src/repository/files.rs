// Rust guideline compliant 2026-10-15

//! Durable repository filesystem operations and publication fault boundaries.

use super::{Images, RepositoryError, Result, ARCHIVE, ISSUES, VERSION};
use crate::storage::{open_existing_no_follow, reject_managed_path_symlinks, sync_file};
use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// All filesystem access for the repository publication protocol.
pub(super) struct RepositoryFiles {
    directory: PathBuf,
    #[cfg(test)]
    pub(super) remaining_steps: std::cell::Cell<Option<usize>>,
    #[cfg(test)]
    pub(super) steps: std::cell::Cell<usize>,
    #[cfg(test)]
    pub(super) crash: std::cell::Cell<bool>,
}

impl RepositoryFiles {
    pub(super) fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            #[cfg(test)]
            remaining_steps: std::cell::Cell::new(None),
            #[cfg(test)]
            steps: std::cell::Cell::new(0),
            #[cfg(test)]
            crash: std::cell::Cell::new(false),
        }
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.directory.join(name)
    }

    pub(super) fn corrupt(&self, name: &str, reason: impl Into<String>) -> RepositoryError {
        RepositoryError::CorruptStorage {
            path: self.path(name),
            reason: reason.into(),
        }
    }

    pub(super) fn version(&self, name: &str, found: u32) -> Result<()> {
        if found != VERSION {
            return Err(RepositoryError::UnsupportedSchema {
                path: self.path(name),
                found,
            });
        }
        Ok(())
    }

    pub(super) fn parse<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
        bytes: &[u8],
    ) -> Result<T> {
        self.parse_with_legacy(name, bytes, false)
    }

    pub(super) fn parse_with_legacy<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
        bytes: &[u8],
        allow_legacy: bool,
    ) -> Result<T> {
        #[derive(Deserialize)]
        struct Schema {
            schema_version: u32,
        }
        let schema: Schema =
            serde_json::from_slice(bytes).map_err(|error| self.corrupt(name, error.to_string()))?;
        if !(allow_legacy && schema.schema_version == 1) {
            self.version(name, schema.schema_version)?;
        }
        serde_json::from_slice(bytes).map_err(|error| self.corrupt(name, error.to_string()))
    }

    pub(super) fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let path = self.path(name);
        reject_managed_path_symlinks(&path, "Repository file")?;
        match open_existing_no_follow(&path) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                Ok(Some(bytes))
            }
            Err(crate::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn images(&self) -> Result<Images> {
        let image = |name| -> Result<Option<String>> {
            self.read(name)?
                .map(|bytes| {
                    String::from_utf8(bytes).map_err(|error| self.corrupt(name, error.to_string()))
                })
                .transpose()
        };
        Ok(Images {
            issues: image(ISSUES)?,
            archive: image(ARCHIVE)?,
        })
    }

    pub(super) fn replace(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let path = self.path(name);
        reject_managed_path_symlinks(&path, "Repository file")?;
        let mut allocated = None;
        for _ in 0..128 {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let temp = self.path(&format!(".{name}.txn-{}-{counter}.tmp", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => {
                    allocated = Some((temp, file));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let (temp, mut file) = allocated.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Unable to allocate transaction staging file",
            )
        })?;
        let result = (|| {
            file.write_all(bytes)?;
            sync_file(&file)?;
            self.checkpoint()?;
            drop(file);
            reject_managed_path_symlinks(&path, "Repository file")?;
            fs::rename(&temp, &path)?;
            self.checkpoint()?;
            self.sync_directory()
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }

    pub(super) fn remove(&self, name: &str) -> Result<()> {
        let path = self.path(name);
        reject_managed_path_symlinks(&path, "Repository file")?;
        match fs::remove_file(path) {
            Ok(()) => {
                self.checkpoint()?;
                self.sync_directory()
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn sync_directory(&self) -> Result<()> {
        // Unix directory fsync persists renames. Other platforms retain the
        // process-crash contract; power-loss behavior is filesystem dependent.
        #[cfg(unix)]
        sync_file(&fs::File::open(&self.directory)?)?;
        self.checkpoint()
    }

    fn checkpoint(&self) -> Result<()> {
        #[cfg(test)]
        {
            self.steps.set(self.steps.get() + 1);
            if let Some(remaining) = self.remaining_steps.get() {
                self.remaining_steps.set(remaining.checked_sub(1));
                if remaining == 1 {
                    self.remaining_steps.set(None);
                    if self.crash.get() {
                        std::process::exit(86);
                    }
                    return Err(std::io::Error::other("Injected publication interruption").into());
                }
            }
        }
        Ok(())
    }
}
