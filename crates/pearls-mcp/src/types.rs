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

/// Empty input for tools without parameters.
#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub struct EmptyInput {}

/// Input parameters for the `create` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CreateInput {
    /// Title of the Pearl.
    pub title: String,
    /// Description of the Pearl.
    pub description: Option<String>,
    /// Priority level (0-4).
    pub priority: Option<u8>,
    /// Labels to assign.
    pub labels: Option<Vec<String>>,
    /// Author name.
    pub author: Option<String>,
}

/// Output payload for the `create` tool.
#[derive(Debug, Clone, Serialize)]
pub struct CreateResult {
    /// Created Pearl.
    pub pearl: Pearl,
}

/// Input parameters for the `show` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ShowInput {
    /// Pearl ID (full or partial).
    pub id: String,
    /// Include archived Pearls.
    pub include_archived: Option<bool>,
}

/// Output payload for the `show` tool.
#[derive(Debug, Clone, Serialize)]
pub struct ShowResult {
    /// The requested Pearl.
    pub pearl: Pearl,
}

/// Input parameters for the `update` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct UpdateInput {
    /// Pearl ID (full or partial).
    pub id: String,
    /// New title.
    pub title: Option<String>,
    /// New description.
    pub description: Option<String>,
    /// New priority (0-4).
    pub priority: Option<u8>,
    /// New status.
    pub status: Option<String>,
    /// Labels to add.
    pub add_labels: Option<Vec<String>>,
    /// Labels to remove.
    pub remove_labels: Option<Vec<String>>,
}

/// Output payload for the `update` tool.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateResult {
    /// Updated Pearl.
    pub pearl: Pearl,
}

/// Input parameters for the `close` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct CloseInput {
    /// Pearl ID (full or partial).
    pub id: String,
}

/// Output payload for the `close` tool.
#[derive(Debug, Clone, Serialize)]
pub struct CloseResult {
    /// Closed Pearl.
    pub pearl: Pearl,
}

/// Input parameters for the `ready` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct ReadyInput {
    /// Maximum number of items to return.
    pub limit: Option<usize>,
}

/// Input parameters for the `plan_snapshot` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct PlanSnapshotInput {
    /// Maximum number of items to include in summaries.
    pub limit: Option<usize>,
}

/// Output payload for the `next_action` tool.
#[derive(Debug, Clone, Serialize)]
pub struct NextActionResult {
    /// The recommended Pearl, if any.
    pub pearl: Option<Pearl>,
    /// Blocking Pearls related to the recommendation.
    pub blockers: Vec<Pearl>,
    /// Optional message when no recommendation is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Output payload for the `plan_snapshot` tool.
#[derive(Debug, Clone, Serialize)]
pub struct PlanSnapshotResult {
    /// Counts of Pearls grouped by status.
    pub counts_by_status: Vec<StatusCount>,
    /// Top ready Pearls.
    pub top_ready: Vec<Pearl>,
    /// Blocked chains with blockers.
    pub blocked_chains: Vec<BlockedChain>,
}

/// Status count entry.
#[derive(Debug, Clone, Serialize)]
pub struct StatusCount {
    /// Status name in snake case.
    pub status: String,
    /// Count of Pearls in this status.
    pub count: usize,
}

/// Blocked chain entry.
#[derive(Debug, Clone, Serialize)]
pub struct BlockedChain {
    /// The blocked Pearl.
    pub pearl: Pearl,
    /// Blocking Pearls.
    pub blockers: Vec<Pearl>,
}

/// Input parameters for the `transition_safe` tool.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct TransitionSafeInput {
    /// Pearl ID (full or partial).
    pub id: String,
    /// Target status.
    pub status: String,
}

/// Output payload for the `transition_safe` tool.
#[derive(Debug, Clone, Serialize)]
pub struct TransitionSafeResult {
    /// Whether the transition was applied.
    pub transitioned: bool,
    /// The updated Pearl when transition succeeded.
    pub pearl: Option<Pearl>,
    /// Blocking Pearls preventing the transition.
    pub blockers: Vec<Pearl>,
    /// Message describing the outcome.
    pub message: String,
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
