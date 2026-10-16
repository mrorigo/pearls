// Rust guideline compliant 2026-02-06

//! Implementation of the `prl unlink` command.
//!
//! Removes a dependency relationship between two Pearls.

use crate::output_mode::is_json_output;
use pearls_app::service::{resolve_active_task, GraphService, Mutation, TaskRef};

use anyhow::Result;
use std::path::Path;

/// Removes a dependency link between two Pearls.
///
/// # Arguments
///
/// * `from` - The source Pearl ID (full or partial)
/// * `to` - The target Pearl ID (full or partial)
///
/// # Returns
///
/// Ok if the dependency was removed successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - Either Pearl ID is not found
/// - The dependency does not exist
/// - The file cannot be written
pub fn execute(from: String, to: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let mut from_id = String::new();
    let mut to_id = String::new();
    GraphService::open(".")?.execute(|snapshot| {
        let source = resolve_active_task(&snapshot.data, &from)?;
        from_id = source.id.clone();
        to_id = resolve_active_task(&snapshot.data, &to)?.id.clone();
        if !source.deps.iter().any(|dep| dep.target_id == to_id) {
            return Err(pearls_app::AppError::InvalidInput(
                "No dependency exists between the Pearls".into(),
            ));
        }
        Ok(vec![Mutation::Unlink {
            from: TaskRef::Id(from_id.clone()),
            to: TaskRef::Id(to_id.clone()),
        }])
    })?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "unlink",
                "from": from_id,
                "to": to_id
            }))?
        );
    } else {
        println!("✓ Unlinked Pearl: {} -> {}", from_id, to_id);
    }

    Ok(())
}
