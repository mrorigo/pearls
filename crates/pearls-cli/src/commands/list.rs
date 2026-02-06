// Rust guideline compliant 2026-02-06

//! Implementation of the `prl list` command.
//!
//! Lists Pearls with optional filtering by status, priority, labels, and author.
//! Supports sorting and multiple output formats.

use anyhow::Result;
use crate::OutputFormatter;
use pearls_core::{Pearl, Status, Storage};
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

    // Load archived Pearls if requested
    if include_archived {
        let archive_path = pearls_dir.join("archive.jsonl");
        if archive_path.exists() {
            let archive_storage = Storage::new(archive_path)?;
            if let Ok(archived) = archive_storage.load_all() {
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
    status_filter: Option<String>,
    priority_filter: Option<u8>,
    label_filters: Vec<String>,
    author_filter: Option<String>,
) -> Vec<Pearl> {
    pearls
        .into_iter()
        .filter(|p| {
            // Status filter
            if let Some(ref status) = status_filter {
                let matches = match status.to_lowercase().as_str() {
                    "open" => p.status == Status::Open,
                    "in_progress" => p.status == Status::InProgress,
                    "blocked" => p.status == Status::Blocked,
                    "deferred" => p.status == Status::Deferred,
                    "closed" => p.status == Status::Closed,
                    _ => false,
                };
                if !matches {
                    return false;
                }
            }

            // Priority filter
            if let Some(priority) = priority_filter {
                if p.priority != priority {
                    return false;
                }
            }

            // Label filters (all labels must match)
            if !label_filters.is_empty() {
                for label in &label_filters {
                    if !p.labels.iter().any(|l| l.eq_ignore_ascii_case(label)) {
                        return false;
                    }
                }
            }

            // Author filter
            if let Some(ref author) = author_filter {
                if p.author != *author {
                    return false;
                }
            }

            true
        })
        .collect()
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
        "status" => pearls.sort_by(|a, b| {
            format!("{:?}", a.status).cmp(&format!("{:?}", b.status))
        }),
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
