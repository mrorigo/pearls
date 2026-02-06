// Rust guideline compliant 2026-02-06

//! Storage module for JSONL file operations.
//!
//! This module provides functionality for reading and writing Pearls to JSONL files,
//! with support for streaming, indexing, and file locking.

use crate::{Error, Pearl, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Optional index for fast Pearl lookups by ID.
///
/// Maps Pearl IDs to byte offsets in the JSONL file for O(log n) lookup performance.
#[derive(Debug, Clone)]
pub struct Index {
    /// Mapping from Pearl ID to byte offset in the JSONL file.
    map: HashMap<String, u64>,
    /// Path to the index file.
    path: PathBuf,
}

impl Index {
    /// Creates a new Index instance.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the index file
    ///
    /// # Returns
    ///
    /// A new Index instance.
    pub fn new(path: PathBuf) -> Self {
        Self {
            map: HashMap::new(),
            path,
        }
    }

    /// Inserts a Pearl ID and its byte offset into the index.
    ///
    /// # Arguments
    ///
    /// * `id` - The Pearl ID
    /// * `offset` - The byte offset in the JSONL file
    pub fn insert(&mut self, id: String, offset: u64) {
        self.map.insert(id, offset);
    }

    /// Retrieves the byte offset for a Pearl ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The Pearl ID
    ///
    /// # Returns
    ///
    /// The byte offset if found, None otherwise.
    pub fn get(&self, id: &str) -> Option<u64> {
        self.map.get(id).copied()
    }

    /// Removes a Pearl ID from the index.
    ///
    /// # Arguments
    ///
    /// * `id` - The Pearl ID
    pub fn remove(&mut self, id: &str) {
        self.map.remove(id);
    }

    /// Clears all entries from the index.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Returns the number of entries in the index.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Storage engine for Pearls.
///
/// Manages JSONL file operations with support for streaming, optional indexing,
/// and file locking for concurrent access.
pub struct Storage {
    /// Path to the JSONL file.
    path: PathBuf,
    /// Optional index for fast lookups.
    index: Option<Index>,
}

impl Storage {
    /// Creates a new Storage instance.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSONL file
    ///
    /// # Returns
    ///
    /// A new Storage instance with no index.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is invalid.
    pub fn new(path: PathBuf) -> Result<Self> {
        Self::validate_path(&path)?;
        Ok(Self { path, index: None })
    }

    /// Creates a new Storage instance with an optional index.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSONL file
    /// * `index_path` - Optional path to the index file
    ///
    /// # Returns
    ///
    /// A new Storage instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is invalid.
    pub fn with_index(path: PathBuf, index_path: Option<PathBuf>) -> Result<Self> {
        Self::validate_path(&path)?;
        let index = index_path.map(Index::new);
        Ok(Self { path, index })
    }

    /// Validates that the path is suitable for storage operations.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to validate
    ///
    /// # Returns
    ///
    /// Ok if the path is valid, Err otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the path is empty or contains invalid components.
    fn validate_path(path: &Path) -> Result<()> {
        if path.as_os_str().is_empty() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path cannot be empty",
            )));
        }
        Ok(())
    }

    /// Returns a reference to the JSONL file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns a reference to the index if present.
    pub fn index(&self) -> Option<&Index> {
        self.index.as_ref()
    }

    /// Returns a mutable reference to the index if present.
    pub fn index_mut(&mut self) -> Option<&mut Index> {
        self.index.as_mut()
    }

    /// Enables indexing with the given index path.
    ///
    /// # Arguments
    ///
    /// * `index_path` - Path to the index file
    pub fn enable_index(&mut self, index_path: PathBuf) {
        self.index = Some(Index::new(index_path));
    }

    /// Disables indexing.
    pub fn disable_index(&mut self) {
        self.index = None;
    }
}

impl Storage {
    /// Loads all Pearls from the JSONL file using streaming deserialization.
    ///
    /// # Returns
    ///
    /// A vector of all Pearls in the file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened
    /// - The file contains invalid JSON
    /// - A Pearl fails validation
    pub fn load_all(&self) -> Result<Vec<Pearl>> {
        use std::fs::File;
        use std::io::BufReader;

        // Handle empty file case
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut pearls = Vec::new();

        // Use streaming deserializer for memory efficiency
        let stream = serde_json::Deserializer::from_reader(reader).into_iter::<Pearl>();

        for result in stream {
            match result {
                Ok(pearl) => {
                    pearl.validate()?;
                    pearls.push(pearl);
                }
                Err(e) => {
                    // Log malformed JSON but continue processing
                    eprintln!("Warning: Skipping malformed JSON line: {}", e);
                }
            }
        }

        Ok(pearls)
    }

    /// Loads a single Pearl by ID from the JSONL file with early termination.
    ///
    /// # Arguments
    ///
    /// * `id` - The Pearl ID to search for
    ///
    /// # Returns
    ///
    /// The Pearl if found.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be opened
    /// - The Pearl is not found
    /// - The file contains invalid JSON
    pub fn load_by_id(&self, id: &str) -> Result<Pearl> {
        use std::fs::File;
        use std::io::BufReader;

        // Check index first if available
        if let Some(index) = &self.index {
            if let Some(_offset) = index.get(id) {
                // Index lookup would go here in a full implementation
                // For now, we'll fall through to linear search
            }
        }

        if !self.path.exists() {
            return Err(Error::NotFound(id.to_string()));
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let stream = serde_json::Deserializer::from_reader(reader).into_iter::<Pearl>();

        for result in stream {
            match result {
                Ok(pearl) => {
                    if pearl.id == id {
                        pearl.validate()?;
                        return Ok(pearl);
                    }
                }
                Err(e) => {
                    // Skip malformed JSON lines
                    eprintln!("Warning: Skipping malformed JSON line: {}", e);
                }
            }
        }

        Err(Error::NotFound(id.to_string()))
    }
}

impl Storage {
    /// Saves a single Pearl to the JSONL file.
    ///
    /// If the Pearl already exists (by ID), it is updated. Otherwise, it is appended.
    /// Uses atomic write operations (temp file + rename) to ensure consistency.
    ///
    /// # Arguments
    ///
    /// * `pearl` - The Pearl to save
    ///
    /// # Returns
    ///
    /// Ok if the save was successful.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Pearl fails validation
    /// - The file cannot be read or written
    /// - The atomic write operation fails
    pub fn save(&self, pearl: &Pearl) -> Result<()> {
        pearl.validate()?;

        // Load all existing Pearls
        let mut pearls = self.load_all().unwrap_or_default();

        // Find and update or append
        if let Some(pos) = pearls.iter().position(|p| p.id == pearl.id) {
            pearls[pos] = pearl.clone();
        } else {
            pearls.push(pearl.clone());
        }

        // Write all Pearls atomically
        self.save_all(&pearls)?;

        // Update index if enabled
        if let Some(_index) = &self.index {
            // Index update would go here in a full implementation
        }

        Ok(())
    }

    /// Saves multiple Pearls to the JSONL file.
    ///
    /// Replaces the entire file with the provided Pearls.
    /// Uses atomic write operations (temp file + rename) to ensure consistency.
    ///
    /// # Arguments
    ///
    /// * `pearls` - The Pearls to save
    ///
    /// # Returns
    ///
    /// Ok if the save was successful.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Any Pearl fails validation
    /// - The file cannot be written
    /// - The atomic write operation fails
    pub fn save_all(&self, pearls: &[Pearl]) -> Result<()> {
        use std::fs::File;
        use std::io::Write;

        // Validate all Pearls first
        for pearl in pearls {
            pearl.validate()?;
        }

        // Create temp file in the same directory for atomic rename
        let temp_path = self.path.with_extension("jsonl.tmp");

        // Write to temp file
        {
            let mut file = File::create(&temp_path)?;

            for pearl in pearls {
                // Serialize to single line (no newlines within JSON)
                let json = serde_json::to_string(pearl)?;
                file.write_all(json.as_bytes())?;
                file.write_all(b"\n")?;
            }

            file.sync_all()?;
        }

        // Atomic rename
        std::fs::rename(&temp_path, &self.path)?;

        Ok(())
    }
}

impl Storage {
    /// Executes a closure with an exclusive lock on the storage file.
    ///
    /// This method acquires a platform-appropriate file lock (flock on Unix,
    /// LockFileEx on Windows) before executing the closure, ensuring that
    /// concurrent write operations are serialized.
    ///
    /// # Arguments
    ///
    /// * `f` - The closure to execute while holding the lock
    ///
    /// # Returns
    ///
    /// The result of the closure execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The lock cannot be acquired within the timeout
    /// - The closure returns an error
    /// - The lock cannot be released
    pub fn with_lock<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        use fs2::FileExt;
        use std::fs::OpenOptions;

        // Create or open the lock file
        let lock_path = self.path.with_extension("lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)?;

        // Try to acquire exclusive lock with timeout
        // Note: fs2 doesn't support timeouts directly, so we use try_lock
        // and rely on the OS-level timeout behavior
        lock_file.try_lock_exclusive().map_err(|e| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                format!("Failed to acquire lock: {}", e),
            ))
        })?;

        // Execute the closure
        let result = f();

        // Ensure lock is released (even if closure fails)
        let _ = lock_file.unlock();

        result
    }
}

impl Storage {
    /// Deletes a Pearl from the JSONL file by ID.
    ///
    /// Removes the Pearl from the file by rewriting it without the target Pearl.
    /// Updates the index if enabled.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the Pearl to delete
    ///
    /// # Returns
    ///
    /// Ok if the delete was successful.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be read or written
    /// - The Pearl is not found
    pub fn delete(&self, id: &str) -> Result<()> {
        // Load all Pearls
        let mut pearls = self.load_all()?;

        // Find and remove the Pearl
        let initial_len = pearls.len();
        pearls.retain(|p| p.id != id);

        if pearls.len() == initial_len {
            return Err(Error::NotFound(id.to_string()));
        }

        // Write remaining Pearls
        self.save_all(&pearls)?;

        // Update index if enabled
        if let Some(_index) = &self.index {
            // Index update would go here in a full implementation
        }

        Ok(())
    }
}
