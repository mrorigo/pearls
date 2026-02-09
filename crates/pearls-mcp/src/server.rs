// Rust guideline compliant 2026-02-09

//! MCP server runtime for Pearls.

use crate::types::{ListInput, ListResult, ReadyResource};
use pearls_app::{
    list_pearls, parse_dep_type, parse_status, ready_queue, AppError, ErrorEnvelope, ListOptions,
    RepoContext, SuccessEnvelope,
};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{
    AnnotateAble, CallToolResult, Content, ErrorData, Implementation, ListResourcesResult,
    PaginatedRequestParams, ProtocolVersion, RawResource, ReadResourceRequestParams,
    ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::transport::stdio;
use rmcp::{tool, tool_handler, tool_router, RoleServer, ServiceExt};
use rmcp::service::RequestContext;
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

    fn list_tool(&self, input: ListInput) -> Result<ListResult, AppError> {
        let repo = self.repo_context()?;
        let storage = repo.open_storage()?;
        let mut pearls = storage.load_all()?;

        if input.include_archived.unwrap_or(false) {
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

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        if request.uri.as_str() != "pearls://ready" {
            return Err(ErrorData::resource_not_found(
                "Resource not found",
                Some(serde_json::json!({
                    "uri": request.uri,
                })),
            ));
        }

        let ready = self.ready_resource().map_err(map_app_error)?;
        let payload = serde_json::to_string(&ready).map_err(|err| {
            ErrorData::internal_error("Failed to serialize resource", Some(err.to_string().into()))
        })?;

        let contents = ResourceContents::TextResourceContents {
            uri: "pearls://ready".to_string(),
            mime_type: Some("application/json".to_string()),
            text: payload,
            meta: None,
        };

        Ok(ReadResourceResult {
            contents: vec![contents],
        })
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
