// Rust guideline compliant 2026-02-09

//! ID resolution helpers for Pearls.

use crate::error::Result;
use pearls_core::{identity, Pearl};

/// Resolves a partial Pearl ID to its canonical full ID.
///
/// # Arguments
///
/// * `partial` - Partial or full Pearl ID
/// * `pearls` - Full list of Pearls to match against
///
/// # Returns
///
/// The canonical Pearl ID.
///
/// # Errors
///
/// Returns an error if the partial ID is invalid, ambiguous, or not found.
pub fn resolve_pearl_id(partial: &str, pearls: &[Pearl]) -> Result<String> {
    Ok(identity::resolve_partial_id(partial, pearls)?)
}
