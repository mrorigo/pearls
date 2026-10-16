// Rust guideline compliant 2026-10-15

//! Recoverable, repository-scoped task mutations for embedded Rust callers.
//!
//! Requests use full IDs and ordered local references. Each batch validates the
//! complete active and archived graph under the publication lock. Status changes
//! observe preceding mutations; every intermediate relationship graph must be valid.
//! Opening never discovers or initializes another repository or changes Git hooks.
//! JSONL remains the task format; transaction schema version 2 is owned by core.
//! Unknown storage schemas fail closed. Receipts do not provide guarantees across
//! externally reverted Git history. Expired operation keys remain reserved.

mod adapters;
mod mutations;
mod reads;
mod types;

pub use adapters::{resolve_active_task, AppliedBatch};
pub use pearls_core::repository::{GraphRevision, OperationReceipt, RepositorySnapshot};
pub use reads::{BlockedTask, GraphSnapshot, ScheduleBlockReason, ScheduleResult, TaskSelection};
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
/// CLI, MCP, and hooks use the same application mutations and repository lock.
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
        self.validated_snapshot().map(|(snapshot, _)| snapshot)
    }

    /// Reads requested tasks and their transitive outgoing relationship targets.
    ///
    /// # Arguments
    /// * `selection` - Explicit full IDs; duplicates are ignored and empty is valid.
    ///
    /// # Errors
    /// Returns `NotFound` for unknown IDs, or graph, identity, and storage failures.
    /// The complete repository is validated before selecting tasks.
    pub fn snapshot_for(&self, selection: &TaskSelection) -> Result<GraphSnapshot> {
        let (snapshot, graph) = self.validated_snapshot()?;
        reads::snapshot(snapshot, &graph, selection)
    }

    /// Computes disjoint scheduling sets for explicitly requested tasks.
    ///
    /// Every fact, including blockers outside the selection, comes from the same
    /// repository read. Ready contains only unblocked Open tasks. An empty ready
    /// set does not imply completion.
    ///
    /// # Arguments
    /// * `selection` - Explicit full IDs, including archived tasks when required.
    ///
    /// # Errors
    /// Returns `NotFound` for unknown IDs, or graph, identity, and storage failures.
    pub fn schedule(&self, selection: &TaskSelection) -> Result<ScheduleResult> {
        let (snapshot, graph) = self.validated_snapshot()?;
        reads::schedule(snapshot.revision, &graph, selection)
    }

    fn validated_snapshot(&self) -> Result<(RepositorySnapshot, pearls_core::IssueGraph)> {
        let snapshot = self.store.snapshot()?;
        let graph = mutations::validate(&snapshot.data)?;
        Ok((snapshot, graph))
    }

    /// Retrieves the durable receipt for an opaque operation key.
    ///
    /// # Errors
    /// Returns typed storage or recovery failures, or `OperationExpired` for a
    /// reserved key whose full receipt expired. An unknown key returns `None`.
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
        let operation = operation_request(request)?;
        self.store.transact(&operation, |data| {
            if request.mutations.is_empty() {
                return Err(AppError::InvalidInput("A batch requires mutations".into()));
            }
            let config = pearls_core::Config::load(&self.pearls_dir)?;
            let created = mutations::apply(data, &request.mutations, &config)?;
            Ok(created)
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

/// Computes scheduling facts from a previously captured consistent snapshot.
///
/// This lets adapters derive IDs and legacy response fields from the same read.
/// The complete supplied graph is validated; no storage or process output occurs.
///
/// # Errors
/// Returns graph, external identity, or unknown required-ID failures.
pub fn schedule_snapshot(
    snapshot: &RepositorySnapshot,
    selection: &TaskSelection,
) -> Result<ScheduleResult> {
    let graph = mutations::validate(&snapshot.data)?;
    reads::schedule(snapshot.revision.clone(), &graph, selection)
}

fn operation_request(request: &BatchRequest) -> Result<OperationRequest> {
    let input = serde_json::json!({"api_version":API_VERSION,"expected_revision":request.expected_revision,"mutations":request.mutations});
    Ok(OperationRequest {
        operation_key: request.operation_key.clone(),
        request_digest: format!("{:x}", Sha256::digest(canonical_bytes(input)?)),
        expected_revision: request.expected_revision.clone(),
    })
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn api_v1_canonical_digest_matches_the_compatibility_fixture() {
        let request: BatchRequest = serde_json::from_value(serde_json::json!({
            "operation_key":"fixture-key",
            "expected_revision":"fixture-revision",
            "mutations":[{"kind":"create","local_ref":"fixture","task":{
                "title":"Fixture café","author":"tester","description":"","priority":null,
                "labels":["one","two"],"metadata":{"nested":{"b":2,"a":[2,1]}},"external_key":"fixture:1"
            }}]
        })).unwrap();
        assert_eq!(API_VERSION, 1);
        assert_eq!(
            operation_request(&request).unwrap().request_digest,
            "8d9026488b6cc279fc735e9af7e087c5bf5729c68fa95b4a3b51970cafd55ffb"
        );
        let mut different_key = request;
        different_key.operation_key = "different-key".into();
        assert_eq!(
            operation_request(&different_key).unwrap().request_digest,
            "8d9026488b6cc279fc735e9af7e087c5bf5729c68fa95b4a3b51970cafd55ffb"
        );
    }
}
