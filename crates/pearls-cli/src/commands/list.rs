// Rust guideline compliant 2026-02-09

//! Implementation of the `prl list` command.
//!
//! Lists Pearls with optional filtering by status, priority, labels, and author.
//! Supports sorting and multiple output formats.

use crate::OutputFormatter;
use anyhow::Result;
use pearls_app::{list_pearls, parse_dep_type, parse_status, ListOptions, RepoContext};

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
#[allow(clippy::too_many_arguments)]
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
    let repo = RepoContext::discover(None)?;

    let storage = repo.open_storage()?;
    let mut pearls = storage.load_all()?;

    if include_archived {
        if let Some(archive_storage) = repo.open_archive_storage()? {
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

    let status_filter = match status_filter {
        Some(status) => Some(parse_status(&status)?),
        None => None,
    };

    let dep_type_filter = match dep_type_filter {
        Some(value) => Some(parse_dep_type(&value)?),
        None => None,
    };

    let options = ListOptions {
        status: status_filter,
        priority: priority_filter,
        labels: label_filters,
        author: author_filter,
        dep_type: dep_type_filter,
        created_after,
        created_before,
        updated_after,
        updated_before,
        sort: sort_field,
    };

    let pearls = list_pearls(pearls, &options);

    println!("{}", formatter.format_list(&pearls));

    Ok(())
}
