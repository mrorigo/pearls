// Rust guideline compliant 2026-02-06

//! Implementation of the `prl link` command.
//!
//! Links two Pearls with a dependency relationship and performs cycle detection.

use crate::output_mode::is_json_output;
use anyhow::Result;
use pearls_core::{identity, DepType, Dependency, IssueGraph, Storage};
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

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let mut pearls = storage.load_all()?;

    let from_id = resolve_id(&from, &pearls)?;
    let to_id = resolve_id(&to, &pearls)?;

    if from_id == to_id {
        anyhow::bail!("Cannot link a Pearl to itself.");
    }

    let dep_type = parse_dep_type(&dep_type)?;

    let from_index = pearls
        .iter()
        .position(|pearl| pearl.id == from_id)
        .ok_or_else(|| anyhow::anyhow!("Pearl '{}' not found", from_id))?;

    let mut updated = pearls[from_index].clone();
    if updated
        .deps
        .iter()
        .any(|dep| dep.target_id == to_id && dep.dep_type == dep_type)
    {
        anyhow::bail!(
            "Dependency already exists between {} and {}",
            from_id,
            to_id
        );
    }

    updated.deps.push(Dependency {
        target_id: to_id.clone(),
        dep_type,
    });
    pearls[from_index] = updated.clone();

    IssueGraph::from_pearls(pearls.clone())?;
    updated.validate()?;
    storage.save(&updated)?;

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

fn resolve_id(id: &str, pearls: &[pearls_core::Pearl]) -> Result<String> {
    if id.starts_with("prl-")
        && (id.len() == 10 || id.len() == 12)
        && identity::validate_id_format(id).is_ok()
    {
        return Ok(id.to_string());
    }

    identity::resolve_partial_id(id, pearls).map_err(|e| anyhow::anyhow!("{}", e))
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
