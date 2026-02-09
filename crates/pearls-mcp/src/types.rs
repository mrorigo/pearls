// Rust guideline compliant 2026-02-09

//! MCP tool input and output types for Pearls.

use pearls_core::Pearl;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Input parameters for the `list` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ListInput {
    /// Filter by status.
    pub status: Option<String>,
    /// Filter by priority.
    pub priority: Option<u8>,
    /// Filter by label.
    pub labels: Option<Vec<String>>,
    /// Filter by author.
    pub author: Option<String>,
    /// Include archived Pearls.
    pub include_archived: Option<bool>,
    /// Sort by field.
    pub sort: Option<String>,
    /// Filter by dependency type.
    pub dep_type: Option<String>,
    /// Filter by created_at >= timestamp.
    pub created_after: Option<i64>,
    /// Filter by created_at <= timestamp.
    pub created_before: Option<i64>,
    /// Filter by updated_at >= timestamp.
    pub updated_after: Option<i64>,
    /// Filter by updated_at <= timestamp.
    pub updated_before: Option<i64>,
}

/// Output payload for the `list` tool.
#[derive(Debug, Clone, Serialize)]
pub struct ListResult {
    /// List of Pearls.
    pub pearls: Vec<Pearl>,
    /// Total number of Pearls returned.
    pub total: usize,
}

/// Output payload for `pearls://ready`.
#[derive(Debug, Clone, Serialize)]
pub struct ReadyResource {
    /// Ready queue entries.
    pub ready: Vec<Pearl>,
    /// Total ready items.
    pub total: usize,
    /// Number of returned items.
    pub returned: usize,
    /// Optional message when empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
