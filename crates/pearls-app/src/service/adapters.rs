// Rust guideline compliant 2026-10-15

//! One-shot adapter requests and explicit maintenance operations.

use super::{
    mutations, BatchRequest, GraphService, Mutation, OperationReceipt, RepositorySnapshot,
    EXTERNAL_KEY,
};
use crate::{AppError, Result};
use pearls_core::{repository::RepositoryData, Pearl, Status};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// A one-shot adapter operation's receipt and exact committed after-images.
#[derive(Debug)]
pub struct AppliedBatch {
    /// Durable operation receipt, including created IDs.
    pub receipt: OperationReceipt,
    /// Task data captured inside the publication transaction, before later writers.
    pub data: RepositoryData,
}

fn operation_key() -> Result<String> {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::InvalidInput(error.to_string()))?;
    Ok(format!(
        "pearls:adapter:{}:{}:{}",
        std::process::id(),
        time.as_nanos(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

impl GraphService {
    /// Prepares and applies a one-shot operation for legacy CLI, MCP, or hook adapters.
    ///
    /// The callback resolves partial IDs and prepares typed mutations from one
    /// validated snapshot while holding the publication lock. Concurrent adapter
    /// calls serialize. The callback must not call storage or other services. Responses
    /// contain the exact committed data, avoiding a second read after publication.
    /// Embedded callers needing durable replay should use `apply` with their own key.
    ///
    /// # Errors
    /// Returns preparation, validation, revision, or storage failures.
    pub fn execute<F>(&self, prepare: F) -> Result<AppliedBatch>
    where
        F: FnOnce(&RepositorySnapshot) -> Result<Vec<Mutation>>,
    {
        let key = operation_key()?;
        let mut after = None;
        let receipt = self.store.transact_prepared(|mut snapshot| {
            mutations::validate(&snapshot.data)?;
            let mutations = prepare(&snapshot)?;
            if mutations.is_empty() {
                return Err(AppError::InvalidInput(
                    "An operation requires mutations".into(),
                ));
            }
            let request = BatchRequest {
                operation_key: key,
                expected_revision: snapshot.revision,
                mutations,
            };
            let operation = super::operation_request(&request)?;
            let config = pearls_core::Config::load(&self.pearls_dir)?;
            let created_ids = mutations::apply(&mut snapshot.data, &request.mutations, &config)?;
            after = Some(snapshot.data.clone());
            Ok(pearls_core::repository::PreparedOperation {
                request: operation,
                data: snapshot.data,
                created_ids,
            })
        })?;
        let data = after.ok_or_else(|| {
            AppError::InvalidInput("Adapter operation identity was already used".into())
        })?;
        Ok(AppliedBatch { receipt, data })
    }

    /// Initializes task files in an explicitly created `.pearls` directory.
    ///
    /// Existing task records and unexpired receipts are preserved. Schema v1 is
    /// explicitly migrated to v2 with a new replay window for legacy receipts.
    /// This does not create the
    /// directory, configure Git, or install hooks; the initialization adapter owns
    /// those explicit actions.
    ///
    /// # Errors
    /// Returns graph, identity, storage, or recovery failures.
    pub fn initialize_files(&self) -> Result<OperationReceipt> {
        self.store
            .migrate_and_repair(&operation_key()?, "initialize:v1", |active, archived| {
                let data = RepositoryData {
                    active: parse(active, &self.pearls_dir.join("issues.jsonl"))?,
                    archived: parse(archived, &self.pearls_dir.join("archive.jsonl"))?,
                };
                mutations::validate(&data)?;
                Ok(data)
            })
    }

    /// Archives eligible Closed tasks atomically with its operation receipt.
    ///
    /// Identical legacy duplicates without external keys are reconciled explicitly.
    /// Conflicting records or duplicate external identities fail closed. Receipts
    /// survive compaction. The cutoff is inclusive, preserving CLI behavior.
    ///
    /// # Errors
    /// Returns integrity, graph, external identity, locking, or storage failures.
    pub fn compact(&self, cutoff: i64) -> Result<AppliedBatch> {
        let mut result = None;
        let receipt = self.store.repair(
            &operation_key()?,
            &format!("compact:v1:{cutoff}"),
            |active, archived| {
                let mut data = RepositoryData {
                    active: parse(active, &self.pearls_dir.join("issues.jsonl"))?,
                    archived: parse(archived, &self.pearls_dir.join("archive.jsonl"))?,
                };
                let mut seen: HashMap<&str, &Pearl> = HashMap::new();
                for task in data.archived.iter().chain(&data.active) {
                    if let Some(existing) = seen.insert(&task.id, task) {
                        if let Some(key) =
                            task.metadata.get(EXTERNAL_KEY).and_then(|key| key.as_str())
                        {
                            return Err(AppError::ExternalKeyConflict {
                                key: key.into(),
                                task_ids: vec![task.id.clone(), existing.id.clone()],
                            });
                        }
                        if existing != task {
                            return Err(AppError::Repository(
                                pearls_core::repository::RepositoryError::CorruptStorage {
                                    path: self.pearls_dir.join("archive.jsonl"),
                                    reason: format!(
                                        "Cannot compact: different Pearl with same ID {}",
                                        task.id
                                    ),
                                },
                            ));
                        }
                    }
                }
                let mut archived_ids = HashSet::new();
                let mut remaining_ids = HashSet::new();
                let mut remaining = Vec::new();
                data.archived
                    .retain(|task| archived_ids.insert(task.id.clone()));
                for task in data.active {
                    if task.status == Status::Closed && task.updated_at <= cutoff {
                        if archived_ids.insert(task.id.clone()) {
                            data.archived.push(task);
                        }
                    } else if archived_ids.contains(&task.id) {
                        return Err(AppError::InvalidInput(format!(
                            "Cannot compact duplicate active/archive ID {}",
                            task.id
                        )));
                    } else if remaining_ids.insert(task.id.clone()) {
                        remaining.push(task);
                    }
                }
                data.active = remaining;
                data.archived.sort_by(|a, b| a.id.cmp(&b.id));
                mutations::validate(&data)?;
                result = Some(data.clone());
                Ok(data)
            },
        )?;
        Ok(AppliedBatch {
            receipt,
            data: result
                .ok_or_else(|| AppError::InvalidInput("Duplicate maintenance identity".into()))?,
        })
    }

    /// Repairs malformed active JSONL, duplicate active IDs, and missing endpoints.
    ///
    /// Explicitly migrates v1 transaction metadata when present.
    /// Keeps the newest duplicate (first on timestamp ties), drops malformed JSON
    /// lines, and preserves archive records. Archived endpoints are retained.
    /// Parsed records failing task validation, conflicting identities, or cycles prevent publication.
    ///
    /// # Errors
    /// Returns validation, integrity, locking, or recovery failures.
    pub fn repair(&self) -> Result<OperationReceipt> {
        self.store
            .migrate_and_repair(&operation_key()?, "doctor:v1", |active, archived| {
                let mut unique: BTreeMap<String, Pearl> = BTreeMap::new();
                for task in active
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .filter_map(|line| serde_json::from_str::<Pearl>(line).ok())
                {
                    let entry = unique
                        .entry(task.id.clone())
                        .or_insert_with(|| task.clone());
                    if task.updated_at > entry.updated_at {
                        *entry = task;
                    }
                }
                let archived = parse(archived, &self.pearls_dir.join("archive.jsonl"))?;
                let ids: HashSet<_> = unique
                    .keys()
                    .chain(archived.iter().map(|task| &task.id))
                    .cloned()
                    .collect();
                let mut data = RepositoryData {
                    active: unique.into_values().collect(),
                    archived,
                };
                for task in &mut data.active {
                    task.deps.retain(|dep| ids.contains(&dep.target_id));
                }
                mutations::validate(&data)?;
                Ok(data)
            })
    }
}

fn parse(text: &str, path: &std::path::Path) -> Result<Vec<Pearl>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).map_err(|error| {
                AppError::Repository(pearls_core::repository::RepositoryError::CorruptStorage {
                    path: path.into(),
                    reason: format!("Invalid Pearl JSON: {error}"),
                })
            })
        })
        .collect()
}

/// Resolves an active task by the legacy partial-ID rules from a consistent read.
///
/// # Errors
/// Returns `NotFound`, ambiguous-ID, or invalid-prefix errors.
pub fn resolve_active_task<'a>(data: &'a RepositoryData, partial: &str) -> Result<&'a Pearl> {
    let id = crate::resolve_pearl_id(partial, &data.active)?;
    data.active
        .iter()
        .find(|task| task.id == id)
        .ok_or(AppError::TaskNotFound { id })
}

impl AppliedBatch {
    /// Returns an active task from this operation's exact after-images.
    ///
    /// # Errors
    /// Returns `NotFound` if this task is absent or archived.
    pub fn task(&self, id: &str) -> Result<Pearl> {
        self.data
            .active
            .iter()
            .find(|task| task.id == id)
            .cloned()
            .ok_or_else(|| AppError::TaskNotFound { id: id.into() })
    }
}
