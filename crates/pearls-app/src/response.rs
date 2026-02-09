// Rust guideline compliant 2026-02-09

//! Response envelopes for tool and resource outputs.

use crate::error::{AppError, ErrorCode};
use serde::Serialize;

/// Standard success envelope for tool responses.
#[derive(Debug, Serialize)]
pub struct SuccessEnvelope<T> {
    /// Status indicator.
    pub status: &'static str,
    /// Result payload.
    pub result: T,
}

impl<T> SuccessEnvelope<T> {
    /// Creates a new success envelope.
    #[must_use]
    pub fn new(result: T) -> Self {
        Self { status: "ok", result }
    }
}

/// Standard error envelope for tool and resource responses.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    /// Stable error code.
    pub code: ErrorCode,
    /// Human-readable error message.
    pub message: String,
    /// Optional structured details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorEnvelope {
    /// Creates a new error envelope from an application error.
    #[must_use]
    pub fn from_error(error: &AppError) -> Self {
        Self {
            code: error.code(),
            message: error.to_string(),
            details: error.details(),
        }
    }
}
