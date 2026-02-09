// Rust guideline compliant 2026-02-09

//! Transition validation helpers for Pearls.

use crate::error::Result;
use pearls_core::{fsm, IssueGraph, Pearl, Status};

/// Validates a state transition for a Pearl.
///
/// # Arguments
///
/// * `pearl` - The Pearl to validate
/// * `new_status` - Desired status
/// * `graph` - Dependency graph for validation
///
/// # Returns
///
/// Ok if the transition is valid.
///
/// # Errors
///
/// Returns an error if the transition is not allowed.
pub fn validate_transition(pearl: &Pearl, new_status: Status, graph: &IssueGraph) -> Result<()> {
    Ok(fsm::validate_transition(pearl, new_status, graph)?)
}
