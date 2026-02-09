// Rust guideline compliant 2026-02-09

//! Archive compaction helpers for Pearls.

use crate::error::Result;
use pearls_core::{Pearl, Status};
use std::collections::HashMap;

/// Computes archive candidates and remaining Pearls.
///
/// # Arguments
///
/// * `pearls` - Pearls to evaluate
/// * `archive` - Existing archived Pearls
/// * `cutoff` - Unix timestamp cutoff (closed before this are archived)
///
/// # Returns
///
/// Tuple of `(archive_candidates, remaining, merged_archive)`.
///
/// # Errors
///
/// Returns an error if archive merging fails.
pub fn compact_closed(
    pearls: Vec<Pearl>,
    archive: Vec<Pearl>,
    cutoff: i64,
) -> Result<(Vec<Pearl>, Vec<Pearl>, Vec<Pearl>)> {
    let (archive_candidates, remaining): (Vec<Pearl>, Vec<Pearl>) = pearls
        .into_iter()
        .partition(|pearl| pearl.status == Status::Closed && pearl.updated_at < cutoff);

    let mut archive_map: HashMap<String, Pearl> = archive
        .into_iter()
        .map(|pearl| (pearl.id.clone(), pearl))
        .collect();

    for pearl in &archive_candidates {
        archive_map.insert(pearl.id.clone(), pearl.clone());
    }

    let merged_archive = archive_map.into_values().collect();
    Ok((archive_candidates, remaining, merged_archive))
}
