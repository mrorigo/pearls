// Rust guideline compliant 2026-02-06

//! Merge driver helpers for Pearls JSONL files.

use crate::merge::{merge_with_conflicts, MergeConflict};
use anyhow::Result;

/// Runs a three-way merge on JSONL files.
///
/// # Arguments
///
/// * `ancestor` - Path to ancestor file
/// * `current` - Path to current (ours) file
/// * `other` - Path to other (theirs) file
/// * `output` - Optional output path (defaults to current)
///
/// # Returns
///
/// Ok if the merge succeeds, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - Files cannot be read
/// - JSONL parsing fails
/// - Conflicts are detected
pub fn merge_files(ancestor: &str, current: &str, other: &str, output: Option<&str>) -> Result<()> {
    let ancestor = read_jsonl(ancestor)?;
    let ours = read_jsonl(current)?;
    let theirs = read_jsonl(other)?;

    let (merged, conflicts) = merge_with_conflicts(ancestor, ours, theirs)?;
    let output_path = output.unwrap_or(current);

    if conflicts.is_empty() {
        write_jsonl(output_path, &merged)?;
        return Ok(());
    }

    write_conflicts(output_path, &merged, &conflicts)?;
    anyhow::bail!(
        "Merge conflicts detected. Resolve conflicts in {}",
        output_path
    );
}

fn read_jsonl(path: &str) -> Result<Vec<pearls_core::Pearl>> {
    let content = std::fs::read_to_string(path)?;
    let mut pearls = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let pearl: pearls_core::Pearl = serde_json::from_str(line)?;
        pearls.push(pearl);
    }
    Ok(pearls)
}

fn write_jsonl(path: &str, pearls: &[pearls_core::Pearl]) -> Result<()> {
    let mut out = String::new();
    for pearl in pearls {
        let json = serde_json::to_string(pearl)?;
        out.push_str(&json);
        out.push('\n');
    }
    std::fs::write(path, out)?;
    Ok(())
}

fn write_conflicts(
    path: &str,
    merged: &[pearls_core::Pearl],
    conflicts: &[MergeConflict],
) -> Result<()> {
    let mut out = String::new();
    for pearl in merged {
        let json = serde_json::to_string(pearl)?;
        out.push_str(&json);
        out.push('\n');
    }

    for conflict in conflicts {
        out.push_str(&format!("<<<<<<< ours {}\n", conflict.id));
        out.push_str(&serde_json::to_string(&conflict.ours)?);
        out.push('\n');
        out.push_str("=======\n");
        out.push_str(&serde_json::to_string(&conflict.theirs)?);
        out.push('\n');
        out.push_str(">>>>>>> theirs\n");
    }

    std::fs::write(path, out)?;
    Ok(())
}
