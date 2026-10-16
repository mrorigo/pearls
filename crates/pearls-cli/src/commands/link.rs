// Rust guideline compliant 2026-02-06

//! Implementation of the `prl link` command.
//!
//! Links two Pearls with a dependency relationship and performs cycle detection.

use crate::output_mode::is_json_output;
use pearls_app::service::{resolve_active_task, GraphService, Mutation, TaskRef};

use anyhow::Result;
use pearls_core::DepType;
use std::path::Path;

/// Creates a dependency link between two Pearls.
///
/// # Arguments
///
/// * `from` - The source Pearl ID (full or partial)
/// * `to` - The target Pearl ID (full or partial)
/// * `dep_type` - Dependency type string
///
/// # Returns
///
/// Ok if the dependency was created successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - Either Pearl ID is not found
/// - The dependency would create a cycle
/// - The dependency already exists
/// - The file cannot be written
pub fn execute(from: String, to: String, dep_type: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let dep_type = parse_dep_type(&dep_type)?;
    let mut from_id = String::new();
    let mut to_id = String::new();
    GraphService::open(".")?.execute(|snapshot| {
        let source = resolve_active_task(&snapshot.data, &from)?;
        from_id = source.id.clone();
        to_id = resolve_active_task(&snapshot.data, &to)?.id.clone();
        if from_id == to_id {
            return Err(pearls_app::AppError::InvalidInput(
                "Cannot link a Pearl to itself".into(),
            ));
        }
        if source
            .deps
            .iter()
            .any(|dep| dep.target_id == to_id && dep.dep_type == dep_type)
        {
            return Err(pearls_app::AppError::InvalidInput(
                "Dependency already exists".into(),
            ));
        }
        Ok(vec![Mutation::Link {
            from: TaskRef::Id(from_id.clone()),
            to: TaskRef::Id(to_id.clone()),
            dep_type,
        }])
    })?;

    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "ok",
                "action": "link",
                "from": from_id,
                "to": to_id,
                "dep_type": format_dep_type(dep_type)
            }))?
        );
    } else {
        println!(
            "✓ Linked Pearl: {} -> {} ({})",
            from_id,
            to_id,
            format_dep_type(dep_type)
        );
    }

    Ok(())
}

fn parse_dep_type(value: &str) -> Result<DepType> {
    match value {
        "blocks" => Ok(DepType::Blocks),
        "parent_child" => Ok(DepType::ParentChild),
        "related" => Ok(DepType::Related),
        "discovered_from" => Ok(DepType::DiscoveredFrom),
        _ => anyhow::bail!(
            "Invalid dependency type '{}'. Use blocks, parent_child, related, or discovered_from.",
            value
        ),
    }
}

fn format_dep_type(dep_type: DepType) -> &'static str {
    match dep_type {
        DepType::Blocks => "blocks",
        DepType::ParentChild => "parent_child",
        DepType::Related => "related",
        DepType::DiscoveredFrom => "discovered_from",
    }
}
