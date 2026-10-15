// Rust guideline compliant 2026-10-15

//! Recoverable, repository-scoped task mutations for embedded Rust callers.
//!
//! Requests use full IDs and ordered local references. Each batch validates the
//! complete active and archived graph under the publication lock. Status changes
//! observe preceding mutations; every intermediate relationship graph must be valid.
//! Opening never discovers or initializes another repository or changes Git hooks.
//! JSONL remains the task format; transaction schema version 1 is owned by core.
//! Unknown storage schemas fail closed. Receipts do not provide guarantees across
//! externally reverted Git history. Receipt expiration is not implemented yet.

mod mutations;
mod types;

pub use pearls_core::repository::{GraphRevision, OperationReceipt, RepositorySnapshot};
pub use types::{
    BatchRequest, CreateTask, Mutation, TaskPatch, TaskRef, API_VERSION, EXTERNAL_DEFINITION,
    EXTERNAL_KEY,
};

use crate::{AppError, Result};
use pearls_core::repository::{OperationRequest, RepositoryStore};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Public application service bound to one explicitly initialized repository.
///
/// Low-level storage writers must not run concurrently with service operations.
/// CLI and MCP adapter migration is a separate implementation slice.
pub struct GraphService {
    store: RepositoryStore,
    pearls_dir: PathBuf,
}

impl GraphService {
    /// Opens an explicit repository root without initialization or discovery.
    ///
    /// # Errors
    /// Returns a typed storage error for missing, unsafe, or unreadable paths.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let store = RepositoryStore::open(root.as_ref())?;
        let pearls_dir = root.as_ref().canonicalize()?.join(".pearls");
        Ok(Self { store, pearls_dir })
    }

    /// Reads and validates a consistent complete active and archived graph.
    ///
    /// Recovery and observed external edits may update transaction metadata.
    ///
    /// # Errors
    /// Returns graph, external-key, or typed storage failures.
    pub fn snapshot(&self) -> Result<RepositorySnapshot> {
        let snapshot = self.store.snapshot()?;
        mutations::validate(&snapshot.data)?;
        Ok(snapshot)
    }

    /// Retrieves the durable receipt for an opaque operation key.
    ///
    /// # Errors
    /// Returns typed storage or recovery failures. An unknown key returns `None`.
    pub fn receipt(&self, operation_key: &str) -> Result<Option<OperationReceipt>> {
        Ok(self.store.receipt(operation_key)?)
    }

    /// Applies a batch atomically, returning its durable receipt or original replay.
    ///
    /// Receipt lookup precedes revision and mutation validation. Canonical identity
    /// includes API version, expected revision, and ordered mutations. JSON object
    /// key order is ignored recursively; array order and explicit defaults matter.
    ///
    /// # Errors
    /// Returns typed concurrency, identity, graph, transition, input, or storage
    /// failures. A rejected batch publishes neither task changes nor its receipt.
    pub fn apply(&self, request: &BatchRequest) -> Result<OperationReceipt> {
        let digest_input = serde_json::json!({
            "api_version": API_VERSION,
            "expected_revision": request.expected_revision,
            "mutations": request.mutations,
        });
        let digest = format!("{:x}", Sha256::digest(canonical_bytes(digest_input)?));
        let operation = OperationRequest {
            operation_key: request.operation_key.clone(),
            request_digest: digest,
            expected_revision: request.expected_revision.clone(),
        };
        self.store.transact(&operation, |data| {
            if request.mutations.is_empty() {
                return Err(AppError::InvalidInput("A batch requires mutations".into()));
            }
            let config = pearls_core::Config::load(&self.pearls_dir)?;
            mutations::apply(data, &request.mutations, &config)
        })
    }
}

fn canonical_bytes(value: Value) -> Result<Vec<u8>> {
    fn sorted(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let ordered: std::collections::BTreeMap<_, _> = map
                    .into_iter()
                    .map(|(key, value)| (key, sorted(value)))
                    .collect();
                Value::Object(ordered.into_iter().collect())
            }
            Value::Array(values) => Value::Array(values.into_iter().map(sorted).collect()),
            other => other,
        }
    }
    serde_json::to_vec(&sorted(value)).map_err(|error| AppError::Core(error.into()))
}
