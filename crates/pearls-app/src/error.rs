// Rust guideline compliant 2026-02-09

//! Error handling for Pearls application services.

use pearls_core::{repository::RepositoryError, Error as CoreError};
use serde::Serialize;
use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for application-level operations.
pub type Result<T> = std::result::Result<T, AppError>;

/// Stable error codes for tool and resource responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The requested resource or entity was not found.
    NotFound,
    /// The provided identifier matched multiple entities.
    AmbiguousId,
    /// The requested state transition is invalid.
    InvalidTransition,
    /// Input validation failed.
    ValidationError,
    /// The complete task graph has invalid endpoints, identities, or cycles.
    InvalidGraph,
    /// The caller supplied a stale graph revision.
    RevisionConflict,
    /// An operation key was reused with different content.
    IdempotencyConflict,
    /// The full operation receipt expired; its key remains reserved.
    OperationExpired,
    /// An external key identifies multiple tasks or a different definition.
    ExternalKeyConflict,
    /// The repository lock timed out.
    LockTimeout,
    /// Stored data or its schema cannot safely be used.
    CorruptStorage,
    /// IO failure while reading or writing repository data.
    IoError,
    /// Git operation error.
    GitError,
    /// The repository has not been initialized.
    RepoNotInitialized,
    /// The request included invalid inputs.
    InvalidInput,
    /// JSON serialization or parsing failed.
    JsonError,
    /// A fallback for unexpected errors.
    Unknown,
}

/// Application-level errors with stable mapping to error codes.
#[derive(Debug, Error)]
pub enum AppError {
    /// Repository is missing or not initialized.
    #[error("Pearls repository not initialized at {path}. Run 'prl init' first.")]
    RepoNotInitialized {
        /// Path where `.pearls` was expected.
        path: PathBuf,
    },

    /// Invalid input was provided by the caller.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Exact task or comment identity was not found.
    #[error("Entity not found: {id}")]
    TaskNotFound {
        /// Missing full identity.
        id: String,
    },

    /// An external key has conflicting definitions or duplicate owners.
    #[error("External key conflict: {key}")]
    ExternalKeyConflict {
        /// Conflicting opaque key.
        key: String,
        /// Existing tasks associated with the conflict.
        task_ids: Vec<String>,
    },

    /// Stored external identity metadata cannot safely be used.
    #[error("Invalid external identity metadata on {task_id}: {reason}")]
    InvalidExternalIdentity {
        /// Task containing malformed reserved metadata.
        task_id: String,
        /// Integrity failure description.
        reason: String,
    },

    /// Recoverable repository persistence or concurrency failure.
    #[error(transparent)]
    Repository(#[from] RepositoryError),

    /// Error from core library operations.
    #[error(transparent)]
    Core(#[from] CoreError),

    /// IO error not represented by core errors.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl AppError {
    /// Returns a stable error code for the error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            AppError::RepoNotInitialized { .. } => ErrorCode::RepoNotInitialized,
            AppError::InvalidInput(_) => ErrorCode::InvalidInput,
            AppError::TaskNotFound { .. } => ErrorCode::NotFound,
            AppError::InvalidExternalIdentity { .. } => ErrorCode::CorruptStorage,
            AppError::ExternalKeyConflict { .. } => ErrorCode::ExternalKeyConflict,
            AppError::Repository(error) => match error {
                RepositoryError::NotInitialized(_) => ErrorCode::RepoNotInitialized,
                RepositoryError::InvalidInput(_) => ErrorCode::InvalidInput,
                RepositoryError::CorruptStorage { .. }
                | RepositoryError::UnsupportedSchema { .. } => ErrorCode::CorruptStorage,
                RepositoryError::RevisionConflict { .. } => ErrorCode::RevisionConflict,
                RepositoryError::IdempotencyConflict { .. } => ErrorCode::IdempotencyConflict,
                RepositoryError::OperationExpired { .. } => ErrorCode::OperationExpired,
                RepositoryError::LockTimeout => ErrorCode::LockTimeout,
                RepositoryError::InvalidTask(_) => ErrorCode::ValidationError,
                RepositoryError::Io(_) => ErrorCode::IoError,
            },
            AppError::Io(_) => ErrorCode::IoError,
            AppError::Core(core) => match core {
                CoreError::NotFound(_) => ErrorCode::NotFound,
                CoreError::AmbiguousId(_, _) => ErrorCode::AmbiguousId,
                CoreError::InvalidTransition(_) => ErrorCode::InvalidTransition,
                CoreError::TransitionDenied(_) => ErrorCode::InvalidTransition,
                CoreError::InvalidPearl(_) => ErrorCode::ValidationError,
                CoreError::CycleDetected(_) | CoreError::InvalidGraph(_) => ErrorCode::InvalidGraph,
                CoreError::Io(_) => ErrorCode::IoError,
                CoreError::Json(_) => ErrorCode::JsonError,
                CoreError::Git(_) => ErrorCode::GitError,
            },
        }
    }

    /// Returns structured details for errors that benefit from extra context.
    #[must_use]
    pub fn details(&self) -> Option<serde_json::Value> {
        match self {
            AppError::RepoNotInitialized { path } => Some(serde_json::json!({
                "path": path,
            })),
            AppError::InvalidInput(_) => None,
            AppError::TaskNotFound { id } => Some(serde_json::json!({"id": id})),
            AppError::InvalidExternalIdentity { task_id, reason } => {
                Some(serde_json::json!({"task_id": task_id, "reason": reason}))
            }
            AppError::ExternalKeyConflict { key, task_ids } => {
                Some(serde_json::json!({"key": key, "task_ids": task_ids}))
            }
            AppError::Repository(error) => match error {
                RepositoryError::NotInitialized(path) => Some(serde_json::json!({"path": path})),
                RepositoryError::RevisionConflict { expected, actual } => {
                    Some(serde_json::json!({"expected": expected, "actual": actual}))
                }
                RepositoryError::IdempotencyConflict { operation_key }
                | RepositoryError::OperationExpired { operation_key } => {
                    Some(serde_json::json!({"operation_key": operation_key}))
                }
                RepositoryError::CorruptStorage { path, reason } => {
                    Some(serde_json::json!({"path": path, "reason": reason}))
                }
                RepositoryError::UnsupportedSchema { path, found } => {
                    Some(serde_json::json!({"path": path, "found": found}))
                }
                _ => None,
            },
            AppError::Io(_) => None,
            AppError::Core(core) => match core {
                CoreError::AmbiguousId(partial, matches) => Some(serde_json::json!({
                    "partial": partial,
                    "matches": matches,
                })),
                CoreError::CycleDetected(cycle) => Some(serde_json::json!({
                    "cycle": cycle,
                })),
                CoreError::InvalidGraph(issue) => serde_json::to_value(issue).ok(),
                CoreError::TransitionDenied(denial) => serde_json::to_value(denial).ok(),
                _ => None,
            },
        }
    }
}
