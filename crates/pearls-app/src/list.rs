// Rust guideline compliant 2026-02-09

//! Listing and filtering helpers for Pearls.

use crate::error::{AppError, Result};
use pearls_core::{DepType, Pearl, Status};
use rayon::prelude::*;

/// List options for filtering and sorting Pearls.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Filter by status.
    pub status: Option<Status>,
    /// Filter by priority.
    pub priority: Option<u8>,
    /// Filter by labels.
    pub labels: Vec<String>,
    /// Filter by author.
    pub author: Option<String>,
    /// Filter by dependency type.
    pub dep_type: Option<DepType>,
    /// Filter by created_at >= timestamp.
    pub created_after: Option<i64>,
    /// Filter by created_at <= timestamp.
    pub created_before: Option<i64>,
    /// Filter by updated_at >= timestamp.
    pub updated_after: Option<i64>,
    /// Filter by updated_at <= timestamp.
    pub updated_before: Option<i64>,
    /// Sort field override.
    pub sort: Option<String>,
}

/// Parses a status string into a `Status` value.
///
/// # Arguments
///
/// * `value` - Status string
///
/// # Returns
///
/// The parsed status.
///
/// # Errors
///
/// Returns an error if the status is invalid.
pub fn parse_status(value: &str) -> Result<Status> {
    match value.to_lowercase().as_str() {
        "open" => Ok(Status::Open),
        "in_progress" | "in-progress" => Ok(Status::InProgress),
        "blocked" => Ok(Status::Blocked),
        "deferred" => Ok(Status::Deferred),
        "closed" => Ok(Status::Closed),
        _ => Err(AppError::InvalidInput(format!(
            "Invalid status filter: {}",
            value
        ))),
    }
}

/// Parses a dependency type string into a `DepType` value.
///
/// # Arguments
///
/// * `value` - Dependency type string
///
/// # Returns
///
/// The parsed dependency type.
///
/// # Errors
///
/// Returns an error if the dependency type is invalid.
pub fn parse_dep_type(value: &str) -> Result<DepType> {
    match value {
        "blocks" => Ok(DepType::Blocks),
        "parent_child" => Ok(DepType::ParentChild),
        "related" => Ok(DepType::Related),
        "discovered_from" => Ok(DepType::DiscoveredFrom),
        _ => Err(AppError::InvalidInput(format!(
            "Invalid dependency type: {}",
            value
        ))),
    }
}

/// Filters and sorts a list of Pearls based on `ListOptions`.
///
/// # Arguments
///
/// * `pearls` - Pearls to filter and sort
/// * `options` - List options
///
/// # Returns
///
/// The filtered and sorted list of Pearls.
pub fn list_pearls(mut pearls: Vec<Pearl>, options: &ListOptions) -> Vec<Pearl> {
    pearls = apply_filters(pearls, options);

    if let Some(field) = &options.sort {
        sort_pearls(&mut pearls, field);
    } else {
        pearls.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    pearls
}

fn apply_filters(pearls: Vec<Pearl>, options: &ListOptions) -> Vec<Pearl> {
    const PARALLEL_THRESHOLD: usize = 1_000;

    let predicate = |p: &Pearl| {
        if let Some(status) = options.status {
            if p.status != status {
                return false;
            }
        }

        if let Some(priority) = options.priority {
            if p.priority != priority {
                return false;
            }
        }

        if !options.labels.is_empty() {
            for label in &options.labels {
                if !p.labels.iter().any(|l| l.eq_ignore_ascii_case(label)) {
                    return false;
                }
            }
        }

        if let Some(ref author) = options.author {
            if p.author != *author {
                return false;
            }
        }

        if let Some(dep_type) = options.dep_type {
            if !p.deps.iter().any(|dep| dep.dep_type == dep_type) {
                return false;
            }
        }

        if let Some(after) = options.created_after {
            if p.created_at < after {
                return false;
            }
        }
        if let Some(before) = options.created_before {
            if p.created_at > before {
                return false;
            }
        }
        if let Some(after) = options.updated_after {
            if p.updated_at < after {
                return false;
            }
        }
        if let Some(before) = options.updated_before {
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

fn sort_pearls(pearls: &mut [Pearl], field: &str) {
    match field {
        "id" => pearls.sort_by(|a, b| a.id.cmp(&b.id)),
        "title" => pearls.sort_by(|a, b| a.title.cmp(&b.title)),
        "status" => pearls.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status))),
        "priority" => pearls.sort_by(|a, b| a.priority.cmp(&b.priority)),
        "created_at" => pearls.sort_by(|a, b| a.created_at.cmp(&b.created_at)),
        "updated_at" => pearls.sort_by(|a, b| a.updated_at.cmp(&b.updated_at)),
        "author" => pearls.sort_by(|a, b| a.author.cmp(&b.author)),
        _ => pearls.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
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
            comments: Vec::new(),
        }
    }

    #[test]
    fn test_label_filter_case_insensitive() {
        let pearls = vec![sample_pearl("prl-abc123", "Urgent")];
        let options = ListOptions {
            labels: vec!["urgent".to_string()],
            ..ListOptions::default()
        };
        let filtered = list_pearls(pearls, &options);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_dep_type_filter() {
        let pearls = vec![sample_pearl("prl-abc123", "core")];
        let options = ListOptions {
            dep_type: Some(DepType::Blocks),
            ..ListOptions::default()
        };
        let filtered = list_pearls(pearls, &options);
        assert_eq!(filtered.len(), 1);
    }
}
