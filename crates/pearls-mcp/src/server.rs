// Rust guideline compliant 2026-02-09

//! MCP server runtime for Pearls.

use crate::types::{
    BlockedChain, CloseInput, CloseResult, CommentsAddInput, CommentsAddResult, CommentsDeleteInput,
    CommentsDeleteResult, CommentsListInput, CommentsListResult, CreateInput, CreateResult,
    EmptyInput, LinkInput, LinkItem, LinkResult, ListInput, ListResult,
    NextActionResult, PlanSnapshotInput, PlanSnapshotResult, ReadyInput, ReadyResource, ShowInput,
    ShowResult, StatusCount, TransitionSafeInput, TransitionSafeResult, UnlinkInput, UnlinkItem,
    UnlinkResult, UpdateInput, UpdateResult,
};
use pearls_app::{
    list_pearls, parse_dep_type, parse_status, ready_queue, resolve_pearl_id, unix_timestamp,
    validate_transition, AppError, ErrorEnvelope, ListOptions, RepoContext, SuccessEnvelope,
};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, ErrorData, Implementation, ListResourceTemplatesResult,
    ListResourcesResult, PaginatedRequestParams, ProtocolVersion, RawResource,
    RawResourceTemplate, ReadResourceRequestParams, ReadResourceResult, ResourceContents,
    ServerCapabilities, ServerInfo,
};
use rmcp::transport::stdio;
use rmcp::{tool, tool_handler, tool_router, RoleServer, ServiceExt};
use rmcp::service::RequestContext;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::path::PathBuf;
use thiserror::Error;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt;

/// Runtime options for the MCP server.
#[derive(Debug, Clone)]
pub struct McpOptions {
    /// Optional repository root to pin to.
    pub repo: Option<PathBuf>,
    /// Whether mutating tools are disabled.
    pub read_only: bool,
    /// Logging level.
    pub log_level: String,
    /// Optional log file path.
    pub log_file: Option<PathBuf>,
}

impl Default for McpOptions {
    fn default() -> Self {
        Self {
            repo: None,
            read_only: false,
            log_level: "info".to_string(),
            log_file: None,
        }
    }
}

/// MCP server errors.
#[derive(Debug, Error)]
pub enum McpServerError {
    /// IO errors during runtime setup.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Invalid log level provided.
    #[error("Invalid log level: {0}")]
    InvalidLogLevel(String),
    /// Transport or server errors.
    #[error("MCP server error: {0}")]
    Transport(String),
}

/// Runs the MCP server on stdio.
///
/// # Arguments
///
/// * `options` - MCP runtime options
///
/// # Returns
///
/// Ok if the server exits gracefully.
///
/// # Errors
///
/// Returns an error if the runtime cannot be initialized or the server fails.
pub fn run(options: McpOptions) -> Result<(), McpServerError> {
    let _guard = init_tracing(&options)?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {
        let server = PearlsMcp::new(options);
        let service = server
            .serve(stdio())
            .await
            .map_err(|err| McpServerError::Transport(err.to_string()))?;
        service
            .waiting()
            .await
            .map_err(|err| McpServerError::Transport(err.to_string()))?;
        Ok(())
    })
}

fn init_tracing(options: &McpOptions) -> Result<Option<WorkerGuard>, McpServerError> {
    let level = parse_log_level(&options.log_level)?;

    if let Some(path) = &options.log_file {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let (writer, guard) = tracing_appender::non_blocking(file);
        let subscriber = fmt()
            .with_max_level(level)
            .with_target(false)
            .json()
            .with_writer(writer)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
        return Ok(Some(guard));
    }

    let subscriber = fmt()
        .with_max_level(level)
        .with_target(false)
        .json()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
    Ok(None)
}

fn parse_log_level(level: &str) -> Result<Level, McpServerError> {
    match level.to_lowercase().as_str() {
        "error" => Ok(Level::ERROR),
        "warn" => Ok(Level::WARN),
        "info" => Ok(Level::INFO),
        "debug" => Ok(Level::DEBUG),
        other => Err(McpServerError::InvalidLogLevel(other.to_string())),
    }
}

#[derive(Clone)]
struct PearlsMcp {
    tool_router: ToolRouter<Self>,
    options: McpOptions,
}

impl PearlsMcp {
    fn new(options: McpOptions) -> Self {
        Self {
            tool_router: Self::tool_router(),
            options,
        }
    }

    fn repo_context(&self) -> Result<RepoContext, AppError> {
        let root = self.options.repo.as_deref();
        RepoContext::discover(root)
    }

    fn ready_resource(&self) -> Result<ReadyResource, AppError> {
        let repo = self.repo_context()?;
        let storage = repo.open_storage()?;
        let pearls = storage.load_all()?;
        if pearls.is_empty() {
            return Ok(ReadyResource {
                ready: Vec::new(),
                total: 0,
                returned: 0,
                message: Some("No Pearls found".to_string()),
            });
        }

        let ready = ready_queue(pearls)?;
        if ready.is_empty() {
            return Ok(ReadyResource {
                ready: Vec::new(),
                total: 0,
                returned: 0,
                message: Some("No Pearls ready for work".to_string()),
            });
        }

        let returned = ready.len();
        Ok(ReadyResource {
            ready,
            total: returned,
            returned,
            message: None,
        })
    }

    fn load_active_pearls(&self) -> Result<Vec<pearls_core::Pearl>, AppError> {
        let repo = self.repo_context()?;
        let storage = repo.open_storage()?;
        Ok(storage.load_all()?)
    }

    fn load_all_pearls(&self, include_archived: bool) -> Result<Vec<pearls_core::Pearl>, AppError> {
        let repo = self.repo_context()?;
        let storage = repo.open_storage()?;
        let mut pearls = storage.load_all()?;
        if include_archived {
            if let Some(archive_storage) = repo.open_archive_storage()? {
                if let Ok(archived) = archive_storage.load_all() {
                    let mut archived = archived;
                    for pearl in &mut archived {
                        pearl
                            .metadata
                            .insert("archived".to_string(), serde_json::Value::Bool(true));
                    }
                    pearls.extend(archived);
                }
            }
        }
        Ok(pearls)
    }

    fn create_tool(&self, input: CreateInput) -> Result<CreateResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        if input.items.is_empty() {
            return Err(AppError::InvalidInput(
                "Create request must include at least one item".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let config = repo.load_config()?;
        let mut created = Vec::new();

        for item in input.items {
            if item.title.trim().is_empty() {
                return Err(AppError::InvalidInput("Title cannot be empty".to_string()));
            }

            let author = item
                .author
                .or_else(default_author)
                .unwrap_or_else(|| "unknown".to_string());
            let mut pearl = pearls_core::Pearl::new(item.title, author);

            if let Some(description) = item.description {
                enforce_description_limit(&description)?;
                pearl.description = description;
            }

            if let Some(priority) = item.priority {
                if priority > 4 {
                    return Err(AppError::InvalidInput(format!(
                        "Priority must be 0-4, got {}",
                        priority
                    )));
                }
                pearl.priority = priority;
            } else {
                pearl.priority = config.default_priority;
            }

            if let Some(labels) = item.labels {
                pearl.labels = labels;
            }

            pearl.validate()?;
            created.push(pearl);
        }

        let mut storage = repo.open_storage()?;
        for pearl in &created {
            storage.save(pearl)?;
        }

        Ok(CreateResult { pearls: created })
    }

    fn show_tool(&self, input: ShowInput) -> Result<ShowResult, AppError> {
        let pearls = self.load_all_pearls(input.include_archived.unwrap_or(false))?;
        let full_id = resolve_pearl_id(&input.id, &pearls)?;
        let pearl = pearls
            .into_iter()
            .find(|pearl| pearl.id == full_id)
            .ok_or(AppError::Core(pearls_core::Error::NotFound(full_id)))?;
        Ok(ShowResult { pearl })
    }

    fn update_tool(&self, input: UpdateInput) -> Result<UpdateResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let mut pearls = storage.load_all()?;
        let full_id = resolve_pearl_id(&input.id, &pearls)?;
        let position = pearls
            .iter()
            .position(|pearl| pearl.id == full_id)
            .ok_or_else(|| AppError::Core(pearls_core::Error::NotFound(full_id.clone())))?;
        let mut pearl = pearls[position].clone();

        if let Some(title) = input.title {
            pearl.title = title;
        }
        if let Some(description) = input.description {
            enforce_description_limit(&description)?;
            pearl.description = description;
        }
        if let Some(priority) = input.priority {
            if priority > 4 {
                return Err(AppError::InvalidInput(format!(
                    "Priority must be 0-4, got {}",
                    priority
                )));
            }
            pearl.priority = priority;
        }
        if let Some(status) = input.status {
            let new_status = parse_status(&status)?;
            let graph = pearls_core::IssueGraph::from_pearls(pearls.clone())?;
            validate_transition(&pearl, new_status, &graph)?;
            pearl.status = new_status;
        }

        if let Some(add_labels) = input.add_labels {
            for label in add_labels {
                if !pearl.labels.contains(&label) {
                    pearl.labels.push(label);
                }
            }
        }
        if let Some(remove_labels) = input.remove_labels {
            for label in remove_labels {
                pearl.labels.retain(|existing| existing != &label);
            }
        }

        pearl.updated_at = unix_timestamp()?;
        pearl.validate()?;

        pearls[position] = pearl.clone();
        storage.save(&pearl)?;

        Ok(UpdateResult { pearl })
    }

    fn close_tool(&self, input: CloseInput) -> Result<CloseResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let mut pearls = storage.load_all()?;
        let full_id = resolve_pearl_id(&input.id, &pearls)?;
        let position = pearls
            .iter()
            .position(|pearl| pearl.id == full_id)
            .ok_or_else(|| AppError::Core(pearls_core::Error::NotFound(full_id.clone())))?;
        let mut pearl = pearls[position].clone();

        let graph = pearls_core::IssueGraph::from_pearls(pearls.clone())?;
        validate_transition(&pearl, pearls_core::Status::Closed, &graph)?;
        pearl.status = pearls_core::Status::Closed;
        pearl.updated_at = unix_timestamp()?;
        pearl.validate()?;

        pearls[position] = pearl.clone();
        storage.save(&pearl)?;

        Ok(CloseResult { pearl })
    }

    fn ready_tool(&self, input: ReadyInput) -> Result<ReadyResource, AppError> {
        let pearls = self.load_active_pearls()?;
        if pearls.is_empty() {
            return Ok(ReadyResource {
                ready: Vec::new(),
                total: 0,
                returned: 0,
                message: Some("No Pearls found".to_string()),
            });
        }

        let ready = ready_queue(pearls)?;
        let total = ready.len();
        if total == 0 {
            return Ok(ReadyResource {
                ready: Vec::new(),
                total: 0,
                returned: 0,
                message: Some("No Pearls ready for work".to_string()),
            });
        }

        let limited: Vec<pearls_core::Pearl> = ready
            .into_iter()
            .take(input.limit.unwrap_or(usize::MAX))
            .collect();
        Ok(ReadyResource {
            ready: limited.clone(),
            total,
            returned: limited.len(),
            message: None,
        })
    }

    fn comments_add_tool(&self, input: CommentsAddInput) -> Result<CommentsAddResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let pearls = storage.load_all()?;
        let full_id = resolve_pearl_id(&input.id, &pearls)?;
        let mut pearl = storage.load_by_id(&full_id)?;

        let author = input
            .author
            .or_else(default_author)
            .unwrap_or_else(|| "unknown".to_string());
        let comment_id = pearl
            .add_comment(author, input.body)
            .map_err(AppError::from)?;
        pearl.validate()?;
        storage.save(&pearl)?;

        Ok(CommentsAddResult {
            id: pearl.id,
            comment_id,
        })
    }

    fn comments_list_tool(
        &self,
        input: CommentsListInput,
    ) -> Result<CommentsListResult, AppError> {
        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let pearls = storage.load_all()?;
        let full_id = resolve_pearl_id(&input.id, &pearls)?;
        let pearl = storage.load_by_id(&full_id)?;

        Ok(CommentsListResult {
            id: pearl.id,
            comments: pearl.comments.clone(),
            total: pearl.comments.len(),
        })
    }

    fn comments_delete_tool(
        &self,
        input: CommentsDeleteInput,
    ) -> Result<CommentsDeleteResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let pearls = storage.load_all()?;
        let full_id = resolve_pearl_id(&input.id, &pearls)?;
        let mut pearl = storage.load_by_id(&full_id)?;

        let resolved_comment_id = resolve_comment_id(&input.comment_id, &pearl.comments)?;
        if !pearl.delete_comment(&resolved_comment_id) {
            return Err(AppError::InvalidInput(format!(
                "Comment '{}' not found for Pearl {}",
                resolved_comment_id, pearl.id
            )));
        }
        pearl.validate()?;
        storage.save(&pearl)?;

        Ok(CommentsDeleteResult {
            id: pearl.id,
            comment_id: resolved_comment_id,
        })
    }

    fn link_tool(&self, input: LinkInput) -> Result<LinkResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        if input.links.is_empty() {
            return Err(AppError::InvalidInput(
                "Link request must include at least one link".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let mut pearls = storage.load_all()?;
        let mut resolved = Vec::new();

        for link in input.links {
            let dep_type = parse_dep_type(&link.dep_type)?;
            let from_id = resolve_pearl_id(&link.from, &pearls)?;
            let to_id = resolve_pearl_id(&link.to, &pearls)?;

            let from_index = pearls
                .iter()
                .position(|pearl| pearl.id == from_id)
                .ok_or_else(|| AppError::Core(pearls_core::Error::NotFound(from_id.clone())))?;

            let mut updated = pearls[from_index].clone();
            if !updated
                .deps
                .iter()
                .any(|dep| dep.target_id == to_id && dep.dep_type == dep_type)
            {
                updated.deps.push(pearls_core::Dependency {
                    target_id: to_id.clone(),
                    dep_type,
                });
            }

            pearls[from_index] = updated.clone();
            resolved.push(LinkItem {
                from: from_id,
                to: to_id,
                dep_type: link.dep_type,
            });
        }

        pearls_core::IssueGraph::from_pearls(pearls.clone())?;
        for pearl in &pearls {
            storage.save(pearl)?;
        }

        Ok(LinkResult { links: resolved })
    }

    fn unlink_tool(&self, input: UnlinkInput) -> Result<UnlinkResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        if input.links.is_empty() {
            return Err(AppError::InvalidInput(
                "Unlink request must include at least one link".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let mut pearls = storage.load_all()?;
        let mut resolved = Vec::new();
        let mut removed = 0usize;

        for link in input.links {
            let from_id = resolve_pearl_id(&link.from, &pearls)?;
            let to_id = resolve_pearl_id(&link.to, &pearls)?;

            let from_index = pearls
                .iter()
                .position(|pearl| pearl.id == from_id)
                .ok_or_else(|| AppError::Core(pearls_core::Error::NotFound(from_id.clone())))?;

            let mut updated = pearls[from_index].clone();
            let before = updated.deps.len();
            updated.deps.retain(|dep| dep.target_id != to_id);
            removed += before.saturating_sub(updated.deps.len());
            pearls[from_index] = updated.clone();

            resolved.push(UnlinkItem {
                from: from_id,
                to: to_id,
            });
        }

        pearls_core::IssueGraph::from_pearls(pearls.clone())?;
        for pearl in &pearls {
            storage.save(pearl)?;
        }

        Ok(UnlinkResult {
            links: resolved,
            removed,
        })
    }

    fn list_tool(&self, input: ListInput) -> Result<ListResult, AppError> {
        let pearls = self.load_all_pearls(input.include_archived.unwrap_or(false))?;

        let status = match input.status.as_deref() {
            Some(status) => Some(parse_status(status)?),
            None => None,
        };

        let dep_type = match input.dep_type.as_deref() {
            Some(dep_type) => Some(parse_dep_type(dep_type)?),
            None => None,
        };

        let options = ListOptions {
            status,
            priority: input.priority,
            labels: input.labels.unwrap_or_default(),
            author: input.author,
            dep_type,
            created_after: input.created_after,
            created_before: input.created_before,
            updated_after: input.updated_after,
            updated_before: input.updated_before,
            sort: input.sort,
        };

        let pearls = list_pearls(pearls, &options);
        let total = pearls.len();
        Ok(ListResult { pearls, total })
    }

    fn next_action_tool(&self) -> Result<NextActionResult, AppError> {
        let repo = self.repo_context()?;
        let storage = repo.open_storage()?;
        let pearls = storage.load_all()?;
        if pearls.is_empty() {
            return Ok(NextActionResult {
                pearl: None,
                blockers: Vec::new(),
                message: Some("No Pearls found".to_string()),
            });
        }

        let graph = pearls_core::IssueGraph::from_pearls(pearls.clone())?;
        let ready = graph.ready_queue();
        if let Some(pearl) = ready.first() {
            return Ok(NextActionResult {
                pearl: Some((*pearl).clone()),
                blockers: Vec::new(),
                message: None,
            });
        }

        let mut blocked: Vec<&pearls_core::Pearl> = pearls
            .iter()
            .filter(|pearl| {
                pearl.status != pearls_core::Status::Closed
                    && pearl.status != pearls_core::Status::Deferred
                    && graph.is_blocked(&pearl.id)
            })
            .collect();

        blocked.sort_by(|a, b| match a.priority.cmp(&b.priority) {
            std::cmp::Ordering::Equal => b.updated_at.cmp(&a.updated_at),
            other => other,
        });

        if let Some(pearl) = blocked.first() {
            let blockers = graph
                .blocking_deps(&pearl.id)
                .into_iter()
                .cloned()
                .collect();
            return Ok(NextActionResult {
                pearl: Some((*pearl).clone()),
                blockers,
                message: Some("No ready Pearls; showing top blocked item".to_string()),
            });
        }

        Ok(NextActionResult {
            pearl: None,
            blockers: Vec::new(),
            message: Some("No actionable Pearls found".to_string()),
        })
    }

    fn plan_snapshot_tool(&self, input: PlanSnapshotInput) -> Result<PlanSnapshotResult, AppError> {
        let limit = input.limit.unwrap_or(5);
        let repo = self.repo_context()?;
        let storage = repo.open_storage()?;
        let pearls = storage.load_all()?;
        let graph = pearls_core::IssueGraph::from_pearls(pearls.clone())?;

        let mut counts: HashMap<String, usize> = HashMap::new();
        for pearl in &pearls {
            let key = status_key(pearl.status);
            *counts.entry(key).or_insert(0) += 1;
        }

        let mut counts_by_status: Vec<StatusCount> = counts
            .into_iter()
            .map(|(status, count)| StatusCount { status, count })
            .collect();
        counts_by_status.sort_by(|a, b| a.status.cmp(&b.status));

        let top_ready: Vec<pearls_core::Pearl> = graph
            .ready_queue()
            .into_iter()
            .take(limit)
            .cloned()
            .collect();

        let mut blocked: Vec<&pearls_core::Pearl> = pearls
            .iter()
            .filter(|pearl| {
                pearl.status != pearls_core::Status::Closed
                    && pearl.status != pearls_core::Status::Deferred
                    && graph.is_blocked(&pearl.id)
            })
            .collect();
        blocked.sort_by(|a, b| match a.priority.cmp(&b.priority) {
            std::cmp::Ordering::Equal => b.updated_at.cmp(&a.updated_at),
            other => other,
        });

        let blocked_chains = blocked
            .into_iter()
            .take(limit)
            .map(|pearl| BlockedChain {
                pearl: pearl.clone(),
                blockers: graph
                    .blocking_deps(&pearl.id)
                    .into_iter()
                    .cloned()
                    .collect(),
            })
            .collect();

        Ok(PlanSnapshotResult {
            counts_by_status,
            top_ready,
            blocked_chains,
        })
    }

    fn transition_safe_tool(
        &self,
        input: TransitionSafeInput,
    ) -> Result<TransitionSafeResult, AppError> {
        if self.options.read_only {
            return Err(AppError::InvalidInput(
                "Server is running in read-only mode".to_string(),
            ));
        }

        let repo = self.repo_context()?;
        let mut storage = repo.open_storage()?;
        let mut pearls = storage.load_all()?;
        let full_id = resolve_pearl_id(&input.id, &pearls)?;
        let position = pearls
            .iter()
            .position(|pearl| pearl.id == full_id)
            .ok_or_else(|| AppError::Core(pearls_core::Error::NotFound(full_id.clone())))?;
        let mut pearl = pearls[position].clone();

        let new_status = parse_status(&input.status)?;
        let graph = pearls_core::IssueGraph::from_pearls(pearls.clone())?;

        if let Err(error) = validate_transition(&pearl, new_status, &graph) {
            let blockers = graph
                .blocking_deps(&pearl.id)
                .into_iter()
                .cloned()
                .collect();
            return Ok(TransitionSafeResult {
                transitioned: false,
                pearl: Some(pearl),
                blockers,
                message: error.to_string(),
            });
        }

        pearl.status = new_status;
        pearl.updated_at = unix_timestamp()?;
        pearl.validate()?;

        pearls[position] = pearl.clone();
        storage.save(&pearl)?;

        Ok(TransitionSafeResult {
            transitioned: true,
            pearl: Some(pearl),
            blockers: Vec::new(),
            message: "Transition applied".to_string(),
        })
    }
}

#[tool_router(router = tool_router)]
impl PearlsMcp {
    /// Lists Pearls with optional filtering and sorting.
    #[tool(description = "List Pearls with optional filters.")]
    async fn list(
        &self,
        params: Parameters<ListInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let input = params.0;
        let result = self.list_tool(input).map_err(map_app_error)?;
        let envelope = SuccessEnvelope::new(result);
        let payload = serde_json::to_string(&envelope).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Creates a new Pearl.
    #[tool(description = "Create a Pearl.")]
    async fn create(
        &self,
        params: Parameters<CreateInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.create_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Shows a Pearl by ID.
    #[tool(description = "Show a Pearl by ID.")]
    async fn show(
        &self,
        params: Parameters<ShowInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.show_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Updates a Pearl.
    #[tool(description = "Update a Pearl.")]
    async fn update(
        &self,
        params: Parameters<UpdateInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.update_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Closes a Pearl.
    #[tool(description = "Close a Pearl.")]
    async fn close(
        &self,
        params: Parameters<CloseInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.close_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Returns the ready queue.
    #[tool(description = "Return the ready queue.")]
    async fn ready(
        &self,
        params: Parameters<ReadyInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.ready_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Adds a comment to a Pearl.
    #[tool(name = "comments_add", description = "Add a comment to a Pearl.")]
    async fn comments_add(
        &self,
        params: Parameters<CommentsAddInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.comments_add_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Lists comments for a Pearl.
    #[tool(name = "comments_list", description = "List comments for a Pearl.")]
    async fn comments_list(
        &self,
        params: Parameters<CommentsListInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.comments_list_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Deletes a comment from a Pearl.
    #[tool(name = "comments_delete", description = "Delete a comment from a Pearl.")]
    async fn comments_delete(
        &self,
        params: Parameters<CommentsDeleteInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.comments_delete_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Links two Pearls with a dependency.
    #[tool(description = "Link Pearls with a dependency (from depends on to).")]
    async fn link(
        &self,
        params: Parameters<LinkInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.link_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Removes a dependency between Pearls.
    #[tool(description = "Unlink two Pearls.")]
    async fn unlink(
        &self,
        params: Parameters<UnlinkInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.unlink_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Returns the next recommended Pearl and blocker context.
    #[tool(name = "next_action", description = "Return the next recommended Pearl.")]
    async fn next_action(
        &self,
        _params: Parameters<EmptyInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.next_action_tool().map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Returns a compact plan snapshot for the board.
    #[tool(name = "plan_snapshot", description = "Return a compact plan snapshot.")]
    async fn plan_snapshot(
        &self,
        params: Parameters<PlanSnapshotInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.plan_snapshot_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }

    /// Attempts a safe transition and returns blockers if denied.
    #[tool(name = "transition_safe", description = "Safely transition a Pearl status.")]
    async fn transition_safe(
        &self,
        params: Parameters<TransitionSafeInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.transition_safe_tool(params.0).map_err(map_app_error)?;
        let payload = serde_json::to_string(&SuccessEnvelope::new(result)).map_err(|err| {
            ErrorData::internal_error("Failed to serialize response", Some(err.to_string().into()))
        })?;
        Ok(CallToolResult::success(vec![Content::text(payload)]))
    }
}

#[tool_handler(router = self.tool_router)]
impl rmcp::ServerHandler for PearlsMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "pearls".to_string(),
                title: Some("Pearls MCP".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            ..Default::default()
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let ready = RawResource {
            uri: "pearls://ready".to_string(),
            name: "ready".to_string(),
            title: Some("Ready queue".to_string()),
            description: Some("Ready queue of unblocked Pearls".to_string()),
            mime_type: Some("application/json".to_string()),
            size: None,
            icons: None,
            meta: None,
        }
        .no_annotation();

        Ok(ListResourcesResult::with_all_items(vec![ready]))
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        let template = RawResourceTemplate {
            uri_template: "pearls://prl-{id}".to_string(),
            name: "pearl".to_string(),
            title: Some("Pearl".to_string()),
            description: Some("Read a Pearl by ID".to_string()),
            mime_type: Some("application/json".to_string()),
            icons: None,
        }
        .no_annotation();

        Ok(ListResourceTemplatesResult::with_all_items(vec![template]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        self.read_resource_by_uri(request.uri.as_str())
    }
}

fn map_app_error(error: AppError) -> ErrorData {
    let envelope = ErrorEnvelope::from_error(&error);
    let data = serde_json::to_value(&envelope).ok();
    match envelope.code {
        pearls_app::ErrorCode::NotFound => ErrorData::resource_not_found(envelope.message, data),
        pearls_app::ErrorCode::AmbiguousId
        | pearls_app::ErrorCode::InvalidTransition
        | pearls_app::ErrorCode::ValidationError
        | pearls_app::ErrorCode::InvalidInput
        | pearls_app::ErrorCode::RepoNotInitialized => {
            ErrorData::invalid_params(envelope.message, data)
        }
        pearls_app::ErrorCode::IoError
        | pearls_app::ErrorCode::GitError
        | pearls_app::ErrorCode::JsonError
        | pearls_app::ErrorCode::Unknown => ErrorData::internal_error(envelope.message, data),
    }
}

fn status_key(status: pearls_core::Status) -> String {
    match status {
        pearls_core::Status::Open => "open",
        pearls_core::Status::InProgress => "in_progress",
        pearls_core::Status::Blocked => "blocked",
        pearls_core::Status::Deferred => "deferred",
        pearls_core::Status::Closed => "closed",
    }
    .to_string()
}

fn default_author() -> Option<String> {
    std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("USERNAME").ok())
}

fn enforce_description_limit(description: &str) -> Result<(), AppError> {
    const MAX_BYTES: usize = 64 * 1024;
    if description.len() > MAX_BYTES {
        return Err(AppError::InvalidInput(
            "Description exceeds 64KB limit".to_string(),
        ));
    }
    Ok(())
}

fn resolve_comment_id(
    partial: &str,
    comments: &[pearls_core::Comment],
) -> Result<String, AppError> {
    if partial.len() < 3 {
        return Err(AppError::InvalidInput(
            "Comment ID must be at least 3 characters".to_string(),
        ));
    }

    let matches: Vec<&str> = comments
        .iter()
        .filter(|comment| comment.id.starts_with(partial))
        .map(|comment| comment.id.as_str())
        .collect();

    match matches.len() {
        0 => Err(AppError::InvalidInput(format!(
            "Comment '{}' not found",
            partial
        ))),
        1 => Ok(matches[0].to_string()),
        _ => Err(AppError::InvalidInput(format!(
            "Ambiguous comment ID '{}': matches {}",
            partial,
            matches.join(", ")
        ))),
    }
}

impl PearlsMcp {
    fn read_resource_by_uri(&self, uri: &str) -> Result<ReadResourceResult, ErrorData> {
        if uri == "pearls://ready" {
            let ready = self.ready_resource().map_err(map_app_error)?;
            let payload = serde_json::to_string(&ready).map_err(|err| {
                ErrorData::internal_error(
                    "Failed to serialize resource",
                    Some(err.to_string().into()),
                )
            })?;

            let contents = ResourceContents::TextResourceContents {
                uri: "pearls://ready".to_string(),
                mime_type: Some("application/json".to_string()),
                text: payload,
                meta: None,
            };

            return Ok(ReadResourceResult {
                contents: vec![contents],
            });
        }

        if let Some(id) = uri.strip_prefix("pearls://") {
            if let Some(id) = id.strip_prefix("prl-") {
                let id = format!("prl-{}", id);
                let pearls = self.load_all_pearls(true).map_err(map_app_error)?;
                let full_id = resolve_pearl_id(&id, &pearls).map_err(map_app_error)?;
                let pearl = pearls
                    .into_iter()
                    .find(|pearl| pearl.id == full_id)
                    .ok_or_else(|| {
                        ErrorData::resource_not_found(
                            "Pearl not found",
                            Some(serde_json::json!({ "id": full_id })),
                        )
                    })?;

                let payload = serde_json::to_string(&pearl).map_err(|err| {
                    ErrorData::internal_error(
                        "Failed to serialize resource",
                        Some(err.to_string().into()),
                    )
                })?;

                let contents = ResourceContents::TextResourceContents {
                    uri: uri.to_string(),
                    mime_type: Some("application/json".to_string()),
                    text: payload,
                    meta: None,
                };

                return Ok(ReadResourceResult {
                    contents: vec![contents],
                });
            }
        }

        Err(ErrorData::resource_not_found(
            "Resource not found",
            Some(serde_json::json!({
                "uri": uri,
            })),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CreateItem;
    use pearls_core::{Config, Status};
    use std::fs;
    use tempfile::TempDir;

    fn init_repo() -> TempDir {
        let temp = TempDir::new().expect("Failed to create temp dir");
        let pearls_dir = temp.path().join(".pearls");
        fs::create_dir(&pearls_dir).expect("Failed to create .pearls dir");
        fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");
        let config = Config::default();
        config.save(&pearls_dir).expect("Failed to save config");
        temp
    }

    fn server_for(temp: &TempDir) -> PearlsMcp {
        PearlsMcp::new(McpOptions {
            repo: Some(temp.path().to_path_buf()),
            read_only: false,
            log_level: "info".to_string(),
            log_file: None,
        })
    }

    fn extract_text(result: ReadResourceResult) -> String {
        match &result.contents[0] {
            ResourceContents::TextResourceContents { text, .. } => text.clone(),
            ResourceContents::BlobResourceContents { .. } => {
                panic!("Unexpected blob resource contents")
            }
        }
    }

    #[test]
    fn test_create_show_update_close_ready() {
        let temp = init_repo();
        let server = server_for(&temp);

        let created = server
            .create_tool(CreateInput {
                items: vec![CreateItem {
                    title: "Test Pearl".to_string(),
                    description: Some("Desc".to_string()),
                    priority: Some(1),
                    labels: Some(vec!["test".to_string()]),
                    author: Some("tester".to_string()),
                }],
            })
            .expect("create failed");

        let comment = server
            .comments_add_tool(CommentsAddInput {
                id: created.pearls[0].id.clone(),
                body: "Hello".to_string(),
                author: Some("tester".to_string()),
            })
            .expect("comment add failed");

        let show = server
            .show_tool(ShowInput {
                id: created.pearls[0].id[..5].to_string(),
                include_archived: Some(false),
            })
            .expect("show failed");
        assert_eq!(show.pearl.title, "Test Pearl");
        assert_eq!(show.pearl.comments.len(), 1);
        assert_eq!(show.pearl.comments[0].id, comment.comment_id);

        let updated = server
            .update_tool(UpdateInput {
                id: show.pearl.id.clone(),
                title: Some("Updated".to_string()),
                description: None,
                priority: Some(2),
                status: Some("in_progress".to_string()),
                add_labels: Some(vec!["new".to_string()]),
                remove_labels: Some(vec!["test".to_string()]),
            })
            .expect("update failed");
        assert_eq!(updated.pearl.title, "Updated");
        assert_eq!(updated.pearl.status, Status::InProgress);

        let closed = server
            .close_tool(CloseInput {
                id: updated.pearl.id.clone(),
            })
            .expect("close failed");
        assert_eq!(closed.pearl.status, Status::Closed);

        let ready = server
            .ready_tool(ReadyInput { limit: None })
            .expect("ready failed");
        assert!(ready.ready.is_empty());
    }

    #[test]
    fn test_comments_list_and_delete() {
        let temp = init_repo();
        let server = server_for(&temp);

        let created = server
            .create_tool(CreateInput {
                items: vec![CreateItem {
                    title: "Comment Pearl".to_string(),
                    description: None,
                    priority: None,
                    labels: None,
                    author: None,
                }],
            })
            .expect("create failed");

        let added = server
            .comments_add_tool(CommentsAddInput {
                id: created.pearls[0].id.clone(),
                body: "First".to_string(),
                author: None,
            })
            .expect("comment add failed");

        let list = server
            .comments_list_tool(CommentsListInput {
                id: created.pearls[0].id.clone(),
            })
            .expect("comment list failed");
        assert_eq!(list.total, 1);
        assert_eq!(list.comments[0].id, added.comment_id);

        let deleted = server
            .comments_delete_tool(CommentsDeleteInput {
                id: created.pearls[0].id.clone(),
                comment_id: added.comment_id[..5].to_string(),
            })
            .expect("comment delete failed");
        assert_eq!(deleted.comment_id, added.comment_id);

        let list = server
            .comments_list_tool(CommentsListInput {
                id: created.pearls[0].id.clone(),
            })
            .expect("comment list failed");
        assert_eq!(list.total, 0);
    }

    #[test]
    fn test_link_and_unlink() {
        let temp = init_repo();
        let server = server_for(&temp);

        let created = server
            .create_tool(CreateInput {
                items: vec![
                    CreateItem {
                        title: "Parent".to_string(),
                        description: None,
                        priority: None,
                        labels: None,
                        author: None,
                    },
                    CreateItem {
                        title: "Child".to_string(),
                        description: None,
                        priority: None,
                        labels: None,
                        author: None,
                    },
                    CreateItem {
                        title: "Peer".to_string(),
                        description: None,
                        priority: None,
                        labels: None,
                        author: None,
                    },
                ],
            })
            .expect("create failed");
        let from_id = created.pearls[0].id.clone();
        let to_id = created.pearls[1].id.clone();
        let peer_id = created.pearls[2].id.clone();

        let link = server
            .link_tool(LinkInput {
                links: vec![
                    LinkItem {
                        from: from_id.clone(),
                        to: to_id.clone(),
                        dep_type: "blocks".to_string(),
                    },
                    LinkItem {
                        from: from_id.clone(),
                        to: peer_id.clone(),
                        dep_type: "related".to_string(),
                    },
                ],
            })
            .expect("link failed");
        assert_eq!(link.links.len(), 2);

        let list = server
            .show_tool(ShowInput {
                id: from_id.clone(),
                include_archived: Some(false),
            })
            .expect("show failed");
        assert_eq!(list.pearl.deps.len(), 2);

        let unlink = server
            .unlink_tool(UnlinkInput {
                links: vec![
                    UnlinkItem {
                        from: from_id.clone(),
                        to: to_id.clone(),
                    },
                    UnlinkItem {
                        from: from_id.clone(),
                        to: peer_id.clone(),
                    },
                ],
            })
            .expect("unlink failed");
        assert_eq!(unlink.removed, 2);

        let list = server
            .show_tool(ShowInput {
                id: from_id,
                include_archived: Some(false),
            })
            .expect("show failed");
        assert!(list.pearl.deps.is_empty());
    }

    #[test]
    fn test_read_resources() {
        let temp = init_repo();
        let server = server_for(&temp);

        let created = server
            .create_tool(CreateInput {
                items: vec![CreateItem {
                    title: "Resource Pearl".to_string(),
                    description: None,
                    priority: None,
                    labels: None,
                    author: None,
                }],
            })
            .expect("create failed");

        let ready = server
            .read_resource_by_uri("pearls://ready")
            .expect("ready resource failed");
        let ready_text = extract_text(ready);
        assert!(ready_text.contains("ready"));

        let resource_uri = format!("pearls://{}", created.pearls[0].id);
        let pearl = server
            .read_resource_by_uri(&resource_uri)
            .expect("pearl resource failed");
        let pearl_text = extract_text(pearl);
        assert!(pearl_text.contains(&created.pearls[0].id));
    }

    #[test]
    fn test_read_only_blocks_mutations() {
        let temp = init_repo();
        let server = PearlsMcp::new(McpOptions {
            repo: Some(temp.path().to_path_buf()),
            read_only: true,
            log_level: "info".to_string(),
            log_file: None,
        });

        let result = server.create_tool(CreateInput {
            items: vec![CreateItem {
                title: "Blocked".to_string(),
                description: None,
                priority: None,
                labels: None,
                author: None,
            }],
        });

        assert!(result.is_err());
    }

    #[test]
    fn test_plan_snapshot_and_next_action() {
        let temp = init_repo();
        let server = server_for(&temp);

        let first = server
            .create_tool(CreateInput {
                items: vec![CreateItem {
                    title: "Ready Pearl".to_string(),
                    description: None,
                    priority: Some(0),
                    labels: None,
                    author: None,
                }],
            })
            .expect("create failed");

        let next = server.next_action_tool().expect("next action failed");
        assert_eq!(next.pearl.as_ref().unwrap().id, first.pearls[0].id);

        let snapshot = server
            .plan_snapshot_tool(PlanSnapshotInput { limit: Some(5) })
            .expect("snapshot failed");
        assert!(!snapshot.counts_by_status.is_empty());
    }

    #[test]
    fn test_transition_safe_blocks_when_invalid() {
        let temp = init_repo();
        let server = server_for(&temp);

        let created = server
            .create_tool(CreateInput {
                items: vec![
                    CreateItem {
                        title: "Blocker".to_string(),
                        description: None,
                        priority: None,
                        labels: None,
                        author: None,
                    },
                    CreateItem {
                        title: "Transition Pearl".to_string(),
                        description: None,
                        priority: None,
                        labels: None,
                        author: None,
                    },
                ],
            })
            .expect("create failed");
        let blocker_id = created.pearls[0].id.clone();
        let transition_id = created.pearls[1].id.clone();

        let repo = server.repo_context().expect("repo context");
        let mut storage = repo.open_storage().expect("storage");
        let mut pearl = storage
            .load_by_id(&transition_id)
            .expect("load pearl");
        pearl.deps.push(pearls_core::Dependency {
            target_id: blocker_id.clone(),
            dep_type: pearls_core::DepType::Blocks,
        });
        storage.save(&pearl).expect("save pearl");

        let result = server
            .transition_safe_tool(TransitionSafeInput {
                id: transition_id.clone(),
                status: "closed".to_string(),
            })
            .expect("transition safe failed");

        assert!(!result.transitioned);
        assert!(result.message.contains("transition"));
    }

    #[test]
    fn test_ready_resource_empty() {
        let temp = init_repo();
        let server = server_for(&temp);

        let ready = server
            .read_resource_by_uri("pearls://ready")
            .expect("ready resource failed");
        let text = extract_text(ready);
        assert!(text.contains("No Pearls found"));
    }
}
