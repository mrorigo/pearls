// Rust guideline compliant 2026-02-06

//! Shared utilities for command implementations.

use pearls_core::Pearl;
use std::collections::{BTreeSet, HashSet};

/// Suggests existing labels that aren't already in the provided list.
///
/// # Arguments
///
/// * `storage` - The Pearl storage
/// * `labels` - The labels being added
///
/// # Returns
///
/// Ok if successful, Err otherwise.
pub fn suggest_labels(existing: &[Pearl], labels: &[String]) {
    if existing.is_empty() {
        return;
    }

    // De-duplicate and sort existing labels
    let existing_labels_set: BTreeSet<String> = existing
        .iter()
        .flat_map(|pearl| pearl.labels.iter().cloned())
        .collect();

    if existing_labels_set.is_empty() {
        return;
    }

    let existing_labels: Vec<String> = existing_labels_set.into_iter().collect();

    let lower_existing: HashSet<String> = existing_labels
        .iter()
        .map(|label| label.to_lowercase())
        .collect();

    let mut missing = Vec::new();
    for label in labels {
        if !lower_existing.contains(&label.to_lowercase()) {
            missing.push(label.clone());
        }
    }

    if !missing.is_empty() {
        eprintln!(
            "Label suggestions: existing labels include {}",
            existing_labels.join(", ")
        );
    }
}
