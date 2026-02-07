// Rust guideline compliant 2026-02-06

//! Merge driver wrapper for Pearls JSONL files.

use anyhow::Result;
use pearls_merge::driver::merge_files;

/// Performs a three-way merge for Pearls JSONL files.
///
/// # Arguments
///
/// * `ancestor` - Path to ancestor file
/// * `current` - Path to current (ours) file
/// * `other` - Path to other (theirs) file
/// * `output` - Optional output path
///
/// # Returns
///
/// Ok if merge succeeds, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - Input files cannot be read
/// - JSONL parsing fails
/// - Conflicts are detected
pub fn execute(
    ancestor: String,
    current: String,
    other: String,
    output: Option<String>,
) -> Result<()> {
    merge_files(&ancestor, &current, &other, output.as_deref())
}
