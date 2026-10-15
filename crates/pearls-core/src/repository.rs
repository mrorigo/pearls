// Rust guideline compliant 2026-10-15

//! Recoverable publication of repository task files and operation receipts.
//!
//! Participating readers and writers hold the repository lock and recover any
//! journal before accessing state. A durable journal is the commit decision:
//! recovery rolls it forward before returning a snapshot. Direct `Storage`
//! users and concurrent manual edits do not participate in this protocol.
//! Application graph and transition validation belongs in `pearls-app`.
//!
//! # On-disk protocol
//!
//! `transaction-state.json` stores schema version 1, the monotonic generation,
//! task-file fingerprints, and successful operation receipts. Retain this file
//! alongside task and archive files in Git. Restoring an earlier repository
//! state also restores its generation and receipt history; replay guarantees do
//! not extend across that history rollback.
//!
//! Publication first writes, flushes, and atomically renames a journal containing
//! complete after-images. Once the journal exists, recovery verifies that each
//! destination matches either its before-image or after-image, completes all
//! replacements, invalidates the derived index, and removes the journal last.
//! Each rename/removal is followed by directory synchronization on Unix.
//! Other platforms support process-crash recovery without a power-loss promise.
//! Abandoned staging files have no authority and are ignored on reopening.
//!
//! Both state and journal schemas reject unsupported versions without modifying
//! their contents. Schema migration and receipt expiration are intentionally not
//! performed by ordinary reads or task compaction.

use crate::storage::{reject_managed_path_symlinks, MAX_JSONL_LINE_BYTES};
use crate::{Pearl, Storage};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

const VERSION: u32 = 1;
const ISSUES: &str = "issues.jsonl";
const ARCHIVE: &str = "archive.jsonl";
const STATE: &str = "transaction-state.json";
const JOURNAL: &str = "transaction-journal.json";
mod files;
use files::RepositoryFiles;

type Result<T> = std::result::Result<T, RepositoryError>;

/// Opaque revision identifying an observed repository generation and contents.
///
/// Compare for equality; callers must not interpret the serialized value.
/// Generations are monotonic while transaction state is retained. Restoring an
/// earlier Git state can restore an earlier revision and receipt history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraphRevision(String);

/// Active and archived tasks participating in a repository transaction.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RepositoryData {
    /// Tasks in `issues.jsonl`, in file order.
    pub active: Vec<Pearl>,
    /// Tasks in `archive.jsonl`, in file order.
    pub archived: Vec<Pearl>,
}

/// Consistent task data and its revision.
#[derive(Debug, Clone, PartialEq)]
pub struct RepositorySnapshot {
    /// Active and archived tasks read under the repository lock.
    pub data: RepositoryData,
    /// Revision to provide with a subsequent mutation.
    pub revision: GraphRevision,
}

/// Identity and concurrency precondition of a proposed operation.
#[derive(Debug, Clone)]
pub struct OperationRequest {
    /// Nonempty repository-local idempotency key.
    pub operation_key: String,
    /// Digest of the canonical request, including its expected revision.
    ///
    /// The application layer defines canonicalization. This layer treats the
    /// nonempty digest as opaque and also compares the expected revision.
    pub request_digest: String,
    /// Revision returned by a prior snapshot.
    pub expected_revision: GraphRevision,
}

/// Durable result of a successfully published operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationReceipt {
    /// Repository-local idempotency key.
    pub operation_key: String,
    /// Canonical request digest supplied by the application.
    pub request_digest: String,
    /// Revision before this operation.
    pub before_revision: GraphRevision,
    /// Revision after this operation.
    pub after_revision: GraphRevision,
    /// Batch-local creation references mapped to permanent task IDs.
    pub created_ids: BTreeMap<String, String>,
}

/// Structured failures from repository persistence and concurrency checks.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// The requested repository has no initialized `.pearls` directory.
    #[error("Pearls repository is not initialized at {0}")]
    NotInitialized(PathBuf),
    /// An operation has invalid inputs.
    #[error("Invalid operation: {0}")]
    InvalidInput(String),
    /// Stored data or recovery metadata cannot safely be used.
    #[error("Corrupt storage at {path}: {reason}")]
    CorruptStorage {
        /// Path whose contents failed validation.
        path: PathBuf,
        /// Description of the integrity failure.
        reason: String,
    },
    /// A stored schema is newer or otherwise unsupported.
    #[error("Unsupported repository schema {found} at {path}; supported version is {VERSION}")]
    UnsupportedSchema {
        /// Path containing the unsupported schema.
        path: PathBuf,
        /// Stored schema version.
        found: u32,
    },
    /// The operation's precondition does not match the current revision.
    #[error("Graph revision changed")]
    RevisionConflict {
        /// Revision supplied by the caller.
        expected: GraphRevision,
        /// Current revision.
        actual: GraphRevision,
    },
    /// An operation key has already been used with different request content.
    #[error("Operation key already used: {operation_key}")]
    IdempotencyConflict {
        /// Conflicting repository-local key.
        operation_key: String,
    },
    /// The repository lock could not be acquired within its timeout.
    #[error("Repository lock timed out")]
    LockTimeout,
    /// A proposed task failed core validation.
    #[error("Invalid task: {0}")]
    InvalidTask(crate::Error),
    /// An operating-system operation failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<crate::Error> for RepositoryError {
    fn from(error: crate::Error) -> Self {
        match error {
            crate::Error::Io(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Self::LockTimeout
            }
            crate::Error::Io(error) => Self::Io(error),
            error => Self::InvalidTask(error),
        }
    }
}

/// Repository-scoped transactions over task files and receipts.
///
/// Opening does not initialize a repository or modify hooks or Git settings.
/// The first read establishes transaction metadata for existing JSONL files.
/// Reads can recover publication or record a newly observed external edit.
/// Full receipts are currently retained; expiration policy is separate from
/// task compaction and will be introduced with the application service.
pub struct RepositoryStore {
    files: RepositoryFiles,
}

impl RepositoryStore {
    /// Opens an initialized repository at an explicit path.
    ///
    /// # Arguments
    ///
    /// * `root` - Repository directory containing `.pearls`; no discovery fallback.
    ///
    /// # Errors
    ///
    /// Returns an error for an uninitialized repository, symlinked managed
    /// directory, or inaccessible path. No repository files are created here.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        if root.as_ref().as_os_str().is_empty() {
            return Err(RepositoryError::InvalidInput(
                "Repository path must be explicit and nonempty".to_string(),
            ));
        }
        let directory = root.as_ref().join(".pearls");
        reject_managed_path_symlinks(&directory, "Repository directory")?;
        if !directory.is_dir() {
            return Err(RepositoryError::NotInitialized(directory));
        }
        Ok(Self {
            files: RepositoryFiles::new(directory.canonicalize()?),
        })
    }

    /// Reads a consistent snapshot after recovering any pending publication.
    ///
    /// Valid external task edits advance the persisted generation. Corrupt
    /// files are rejected before new transaction state is published.
    ///
    /// # Errors
    ///
    /// Returns typed lock, storage integrity, schema, and IO failures.
    pub fn snapshot(&self) -> Result<RepositorySnapshot> {
        self.locked(|| {
            let (data, state) = self.load()?;
            Ok(RepositorySnapshot {
                data,
                revision: state.revision(),
            })
        })
    }

    /// Looks up an operation receipt after recovering pending publication.
    ///
    /// # Errors
    ///
    /// Returns typed lock, storage integrity, schema, and IO failures.
    pub fn receipt(&self, operation_key: &str) -> Result<Option<OperationReceipt>> {
        self.locked(|| {
            let (_, state) = self.load()?;
            Ok(state.receipts.get(operation_key).cloned())
        })
    }

    /// Publishes a proposed task set and its receipt in one recoverable operation.
    ///
    /// Receipt replay is checked before the expected revision. For a new key,
    /// `apply` runs once under the repository lock on a private task snapshot and
    /// returns created-ID mappings. Errors discard the proposal. The application
    /// must validate graph and status semantics inside this closure. Its typed
    /// validation errors are returned unchanged; persistence errors convert via
    /// `E: From<RepositoryError>`.
    ///
    /// An IO failure after journaling has an unknown outcome until recovery;
    /// callers recover by looking up or replaying the same operation key.
    /// Observation of external edits may update revision metadata even when the
    /// requested operation subsequently fails its revision precondition.
    ///
    /// # Errors
    ///
    /// Returns input, idempotency, revision, task validation, lock, integrity,
    /// schema, or IO failures. Closure errors publish no requested mutation.
    pub fn transact<F, E>(
        &self,
        request: &OperationRequest,
        apply: F,
    ) -> std::result::Result<OperationReceipt, E>
    where
        F: FnOnce(&mut RepositoryData) -> std::result::Result<BTreeMap<String, String>, E>,
        E: From<RepositoryError>,
    {
        if request.operation_key.is_empty() || request.request_digest.is_empty() {
            return Err(RepositoryError::InvalidInput(
                "Operation key and request digest must be nonempty".to_string(),
            )
            .into());
        }
        self.locked(|| Ok(self.transact_locked(request, apply)))
            .map_err(E::from)?
    }

    fn transact_locked<F, E>(
        &self,
        request: &OperationRequest,
        apply: F,
    ) -> std::result::Result<OperationReceipt, E>
    where
        F: FnOnce(&mut RepositoryData) -> std::result::Result<BTreeMap<String, String>, E>,
        E: From<RepositoryError>,
    {
        let (mut data, mut state) = self.load()?;
        if let Some(receipt) = state.receipts.get(&request.operation_key) {
            if receipt.request_digest != request.request_digest
                || receipt.before_revision != request.expected_revision
            {
                return Err(RepositoryError::IdempotencyConflict {
                    operation_key: request.operation_key.clone(),
                }
                .into());
            }
            return Ok(receipt.clone());
        }
        let before_revision = state.revision();
        if request.expected_revision != before_revision {
            return Err(RepositoryError::RevisionConflict {
                expected: request.expected_revision.clone(),
                actual: before_revision,
            }
            .into());
        }
        let before_files = state.files.clone();
        let before_state = fingerprint(self.files.read(STATE)?.as_deref());
        let created_ids = apply(&mut data)?;
        let images = self.encode(&data)?;
        state.advance()?;
        state.files = images.fingerprints();
        let receipt = OperationReceipt {
            operation_key: request.operation_key.clone(),
            request_digest: request.request_digest.clone(),
            before_revision,
            after_revision: state.revision(),
            created_ids,
        };
        state
            .receipts
            .insert(request.operation_key.clone(), receipt.clone());
        self.publish(before_files, before_state, images, state)?;
        Ok(receipt)
    }

    fn locked<T>(&self, work: impl FnOnce() -> Result<T>) -> Result<T> {
        let storage = Storage::new(self.files.path(ISSUES))?;
        storage.with_repository_lock(work)
    }

    fn load(&self) -> Result<(RepositoryData, State)> {
        self.recover()?;
        let images = self.files.images()?;
        let data = self.decode(&images)?;
        let fingerprints = images.fingerprints();
        let stored = self.files.read(STATE)?;
        let mut state = match stored.as_deref() {
            Some(bytes) => {
                let state: State = self.files.parse(STATE, bytes)?;
                self.files.version(STATE, state.schema_version)?;
                self.validate_state(&state)?;
                state
            }
            None => State {
                schema_version: VERSION,
                generation: 0,
                files: fingerprints.clone(),
                receipts: BTreeMap::new(),
            },
        };
        if state.files != fingerprints {
            state.advance()?;
            state.files = fingerprints.clone();
            self.publish(
                fingerprints,
                fingerprint(stored.as_deref()),
                images,
                state.clone(),
            )?;
        } else if stored.is_none() {
            self.publish(fingerprints, None, images, state.clone())?;
        }
        Ok((data, state))
    }

    fn encode(&self, data: &RepositoryData) -> Result<Images> {
        self.validate(data)?;
        let encode = |pearls: &[Pearl]| -> Result<String> {
            let mut output = String::new();
            for pearl in pearls {
                let line = serde_json::to_string(pearl)
                    .map_err(|error| RepositoryError::InvalidInput(error.to_string()))?;
                if line.len().saturating_add(1) > MAX_JSONL_LINE_BYTES {
                    return Err(RepositoryError::InvalidInput(
                        "Serialized task exceeds the JSONL line limit".to_string(),
                    ));
                }
                output.push_str(&line);
                output.push('\n');
            }
            Ok(output)
        };
        Ok(Images {
            issues: Some(encode(&data.active)?),
            archive: Some(encode(&data.archived)?),
        })
    }

    fn decode(&self, images: &Images) -> Result<RepositoryData> {
        let decode = |name: &str, image: &Option<String>| -> Result<Vec<Pearl>> {
            let mut pearls = Vec::new();
            for (index, line) in image
                .as_deref()
                .unwrap_or_default()
                .split_inclusive('\n')
                .enumerate()
            {
                if line.len() > MAX_JSONL_LINE_BYTES {
                    return Err(self
                        .files
                        .corrupt(name, format!("Line {} exceeds JSONL limit", index + 1)));
                }
                let line = line.trim_end_matches(['\n', '\r']);
                if line.is_empty() {
                    continue;
                }
                let pearl: Pearl = serde_json::from_str(line).map_err(|error| {
                    self.files
                        .corrupt(name, format!("Line {}: {error}", index + 1))
                })?;
                pearl.validate().map_err(|error| {
                    self.files
                        .corrupt(name, format!("Line {}: {error}", index + 1))
                })?;
                pearls.push(pearl);
            }
            Ok(pearls)
        };
        let data = RepositoryData {
            active: decode(ISSUES, &images.issues)?,
            archived: decode(ARCHIVE, &images.archive)?,
        };
        self.validate(&data)
            .map_err(|error| self.files.corrupt(ISSUES, error.to_string()))?;
        Ok(data)
    }

    fn validate(&self, data: &RepositoryData) -> Result<()> {
        let mut ids = HashSet::new();
        for pearl in data.active.iter().chain(&data.archived) {
            pearl.validate().map_err(RepositoryError::InvalidTask)?;
            if !ids.insert(&pearl.id) {
                return Err(RepositoryError::InvalidInput(format!(
                    "Duplicate task ID across active/archive: {}",
                    pearl.id
                )));
            }
        }
        Ok(())
    }

    fn publish(
        &self,
        before: Fingerprints,
        before_state: Option<String>,
        after: Images,
        state: State,
    ) -> Result<()> {
        if self.files.read(JOURNAL)?.is_some() {
            return Err(self
                .files
                .corrupt(JOURNAL, "Pending publication must be recovered first"));
        }
        if self.files.images()?.fingerprints() != before
            || fingerprint(self.files.read(STATE)?.as_deref()) != before_state
        {
            return Err(self
                .files
                .corrupt(STATE, "External edit during transaction"));
        }
        let journal = Journal {
            schema_version: VERSION,
            before,
            before_state,
            after,
            state,
        };
        let bytes = serde_json::to_vec(&journal)
            .map_err(|error| RepositoryError::InvalidInput(error.to_string()))?;
        self.files.replace(JOURNAL, &bytes)?;
        self.recover()
    }

    fn recover(&self) -> Result<()> {
        let Some(bytes) = self.files.read(JOURNAL)? else {
            return Ok(());
        };
        let journal: Journal = self.files.parse(JOURNAL, &bytes)?;
        self.files.version(JOURNAL, journal.schema_version)?;
        self.files.version(STATE, journal.state.schema_version)?;
        self.validate_state(&journal.state)?;
        self.decode(&journal.after)?;
        if journal.state.files != journal.after.fingerprints() {
            return Err(self
                .files
                .corrupt(JOURNAL, "Task fingerprints do not match journal"));
        }
        let state = serde_json::to_vec(&journal.state)
            .map_err(|error| self.files.corrupt(JOURNAL, error.to_string()))?;
        let targets = [
            (
                ISSUES,
                journal.before.issues,
                journal.after.issues.map(String::into_bytes),
            ),
            (
                ARCHIVE,
                journal.before.archive,
                journal.after.archive.map(String::into_bytes),
            ),
            (STATE, journal.before_state, Some(state)),
        ];
        // Check every destination before changing any of them. An external edit
        // conflicting with recovery must never be silently overwritten.
        for (name, before, after) in &targets {
            let actual = fingerprint(self.files.read(name)?.as_deref());
            if actual != *before && actual != fingerprint(after.as_deref()) {
                return Err(self
                    .files
                    .corrupt(name, "External edit conflicts with pending publication"));
            }
        }
        for (name, before, after) in &targets {
            let actual = self.files.read(name)?;
            if actual.as_deref() != after.as_deref() {
                if fingerprint(actual.as_deref()) != *before {
                    return Err(self.files.corrupt(name, "External edit during recovery"));
                }
                match after.as_deref() {
                    Some(bytes) => self.files.replace(name, bytes)?,
                    None => self.files.remove(name)?,
                }
            }
        }
        for (name, _, after) in &targets {
            if self.files.read(name)?.as_deref() != after.as_deref() {
                return Err(self.files.corrupt(name, "External edit during recovery"));
            }
        }
        self.files.remove("index.bin")?;
        self.files.remove(JOURNAL)
    }

    fn validate_state(&self, state: &State) -> Result<()> {
        for digest in [&state.files.issues, &state.files.archive]
            .into_iter()
            .flatten()
        {
            if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(self.files.corrupt(STATE, "Invalid task fingerprint"));
            }
        }
        for (key, receipt) in &state.receipts {
            let generation = |revision: &GraphRevision| {
                revision
                    .0
                    .split_once(':')
                    .and_then(|(value, _)| value.parse::<u64>().ok())
            };
            let before = generation(&receipt.before_revision);
            let after = generation(&receipt.after_revision);
            if key.is_empty()
                || *key != receipt.operation_key
                || receipt.request_digest.is_empty()
                || before.and_then(|value| value.checked_add(1)) != after
                || after.is_none_or(|value| value > state.generation)
                || (after == Some(state.generation) && receipt.after_revision != state.revision())
            {
                return Err(self.files.corrupt(STATE, "Invalid operation receipt"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Fingerprints {
    issues: Option<String>,
    archive: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Images {
    issues: Option<String>,
    archive: Option<String>,
}

impl Images {
    fn fingerprints(&self) -> Fingerprints {
        Fingerprints {
            issues: fingerprint(self.issues.as_deref().map(str::as_bytes)),
            archive: fingerprint(self.archive.as_deref().map(str::as_bytes)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    schema_version: u32,
    generation: u64,
    files: Fingerprints,
    receipts: BTreeMap<String, OperationReceipt>,
}

impl State {
    fn advance(&mut self) -> Result<()> {
        self.generation = self.generation.checked_add(1).ok_or_else(|| {
            RepositoryError::InvalidInput("Repository generation exhausted".to_string())
        })?;
        Ok(())
    }

    fn revision(&self) -> GraphRevision {
        // Missing and empty files are deliberately distinct.
        let mut hash = Sha256::new();
        hash.update(b"pearls-revision-v1\0");
        for digest in [&self.files.issues, &self.files.archive] {
            match digest {
                Some(digest) => {
                    hash.update([1]);
                    hash.update(digest.as_bytes());
                }
                None => hash.update([0]),
            }
        }
        GraphRevision(format!("{}:{:x}", self.generation, hash.finalize()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    schema_version: u32,
    before: Fingerprints,
    before_state: Option<String>,
    after: Images,
    state: State,
}

fn fingerprint(bytes: Option<&[u8]>) -> Option<String> {
    bytes.map(|bytes| format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests;
