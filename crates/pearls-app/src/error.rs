// Rust guideline compliant 2026-02-09

//! Error handling for Pearls application services.

use pearls_core::Error as CoreError;
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
            AppError::Io(_) => ErrorCode::IoError,
            AppError::Core(core) => match core {
                CoreError::NotFound(_) => ErrorCode::NotFound,
                CoreError::AmbiguousId(_, _) => ErrorCode::AmbiguousId,
                CoreError::InvalidTransition(_) => ErrorCode::InvalidTransition,
                CoreError::InvalidPearl(_) => ErrorCode::ValidationError,
                CoreError::CycleDetected(_) => ErrorCode::ValidationError,
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
            AppError::Io(_) => None,
            AppError::Core(core) => match core {
                CoreError::AmbiguousId(partial, matches) => Some(serde_json::json!({
                    "partial": partial,
                    "matches": matches,
                })),
                CoreError::CycleDetected(cycle) => Some(serde_json::json!({
                    "cycle": cycle,
                })),
                _ => None,
            },
        }
    }
}
