// Rust guideline compliant 2026-02-06

//! Storage module for JSONL file operations.
//!
//! This module provides functionality for reading and writing Pearls to JSONL files,
//! with support for streaming, indexing, and file locking.

use crate::{Error, Pearl, Result};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const INDEX_MAGIC: [u8; 8] = *b"PRLIDX1\0";
const INDEX_VERSION: u8 = 1;
const MAX_INDEX_FILE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_INDEX_ENTRIES: u64 = 1_000_000;
const MAX_INDEX_ID_BYTES: usize = 256;
const MAX_JSONL_LINE_BYTES: usize = 16 * 1024 * 1024;
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);
thread_local! {
    static REENTRANT_LOCK_DEPTHS: RefCell<HashMap<PathBuf, usize>> = RefCell::new(HashMap::new());
}

fn invalid_index_error(message: &str) -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message,
    ))
}

fn read_u32<R: std::io::Read>(reader: &mut R) -> Result<u32> {
    let mut buf = [0u8; 4];
    read_index_exact(reader, &mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_u64<R: std::io::Read>(reader: &mut R) -> Result<u64> {
    let mut buf = [0u8; 8];
    read_index_exact(reader, &mut buf)?;
    Ok(u64::from_le_bytes(buf))
}

fn read_index_exact<R: std::io::Read>(reader: &mut R, buf: &mut [u8]) -> Result<()> {
    reader.read_exact(buf).map_err(|err| {
        if err.kind() == std::io::ErrorKind::UnexpectedEof {
            invalid_index_error("Truncated index file")
        } else {
            Error::Io(err)
        }
    })
}

fn write_u32<W: std::io::Write>(writer: &mut W, value: u32) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u64<W: std::io::Write>(writer: &mut W, value: u64) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn read_bounded_jsonl_line<R: std::io::BufRead>(
    reader: &mut R,
    path: &Path,
    line_idx: usize,
    buffer: &mut Vec<u8>,
) -> Result<usize> {
    buffer.clear();

    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(buffer.len());
        }

        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |pos| pos + 1);

        if buffer.len().saturating_add(take) > MAX_JSONL_LINE_BYTES {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Pearl JSON line in {} at line {} exceeds {} bytes",
                    path.display(),
                    line_idx,
                    MAX_JSONL_LINE_BYTES
                ),
            )));
        }

        let saw_newline = available[..take].last() == Some(&b'\n');
        buffer.extend_from_slice(&available[..take]);
        reader.consume(take);

        if saw_newline {
            return Ok(buffer.len());
        }
    }
}

fn normalize_lock_path(path: PathBuf) -> PathBuf {
    let Some(parent) = path.parent() else {
        return path;
    };
    let Some(file_name) = path.file_name() else {
        return path;
    };

    parent
        .canonicalize()
        .map(|canonical_parent| canonical_parent.join(file_name))
        .unwrap_or(path)
}

fn normalize_managed_path(path: &Path, label: &str) -> Result<PathBuf> {
    reject_managed_path_symlinks(path, label)?;

    let Some(parent) = path.parent() else {
        return Ok(path.to_path_buf());
    };
    let Some(file_name) = path.file_name() else {
        return Ok(path.to_path_buf());
    };

    match parent.canonicalize() {
        Ok(parent) => Ok(parent.join(file_name)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(err) => Err(Error::Io(err)),
    }
}

fn unique_temp_path(path: &Path, marker: &str) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0_u128, |duration| duration.as_nanos());
    let temp_extension = format!("{marker}.tmp.{}.{}.{}", std::process::id(), nanos, counter);
    path.with_extension(temp_extension)
}

#[cfg(unix)]
fn open_existing_no_follow(path: &Path) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(Error::Io)
}

#[cfg(unix)]
fn open_lock_no_follow(path: &Path) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(Error::Io)
}

#[cfg(not(unix))]
fn open_lock_no_follow(path: &Path) -> Result<std::fs::File> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(Error::Io)
}

#[cfg(not(unix))]
fn open_existing_no_follow(path: &Path) -> Result<std::fs::File> {
    std::fs::File::open(path).map_err(Error::Io)
}

fn reject_managed_path_symlinks(path: &Path, label: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        match std::fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("{label} parent cannot be a symlink: {}", parent.display()),
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(Error::Io(err)),
        }
    }

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{label} cannot be a symlink: {}", path.display()),
        ))),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Error::Io(err)),
    }
}

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
        let path = normalize_managed_path(&path, "Index file path").unwrap_or(path);
        Self {
            map: HashMap::new(),
            path,
        }
    }

    fn validate_path(path: &Path) -> Result<()> {
        reject_managed_path_symlinks(path, "Index file path")
    }

    /// Loads an Index from disk, or returns an empty Index if the file does not exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the index file
    ///
    /// # Returns
    ///
    /// An Index populated from disk, or empty if the file is missing.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but is invalid or unreadable.
    pub fn load(path: PathBuf) -> Result<Self> {
        Self::validate_path(&path)?;
        let path = normalize_managed_path(&path, "Index file path")?;

        if !path.exists() {
            return Ok(Self::new(path));
        }

        let file_len = std::fs::metadata(&path)?.len();
        if file_len > MAX_INDEX_FILE_BYTES {
            return Err(invalid_index_error(
                "Index file exceeds maximum supported size",
            ));
        }

        let mut file = open_existing_no_follow(&path)?;

        let mut magic = [0u8; 8];
        read_index_exact(&mut file, &mut magic)?;
        if magic != INDEX_MAGIC {
            return Err(invalid_index_error("Invalid index magic header"));
        }

        let mut version = [0u8; 1];
        read_index_exact(&mut file, &mut version)?;
        if version[0] != INDEX_VERSION {
            return Err(invalid_index_error("Unsupported index version"));
        }

        let count = read_u64(&mut file)?;
        if count > MAX_INDEX_ENTRIES {
            return Err(invalid_index_error(
                "Index entry count exceeds maximum supported size",
            ));
        }
        if count.saturating_mul(13) > file_len {
            return Err(invalid_index_error(
                "Index entry count exceeds file size bounds",
            ));
        }

        let mut map = HashMap::new();
        map.try_reserve(count as usize)
            .map_err(|_| invalid_index_error("Index entry count exceeds available memory"))?;

        for _ in 0..count {
            let id_len = read_u32(&mut file)? as usize;
            if id_len == 0 {
                return Err(invalid_index_error("Index entry has empty ID"));
            }
            if id_len > MAX_INDEX_ID_BYTES {
                return Err(invalid_index_error(
                    "Index entry ID exceeds maximum supported size",
                ));
            }
            let mut id_bytes = vec![0u8; id_len];
            read_index_exact(&mut file, &mut id_bytes)?;
            let id = String::from_utf8(id_bytes)
                .map_err(|_| invalid_index_error("Index entry has invalid UTF-8 ID"))?;
            let offset = read_u64(&mut file)?;
            map.insert(id, offset);
        }

        Ok(Self { map, path })
    }

    /// Writes the Index to disk using an atomic temp file + rename.
    ///
    /// # Returns
    ///
    /// Ok if the index was written successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written or renamed.
    pub fn save(&self) -> Result<()> {
        use std::io::Write;

        Self::validate_path(&self.path)?;
        let path = normalize_managed_path(&self.path, "Index file path")?;

        let temp_path = unique_temp_path(&path, "bin");
        reject_managed_path_symlinks(&temp_path, "Index temp file path")?;
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)?;

        file.write_all(&INDEX_MAGIC)?;
        file.write_all(&[INDEX_VERSION])?;
        write_u64(&mut file, self.map.len() as u64)?;

        let mut entries: Vec<(&String, &u64)> = self.map.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));

        for (id, offset) in entries {
            write_u32(&mut file, id.len() as u32)?;
            file.write_all(id.as_bytes())?;
            write_u64(&mut file, *offset)?;
        }

        file.sync_all()?;
        Self::validate_path(&path)?;
        std::fs::rename(&temp_path, &path)?;

        Ok(())
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

    /// Rebuilds the Index by scanning the JSONL file.
    ///
    /// # Arguments
    ///
    /// * `jsonl_path` - Path to the JSONL file
    ///
    /// # Returns
    ///
    /// Ok if the index was rebuilt successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSONL file cannot be read or contains invalid JSON.
    pub fn rebuild(&mut self, jsonl_path: &Path) -> Result<()> {
        use std::io::BufReader;

        self.map.clear();

        if !jsonl_path.exists() {
            return Ok(());
        }

        Storage::reject_symlink_path(jsonl_path)?;

        let file = open_existing_no_follow(jsonl_path)?;
        let mut reader = BufReader::new(file);
        let mut offset: u64 = 0;
        let mut line = Vec::new();
        let mut line_idx = 0usize;

        loop {
            let bytes = read_bounded_jsonl_line(&mut reader, jsonl_path, line_idx + 1, &mut line)?;
            if bytes == 0 {
                break;
            }
            line_idx += 1;

            let line = std::str::from_utf8(&line).map_err(|err| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Invalid UTF-8 in {} at line {}: {}",
                        jsonl_path.display(),
                        line_idx,
                        err
                    ),
                ))
            })?;
            let line_trimmed = line.trim_end_matches(['\n', '\r']);
            if line_trimmed.is_empty() {
                offset = offset.saturating_add(bytes as u64);
                continue;
            }

            let pearl: Pearl = serde_json::from_str(line_trimmed)?;
            self.map.insert(pearl.id, offset);
            offset = offset.saturating_add(bytes as u64);
        }

        Ok(())
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

    /// Returns an iterator over index entries.
    pub fn entries(&self) -> impl Iterator<Item = (&String, &u64)> {
        self.map.iter()
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
        let path = normalize_managed_path(&path, "Storage file path")?;
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
        let path = normalize_managed_path(&path, "Storage file path")?;
        let mut index = None;

        if let Some(index_path) = index_path {
            let index_exists = index_path.exists();
            let mut needs_save = !index_exists;
            let mut loaded = match Index::load(index_path.clone()) {
                Ok(index) => index,
                Err(err) => {
                    if matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData)
                    {
                        needs_save = true;
                        Index::new(index_path.clone())
                    } else {
                        return Err(err);
                    }
                }
            };

            if path.exists() && loaded.is_empty() {
                loaded.rebuild(&path)?;
                needs_save = true;
            }

            if needs_save {
                loaded.save()?;
            }

            index = Some(loaded);
        }

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
        Self::reject_symlink_path(path)?;
        Ok(())
    }

    fn reject_symlink_path(path: &Path) -> Result<()> {
        reject_managed_path_symlinks(path, "Storage file path")
    }

    fn ensure_not_symlink(&self) -> Result<()> {
        Self::reject_symlink_path(&self.path)
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
    pub fn enable_index(&mut self, index_path: PathBuf) -> Result<()> {
        let mut index = Index::new(index_path);
        if self.path.exists() {
            index.rebuild(&self.path)?;
        }
        index.save()?;
        self.index = Some(index);
        Ok(())
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
        use std::io::BufReader;

        self.ensure_not_symlink()?;

        // Handle empty file case
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = open_existing_no_follow(&self.path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut pearls = Vec::new();
        let mut line = Vec::new();
        let mut line_idx = 0usize;

        loop {
            let bytes = read_bounded_jsonl_line(&mut reader, &self.path, line_idx + 1, &mut line)?;
            if bytes == 0 {
                break;
            }
            line_idx += 1;

            let line = match std::str::from_utf8(&line) {
                Ok(line) => line,
                Err(err) => {
                    eprintln!(
                        "Warning: Skipping invalid UTF-8 in {} at line {}: {}",
                        self.path.display(),
                        line_idx,
                        err
                    );
                    continue;
                }
            };
            let line_trimmed = line.trim_end_matches(['\n', '\r']);
            if line_trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<Pearl>(line_trimmed) {
                Ok(pearl) => {
                    pearl.validate()?;
                    pearls.push(pearl);
                }
                Err(e) => {
                    // Log malformed JSON but continue processing
                    eprintln!(
                        "Warning: Skipping malformed JSON in {} at line {}: {}",
                        self.path.display(),
                        line_idx,
                        e
                    );
                }
            }
        }

        Ok(pearls)
    }

    /// Loads all Pearls from the JSONL file and fails on malformed lines.
    ///
    /// Use this in write paths that depend on a complete ID reservation set.
    /// The permissive [`Storage::load_all`] method is retained for existing
    /// read/list behavior that tolerates partially malformed repositories.
    pub fn load_all_strict(&self) -> Result<Vec<Pearl>> {
        use std::io::BufReader;

        self.ensure_not_symlink()?;

        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = open_existing_no_follow(&self.path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut pearls = Vec::new();
        let mut seen_ids = HashSet::new();
        let mut line = Vec::new();
        let mut line_idx = 0usize;

        loop {
            let bytes = read_bounded_jsonl_line(&mut reader, &self.path, line_idx + 1, &mut line)?;
            if bytes == 0 {
                break;
            }
            line_idx += 1;

            let line = std::str::from_utf8(&line).map_err(|err| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Invalid UTF-8 in {} at line {}: {}",
                        self.path.display(),
                        line_idx,
                        err
                    ),
                ))
            })?;
            let line_trimmed = line.trim_end_matches(['\n', '\r']);
            if line_trimmed.is_empty() {
                continue;
            }

            let pearl: Pearl = serde_json::from_str(line_trimmed).map_err(|err| {
                Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Invalid Pearl JSON in {} at line {}: {}",
                        self.path.display(),
                        line_idx,
                        err
                    ),
                ))
            })?;
            pearl.validate()?;
            if !seen_ids.insert(pearl.id.clone()) {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Duplicate Pearl ID in {} at line {}: {}",
                        self.path.display(),
                        line_idx,
                        pearl.id
                    ),
                )));
            }
            pearls.push(pearl);
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
    pub fn load_by_id(&mut self, id: &str) -> Result<Pearl> {
        use std::io::BufReader;

        self.ensure_not_symlink()?;

        // Check index first if available
        if let Some(index) = self.index.as_mut() {
            if let Some(offset) = index.get(id) {
                if let Ok(pearl) = Self::load_by_offset(&self.path, id, offset) {
                    return Ok(pearl);
                }

                // Index appears out of sync; rebuild and retry once.
                index.rebuild(&self.path)?;
                index.save()?;

                if let Some(rebuilt_offset) = index.get(id) {
                    if let Ok(pearl) = Self::load_by_offset(&self.path, id, rebuilt_offset) {
                        return Ok(pearl);
                    }
                }
            }
        }

        if !self.path.exists() {
            return Err(Error::NotFound(id.to_string()));
        }

        let file = open_existing_no_follow(&self.path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut line = Vec::new();
        let mut line_idx = 0usize;

        loop {
            let bytes = read_bounded_jsonl_line(&mut reader, &self.path, line_idx + 1, &mut line)?;
            if bytes == 0 {
                break;
            }
            line_idx += 1;

            let line = match std::str::from_utf8(&line) {
                Ok(line) => line,
                Err(err) => {
                    eprintln!(
                        "Warning: Skipping invalid UTF-8 in {} at line {}: {}",
                        self.path.display(),
                        line_idx,
                        err
                    );
                    continue;
                }
            };
            let line_trimmed = line.trim_end_matches(['\n', '\r']);
            if line_trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<Pearl>(line_trimmed) {
                Ok(pearl) => {
                    if pearl.id == id {
                        pearl.validate()?;
                        return Ok(pearl);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Skipping malformed JSON in {} at line {}: {}",
                        self.path.display(),
                        line_idx,
                        e
                    );
                }
            }
        }

        Err(Error::NotFound(id.to_string()))
    }

    fn load_by_offset(path: &Path, id: &str, offset: u64) -> Result<Pearl> {
        use std::io::{BufReader, Seek, SeekFrom};

        Self::reject_symlink_path(path)?;

        let mut file = open_existing_no_follow(path)?;
        file.seek(SeekFrom::Start(offset))?;

        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        let bytes = read_bounded_jsonl_line(&mut reader, path, 1, &mut line)?;
        if bytes == 0 {
            return Err(Error::NotFound(id.to_string()));
        }

        let line = std::str::from_utf8(&line).map_err(|err| {
            Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Invalid UTF-8 in {} at offset {}: {}",
                    path.display(),
                    offset,
                    err
                ),
            ))
        })?;
        let line_trimmed = line.trim_end_matches(['\n', '\r']);
        if line_trimmed.is_empty() {
            return Err(Error::NotFound(id.to_string()));
        }

        let pearl: Pearl = serde_json::from_str(line_trimmed)?;
        if pearl.id != id {
            return Err(Error::NotFound(id.to_string()));
        }
        pearl.validate()?;
        Ok(pearl)
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
    pub fn save(&mut self, pearl: &Pearl) -> Result<()> {
        let pearl_to_save = pearl.clone();
        self.with_lock(move |storage| storage.save_unlocked(&pearl_to_save))
    }

    /// Creates a new Pearl with an unused ID.
    ///
    /// Unlike [`Storage::save`], this is create-only: it never updates an
    /// existing Pearl. Candidate IDs are checked while the storage lock is held,
    /// including archived IDs when an archive path is provided.
    pub fn create_new(
        &mut self,
        pearl: &mut Pearl,
        archive_path: Option<&Path>,
        max_attempts: u32,
    ) -> Result<()> {
        self.create_many(std::slice::from_mut(pearl), archive_path, max_attempts)
    }

    /// Creates multiple Pearls with unused IDs in one locked write transaction.
    ///
    /// Unlike [`Storage::save_all`], this is create-only: it never updates
    /// existing Pearls. Candidate IDs are checked while the storage lock is
    /// held, including archived IDs when an archive path is provided.
    pub fn create_many(
        &mut self,
        pearls: &mut [Pearl],
        archive_path: Option<&Path>,
        max_attempts: u32,
    ) -> Result<()> {
        if ReentrantLockGuard::is_held(&normalize_lock_path(self.path.with_extension("lock"))) {
            return Err(Error::InvalidPearl(
                "create_many cannot be called while the storage file lock is already held"
                    .to_string(),
            ));
        }

        let mut pearls_to_save = pearls.to_vec();
        let archive_path = archive_path.map(Path::to_path_buf);

        let repository_lock_path = self.repository_lock_path();
        let saved = Self::with_exclusive_lock_path(repository_lock_path, move || {
            self.with_lock(move |storage| {
                storage
                    .create_many_unlocked(
                        &mut pearls_to_save,
                        archive_path.as_deref(),
                        max_attempts,
                    )
                    .map(|()| pearls_to_save)
            })
        })?;

        for (target, saved) in pearls.iter_mut().zip(saved) {
            target.id = saved.id;
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
    pub fn save_all(&mut self, pearls: &[Pearl]) -> Result<()> {
        let pearls_to_save = pearls.to_vec();
        self.with_lock(move |storage| storage.save_all_unlocked(&pearls_to_save))
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
    pub fn with_lock<F, T, E>(&mut self, f: F) -> std::result::Result<T, E>
    where
        F: FnOnce(&mut Storage) -> std::result::Result<T, E>,
        E: From<Error>,
    {
        let lock_path = self.path.with_extension("lock");
        Self::with_exclusive_lock_path(lock_path, move || f(self))
    }

    /// Executes a closure while holding the repository-level Pearls lock.
    ///
    /// Use this for operations that must coordinate multiple Pearls files, such
    /// as active issues plus archives. Single-file operations should use
    /// [`Storage::with_lock`].
    pub fn with_repository_lock<F, T, E>(&self, f: F) -> std::result::Result<T, E>
    where
        F: FnOnce() -> std::result::Result<T, E>,
        E: From<Error>,
    {
        Self::with_exclusive_lock_path(self.repository_lock_path(), f)
    }

    fn repository_lock_path(&self) -> PathBuf {
        self.path.parent().map_or_else(
            || self.path.with_extension("repository.lock"),
            |parent| parent.join("repository.lock"),
        )
    }

    fn with_exclusive_lock_path<F, T, E>(lock_path: PathBuf, f: F) -> std::result::Result<T, E>
    where
        F: FnOnce() -> std::result::Result<T, E>,
        E: From<Error>,
    {
        use fs2::FileExt;
        use std::time::{Duration, Instant};

        let lock_path = normalize_lock_path(lock_path);
        reject_managed_path_symlinks(&lock_path, "Lock file path").map_err(E::from)?;

        if Self::is_repository_lock_path(&lock_path)
            && ReentrantLockGuard::is_any_other_held(&lock_path)
        {
            return Err(E::from(Error::InvalidPearl(
                "repository lock cannot be acquired while another storage lock is already held"
                    .to_string(),
            )));
        }

        let reentrant_guard = ReentrantLockGuard::acquire(lock_path.clone());
        if reentrant_guard.was_held() {
            return f();
        }

        let lock_file = open_lock_no_follow(&lock_path).map_err(E::from)?;

        // Try to acquire exclusive lock with timeout.
        // fs2 does not support timeouts directly, so we retry with backoff.
        let timeout = Duration::from_secs(5);
        let start = Instant::now();
        loop {
            match lock_file.try_lock_exclusive() {
                Ok(()) => break,
                Err(err) => {
                    if start.elapsed() >= timeout {
                        return Err(E::from(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            format!("Failed to acquire lock: {}", err),
                        ))));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }

        // Execute the closure
        let result = f();

        // Ensure lock is released (even if closure fails)
        let _ = lock_file.unlock();

        result
    }

    fn is_repository_lock_path(path: &Path) -> bool {
        path.file_name()
            .is_some_and(|name| name == "repository.lock")
    }
}

struct ReentrantLockGuard {
    path: PathBuf,
    was_held: bool,
}

impl ReentrantLockGuard {
    fn acquire(path: PathBuf) -> Self {
        let was_held = REENTRANT_LOCK_DEPTHS.with(|locks| {
            let mut locks = locks.borrow_mut();
            if let Some(depth) = locks.get_mut(&path) {
                *depth += 1;
                true
            } else {
                locks.insert(path.clone(), 1);
                false
            }
        });
        Self { path, was_held }
    }

    fn was_held(&self) -> bool {
        self.was_held
    }

    fn is_held(path: &Path) -> bool {
        REENTRANT_LOCK_DEPTHS.with(|locks| locks.borrow().contains_key(path))
    }

    fn is_any_other_held(path: &Path) -> bool {
        REENTRANT_LOCK_DEPTHS.with(|locks| locks.borrow().keys().any(|held| held != path))
    }
}

impl Drop for ReentrantLockGuard {
    fn drop(&mut self) {
        REENTRANT_LOCK_DEPTHS.with(|locks| {
            let mut locks = locks.borrow_mut();
            if let Some(depth) = locks.get_mut(&self.path) {
                *depth -= 1;
                if *depth == 0 {
                    locks.remove(&self.path);
                }
            }
        });
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
    pub fn delete(&mut self, id: &str) -> Result<()> {
        let target_id = id.to_string();
        self.with_lock(move |storage| {
            // Load all Pearls
            let mut pearls = storage.load_all_strict()?;

            // Find and remove the Pearl
            let initial_len = pearls.len();
            pearls.retain(|p| p.id != target_id);

            if pearls.len() == initial_len {
                return Err(Error::NotFound(target_id.clone()));
            }

            // Write remaining Pearls
            storage.save_all_unlocked(&pearls)?;

            Ok(())
        })
    }

    /// Rebuilds the index from the JSONL file if indexing is enabled.
    ///
    /// # Returns
    ///
    /// Ok if the index was rebuilt successfully.
    ///
    /// # Errors
    ///
    /// Returns an error if indexing is disabled or the JSONL file cannot be read.
    pub fn rebuild_index(&mut self) -> Result<()> {
        if let Some(index) = self.index.as_mut() {
            index.rebuild(&self.path)?;
            index.save()?;
            Ok(())
        } else {
            Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Indexing is not enabled",
            )))
        }
    }
}

impl Storage {
    fn create_many_unlocked(
        &mut self,
        new_pearls: &mut [Pearl],
        archive_path: Option<&Path>,
        max_attempts: u32,
    ) -> Result<()> {
        if max_attempts == 0 {
            return Err(Error::InvalidPearl(
                "Unable to allocate unique Pearl ID: candidate space exhausted".to_string(),
            ));
        }

        let mut pearls = self.load_all_strict()?;
        let mut reserved_ids: HashSet<String> = pearls.iter().map(|p| p.id.clone()).collect();

        if let Some(archive_path) = archive_path.filter(|path| path.exists()) {
            let archive_storage = Storage::new(archive_path.to_path_buf())?;
            for archived in archive_storage.load_all_strict()? {
                reserved_ids.insert(archived.id);
            }
        }

        for pearl in new_pearls {
            let mut selected = None;
            for nonce in 0..max_attempts {
                let candidate = crate::identity::generate_id(
                    &pearl.title,
                    &pearl.author,
                    pearl.created_at,
                    nonce,
                );

                if reserved_ids.contains(&candidate) {
                    continue;
                }

                selected = Some(candidate);
                break;
            }

            let Some(candidate) = selected else {
                return Err(Error::InvalidPearl(format!(
                    "Unable to allocate unique Pearl ID after {max_attempts} attempts: candidate space exhausted"
                )));
            };

            pearl.id = candidate.clone();
            pearl.validate()?;
            reserved_ids.insert(candidate);
            pearls.push(pearl.clone());
        }

        self.save_all_unlocked(&pearls)
    }

    fn save_unlocked(&mut self, pearl: &Pearl) -> Result<()> {
        pearl.validate()?;

        // Load all existing Pearls while lock is held.
        let mut pearls = self.load_all_strict()?;

        // Find and update or append.
        if let Some(pos) = pearls.iter().position(|p| p.id == pearl.id) {
            pearls[pos] = pearl.clone();
        } else {
            pearls.push(pearl.clone());
        }

        self.save_all_unlocked(&pearls)
    }

    fn save_all_unlocked(&mut self, pearls: &[Pearl]) -> Result<()> {
        use std::io::Write;

        self.ensure_not_symlink()?;

        // Validate all Pearls first.
        for pearl in pearls {
            pearl.validate()?;
        }

        let (temp_path, mut file) = self.create_temp_file()?;
        let write_result = (|| -> Result<()> {
            for pearl in pearls {
                // Serialize to single line (no newlines within JSON).
                let json = serde_json::to_string(pearl)?;
                file.write_all(json.as_bytes())?;
                file.write_all(b"\n")?;
            }
            file.sync_all()?;
            Ok(())
        })();

        if let Err(err) = write_result {
            let _ = std::fs::remove_file(&temp_path);
            return Err(err);
        }

        std::fs::rename(&temp_path, &self.path)?;

        // Update index if enabled.
        if let Some(index) = self.index.as_mut() {
            index.rebuild(&self.path)?;
            index.save()?;
        }

        Ok(())
    }

    fn create_temp_file(&self) -> Result<(PathBuf, std::fs::File)> {
        const MAX_TEMP_FILE_ATTEMPTS: u64 = 128;

        for _ in 0..MAX_TEMP_FILE_ATTEMPTS {
            let temp_path = self.unique_temp_path();
            match OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
            {
                Ok(file) => return Ok((temp_path, file)),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(Error::Io(err)),
            }
        }

        Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "Failed to allocate unique temp file for storage write",
        )))
    }

    fn unique_temp_path(&self) -> PathBuf {
        unique_temp_path(&self.path, "jsonl")
    }
}
