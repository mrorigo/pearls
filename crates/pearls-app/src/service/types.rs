// Rust guideline compliant 2026-10-15

//! Versioned requests for atomic application mutations.

use super::GraphRevision;
use pearls_core::{DepType, Status};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Application request protocol version, included in canonical request digests.
pub const API_VERSION: u32 = 1;
/// Reserved metadata field containing an opaque, repository-local external key.
pub const EXTERNAL_KEY: &str = "pearls.external_key";
/// Reserved metadata field preserving the original external creation definition.
pub const EXTERNAL_DEFINITION: &str = "pearls.external_definition";

/// An exact permanent task ID or a creation reference declared earlier in the batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRef {
    /// Full task ID; partial ID resolution is intentionally excluded.
    Id(String),
    /// Nonempty reference from a preceding creation mutation.
    Local(String),
}

/// Creation fields compared when resolving a previously imported external key.
///
/// The definition excludes runtime status, timestamps, links, and comments.
/// `None` priority preserves the caller's intent to use the configured default.
/// Reserved metadata fields cannot be supplied by callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateTask {
    /// Required nonempty title.
    pub title: String,
    /// Explicit author identity.
    pub author: String,
    /// Markdown description, limited to 64 KiB.
    #[serde(default)]
    pub description: String,
    /// Explicit priority or the configured default.
    #[serde(default)]
    pub priority: Option<u8>,
    /// Initial labels in caller-supplied order.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Opaque metadata; object key order has no effect on request identity.
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
    /// Optional unique opaque external identity, including archived tasks.
    #[serde(default)]
    pub external_key: Option<String>,
}

/// Changes to task content; omitted fields retain their current values.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPatch {
    /// Replacement title.
    pub title: Option<String>,
    /// Replacement description.
    pub description: Option<String>,
    /// Replacement priority.
    pub priority: Option<u8>,
    /// Replacement labels.
    pub labels: Option<Vec<String>>,
}

/// One ordered mutation. Archived tasks can be referenced but cannot be edited.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Mutation {
    /// Imports explicit JSONL records, preserving IDs and validating existing transitions.
    Import {
        /// Incoming tasks; later definitions of the same ID replace earlier ones.
        tasks: Vec<pearls_core::Pearl>,
    },
    /// Creates an Open task or resolves its identical original external definition.
    Create {
        /// Unique nonempty batch-local creation reference.
        local_ref: String,
        /// Initial task definition.
        task: CreateTask,
    },
    /// Replaces selected task fields.
    Update {
        /// Task to edit.
        task: TaskRef,
        /// Fields to replace.
        patch: TaskPatch,
    },
    /// Applies the existing status transition rules at this point in the batch.
    SetStatus {
        /// Task to transition.
        task: TaskRef,
        /// Requested status.
        status: Status,
    },
    /// Adds a relationship; an identical existing relationship is a no-op.
    Link {
        /// Task that depends on the target for a blocking relationship.
        from: TaskRef,
        /// Relationship target, possibly archived.
        to: TaskRef,
        /// Relationship type.
        dep_type: DepType,
    },
    /// Removes all relationship types between two tasks, matching existing CLI behavior.
    Unlink {
        /// Source task.
        from: TaskRef,
        /// Relationship target.
        to: TaskRef,
    },
    /// Sets an opaque metadata value without interpreting it.
    SetMetadata {
        /// Task to edit.
        task: TaskRef,
        /// Nonempty field name, excluding reserved external identity fields.
        key: String,
        /// Opaque JSON value.
        value: Value,
    },
    /// Adds a comment using existing comment validation and ID generation.
    AddComment {
        /// Task receiving the comment.
        task: TaskRef,
        /// Comment author.
        author: String,
        /// Comment body.
        body: String,
    },
    /// Deletes a comment by its full ID; unknown IDs are errors.
    DeleteComment {
        /// Task containing the comment.
        task: TaskRef,
        /// Exact comment ID.
        comment_id: String,
    },
}

/// An atomic request with optimistic concurrency and durable idempotency.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BatchRequest {
    /// Nonempty opaque key, scoped to the repository.
    pub operation_key: String,
    /// Required revision from a service snapshot.
    pub expected_revision: GraphRevision,
    /// Nonempty ordered mutations; order contributes to the request digest.
    pub mutations: Vec<Mutation>,
}
