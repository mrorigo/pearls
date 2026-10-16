// Rust guideline compliant 2026-02-06

//! Error types for the Pearls core library.

use thiserror::Error;

/// Result type alias for Pearls operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for Pearls operations.
#[derive(Debug, Error)]
pub enum Error {
    /// IO error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid Pearl data.
    #[error("Invalid Pearl: {0}")]
    InvalidPearl(String),

    /// Pearl not found.
    #[error("Pearl not found: {0}")]
    NotFound(String),

    /// Cycle detected in dependency graph.
    #[error("Cycle detected: {0:?}")]
    CycleDetected(Vec<String>),

    /// A complete graph has duplicate identities or missing endpoints.
    #[error(transparent)]
    InvalidGraph(#[from] crate::graph::GraphError),

    /// Invalid state transition.
    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),

    /// A graph-aware transition failed with structured blocker information.
    #[error(transparent)]
    TransitionDenied(#[from] crate::fsm::TransitionDenial),

    /// Ambiguous partial ID.
    #[error("Ambiguous ID: {0} matches {1:?}")]
    AmbiguousId(String, Vec<String>),

    /// Git operation error.
    #[error("Git error: {0}")]
    Git(String),
}
