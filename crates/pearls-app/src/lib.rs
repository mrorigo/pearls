// Rust guideline compliant 2026-02-09

//! Shared application services for Pearls.
//!
//! This crate provides reusable, non-CLI-specific helpers for repository
//! checks, ID resolution, list filtering, transition validation, and
//! standardized response envelopes.

pub mod error;
pub mod fsm;
pub mod ids;
pub mod list;
pub mod compact;
pub mod ready;
pub mod repo;
pub mod response;
pub mod time;

pub use error::{AppError, ErrorCode, Result};
pub use fsm::validate_transition;
pub use ids::resolve_pearl_id;
pub use list::{list_pearls, parse_dep_type, parse_status, ListOptions};
pub use compact::compact_closed;
pub use ready::ready_queue;
pub use repo::RepoContext;
pub use response::{ErrorEnvelope, SuccessEnvelope};
pub use time::unix_timestamp;
