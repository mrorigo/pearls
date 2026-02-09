// Rust guideline compliant 2026-02-09

//! MCP server implementation for Pearls.

mod server;
mod types;

pub use server::{run, McpOptions};
