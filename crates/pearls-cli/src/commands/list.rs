// Rust guideline compliant 2026-02-06

//! Implementation of the `prl list` command.
//!
//! Lists Pearls with optional filtering by status, priority, labels, and author.
//! Supports sorting and multiple output formats.

use crate::OutputFormatter;
use anyhow::Result;
use pearls_core::{Pearl, Status, Storage};
use rayon::prelude::*;
use std::path::Path;

/// Lists Pearls with optional filtering and sorting.
///
/// # Arguments
///
/// * `status_filter` - Optional status filter
/// * `priority_filter` - Optional priority filter
/// * `label_filters` - Optional label filters
/// * `author_filter` - Optional author filter
/// * `include_archived` - Whether to include archived Pearls
/// * `sort_field` - Optional field to sort by
/// * `formatter` - The output formatter to use
///
/// # Returns
///
/// Ok if the list was displayed successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The `.pearls` directory does not exist
/// - The file cannot be read
pub fn execute(
    status_filter: Option<String>,
    priority_filter: Option<u8>,
    label_filters: Vec<String>,
    author_filter: Option<String>,
    include_archived: bool,
    sort_field: Option<String>,
    dep_type_filter: Option<String>,
    created_after: Option<i64>,
    created_before: Option<i64>,
    updated_after: Option<i64>,
    updated_before: Option<i64>,
    formatter: &dyn OutputFormatter,
) -> Result<()> {
    let pearls_dir = Path::new(".pearls");

    // Verify .pearls directory exists
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let storage = Storage::new(pearls_dir.join("issues.jsonl"))?;

    // Load all Pearls
    let mut pearls = storage.load_all()?;
    let status_filter = match status_filter {
        Some(status) => Some(parse_status(&status)?),
        None => None,
    };
    let dep_type_filter = dep_type_filter.and_then(parse_dep_type);

    // Load archived Pearls if requested
    if include_archived {
        let archive_path = pearls_dir.join("archive.jsonl");
        if archive_path.exists() {
            let archive_storage = Storage::new(archive_path)?;
            if let Ok(archived) = archive_storage.load_all() {
                let mut archived = archived;
                for pearl in &mut archived {
                    pearl
                        .metadata
                        .insert("archived".to_string(), serde_json::Value::Bool(true));
                }
                pearls.extend(archived);
            }
        }
    }

    // Apply filters
    pearls = apply_filters(
        pearls,
        status_filter,
        priority_filter,
        label_filters,
        author_filter,
        dep_type_filter,
        created_after,
        created_before,
        updated_after,
        updated_before,
    );

    // Apply sorting
    if let Some(field) = sort_field {
        sort_pearls(&mut pearls, &field);
    } else {
        // Default sort: by updated_at descending
        pearls.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    // Display the list
    println!("{}", formatter.format_list(&pearls));

    Ok(())
}

/// Applies filters to a list of Pearls.
///
/// # Arguments
///
/// * `pearls` - The Pearls to filter
/// * `status_filter` - Optional status filter
/// * `priority_filter` - Optional priority filter
/// * `label_filters` - Optional label filters
/// * `author_filter` - Optional author filter
///
/// # Returns
///
/// The filtered list of Pearls.
fn apply_filters(
    pearls: Vec<Pearl>,
    status_filter: Option<Status>,
    priority_filter: Option<u8>,
    label_filters: Vec<String>,
    author_filter: Option<String>,
    dep_type_filter: Option<pearls_core::DepType>,
    created_after: Option<i64>,
    created_before: Option<i64>,
    updated_after: Option<i64>,
    updated_before: Option<i64>,
) -> Vec<Pearl> {
    const PARALLEL_THRESHOLD: usize = 1_000;

    let predicate = |p: &Pearl| {
        if let Some(status) = status_filter {
            if p.status != status {
                return false;
            }
        }

        if let Some(priority) = priority_filter {
            if p.priority != priority {
                return false;
            }
        }

        if !label_filters.is_empty() {
            for label in &label_filters {
                if !p.labels.iter().any(|l| l.eq_ignore_ascii_case(label)) {
                    return false;
                }
            }
        }

        if let Some(ref author) = author_filter {
            if p.author != *author {
                return false;
            }
        }

        if let Some(dep_type) = dep_type_filter {
            if !p.deps.iter().any(|dep| dep.dep_type == dep_type) {
                return false;
            }
        }

        if let Some(after) = created_after {
            if p.created_at < after {
                return false;
            }
        }
        if let Some(before) = created_before {
            if p.created_at > before {
                return false;
            }
        }
        if let Some(after) = updated_after {
            if p.updated_at < after {
                return false;
            }
        }
        if let Some(before) = updated_before {
            if p.updated_at > before {
                return false;
            }
        }

        true
    };

    if pearls.len() >= PARALLEL_THRESHOLD {
        pearls.into_par_iter().filter(|p| predicate(p)).collect()
    } else {
        pearls.into_iter().filter(predicate).collect()
    }
}

fn parse_dep_type(value: String) -> Option<pearls_core::DepType> {
    match value.as_str() {
        "blocks" => Some(pearls_core::DepType::Blocks),
        "parent_child" => Some(pearls_core::DepType::ParentChild),
        "related" => Some(pearls_core::DepType::Related),
        "discovered_from" => Some(pearls_core::DepType::DiscoveredFrom),
        _ => None,
    }
}

fn parse_status(value: &str) -> Result<Status> {
    match value.to_lowercase().as_str() {
        "open" => Ok(Status::Open),
        "in_progress" => Ok(Status::InProgress),
        "blocked" => Ok(Status::Blocked),
        "deferred" => Ok(Status::Deferred),
        "closed" => Ok(Status::Closed),
        _ => anyhow::bail!("Invalid status filter: {}", value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pearls_core::{DepType, Dependency};

    fn sample_pearl(id: &str, label: &str) -> Pearl {
        Pearl {
            id: id.to_string(),
            title: "Title".to_string(),
            description: String::new(),
            status: Status::Open,
            priority: 2,
            created_at: 1000,
            updated_at: 1000,
            author: "author".to_string(),
            labels: vec![label.to_string()],
            deps: vec![Dependency {
                target_id: "prl-aaa111".to_string(),
                dep_type: DepType::Blocks,
            }],
            metadata: Default::default(),
        }
    }

    #[test]
    fn test_label_filter_case_insensitive() {
        let pearls = vec![sample_pearl("prl-abc123", "Urgent")];
        let filtered = apply_filters(
            pearls,
            None,
            None,
            vec!["urgent".to_string()],
            None,
            None,
            None,
            None,
            None,
            None,
        );
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_dep_type_filter() {
        let pearls = vec![sample_pearl("prl-abc123", "core")];
        let filtered = apply_filters(
            pearls,
            None,
            None,
            Vec::new(),
            None,
            Some(pearls_core::DepType::Blocks),
            None,
            None,
            None,
            None,
        );
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_date_range_filters() {
        let mut pearl1 = sample_pearl("prl-abc123", "core");
        pearl1.created_at = 1000;
        pearl1.updated_at = 2000;

        let mut pearl2 = sample_pearl("prl-def456", "core");
        pearl2.created_at = 3000;
        pearl2.updated_at = 4000;

        let filtered = apply_filters(
            vec![pearl1, pearl2],
            None,
            None,
            Vec::new(),
            None,
            None,
            Some(2000),
            Some(3500),
            Some(1500),
            Some(4500),
        );

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "prl-def456");
    }
}

/// Sorts Pearls by the specified field.
///
/// # Arguments
///
/// * `pearls` - The Pearls to sort (modified in place)
/// * `field` - The field to sort by (id, title, status, priority, created_at, updated_at, author)
fn sort_pearls(pearls: &mut [Pearl], field: &str) {
    match field.to_lowercase().as_str() {
        "id" => pearls.sort_by(|a, b| a.id.cmp(&b.id)),
        "title" => pearls.sort_by(|a, b| a.title.cmp(&b.title)),
        "status" => {
            pearls.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)))
        }
        "priority" => pearls.sort_by(|a, b| a.priority.cmp(&b.priority)),
        "created_at" => pearls.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        "updated_at" => pearls.sort_by(|a, b| a.updated_at.cmp(&b.updated_at)),
        "author" => pearls.sort_by(|a, b| a.author.cmp(&b.author)),
        _ => {
            // Default: sort by updated_at descending
            pearls.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        }
    }
}
