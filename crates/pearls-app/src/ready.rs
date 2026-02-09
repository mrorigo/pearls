// Rust guideline compliant 2026-02-09

//! Ready-queue helpers for Pearls.

use crate::error::Result;
use pearls_core::{IssueGraph, Pearl};

/// Computes the ready queue for the provided Pearls.
///
/// # Arguments
///
/// * `pearls` - Pearls to evaluate
///
/// # Returns
///
/// Vector of ready Pearls, sorted by priority and recency.
///
/// # Errors
///
/// Returns an error if the dependency graph cannot be constructed.
pub fn ready_queue(pearls: Vec<Pearl>) -> Result<Vec<Pearl>> {
    let graph = IssueGraph::from_pearls(pearls)?;
    Ok(graph.ready_queue().into_iter().cloned().collect())
}
