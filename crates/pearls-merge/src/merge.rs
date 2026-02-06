// Rust guideline compliant 2026-02-06

//! Three-way merge algorithm for Pearls JSONL files.

use anyhow::Result;
use pearls_core::{DepType, Dependency, Pearl};
use std::collections::{HashMap, HashSet};

/// Conflict encountered during merge.
#[derive(Debug, Clone)]
pub struct MergeConflict {
    /// Pearl ID with conflict.
    pub id: String,
    /// Our version.
    pub ours: Pearl,
    /// Their version.
    pub theirs: Pearl,
}

/// Performs a three-way merge of Pearls.
///
/// # Arguments
///
/// * `ancestor` - Pearls from the common ancestor
/// * `ours` - Pearls from the current branch
/// * `theirs` - Pearls from the other branch
///
/// # Returns
///
/// Merged Pearls or an error if conflicts cannot be resolved.
pub fn three_way_merge(
    ancestor: Vec<Pearl>,
    ours: Vec<Pearl>,
    theirs: Vec<Pearl>,
) -> Result<Vec<Pearl>> {
    let (merged, conflicts) = merge_with_conflicts(ancestor, ours, theirs)?;
    if !conflicts.is_empty() {
        anyhow::bail!("Merge conflicts detected for {} pearls", conflicts.len());
    }
    Ok(merged)
}

/// Performs a three-way merge and returns conflicts separately.
pub fn merge_with_conflicts(
    ancestor: Vec<Pearl>,
    ours: Vec<Pearl>,
    theirs: Vec<Pearl>,
) -> Result<(Vec<Pearl>, Vec<MergeConflict>)> {
    let ancestor_map = to_map(ancestor);
    let ours_map = to_map(ours);
    let theirs_map = to_map(theirs);

    let ids: HashSet<String> = ancestor_map
        .keys()
        .chain(ours_map.keys())
        .chain(theirs_map.keys())
        .cloned()
        .collect();

    let mut merged = Vec::new();
    let mut conflicts = Vec::new();

    for id in ids {
        match (
            ancestor_map.get(&id),
            ours_map.get(&id),
            theirs_map.get(&id),
        ) {
            (_, Some(ours), Some(theirs)) => {
                if ours == theirs {
                    merged.push(ours.clone());
                } else if let Ok(result) = merge_pearl(ours, theirs) {
                    merged.push(result);
                } else {
                    conflicts.push(MergeConflict {
                        id,
                        ours: ours.clone(),
                        theirs: theirs.clone(),
                    });
                }
            }
            (_, Some(ours), None) => merged.push(ours.clone()),
            (_, None, Some(theirs)) => merged.push(theirs.clone()),
            _ => {}
        }
    }

    merged.sort_by(|a, b| a.id.cmp(&b.id));
    Ok((merged, conflicts))
}

fn to_map(pearls: Vec<Pearl>) -> HashMap<String, Pearl> {
    pearls.into_iter().map(|p| (p.id.clone(), p)).collect()
}

fn merge_pearl(ours: &Pearl, theirs: &Pearl) -> Result<Pearl> {
    if ours.id != theirs.id {
        anyhow::bail!("Cannot merge different Pearl IDs");
    }

    if ours.updated_at == theirs.updated_at {
        anyhow::bail!("Conflicting updates with identical timestamps");
    }

    let newer = if ours.updated_at > theirs.updated_at {
        ours
    } else {
        theirs
    };

    let mut merged = newer.clone();
    merged.created_at = std::cmp::min(ours.created_at, theirs.created_at);
    merged.labels = union_labels(&ours.labels, &theirs.labels);
    merged.deps = union_deps(&ours.deps, &theirs.deps);
    merged.metadata = merge_metadata(
        &ours.metadata,
        &theirs.metadata,
        ours.updated_at,
        theirs.updated_at,
    )?;
    merged.updated_at = std::cmp::max(ours.updated_at, theirs.updated_at);
    merged.id = newer.id.clone();
    merged.author = newer.author.clone();
    merged.title = newer.title.clone();
    merged.description = newer.description.clone();
    merged.status = newer.status;
    merged.priority = newer.priority;

    Ok(merged)
}

fn union_labels(ours: &[String], theirs: &[String]) -> Vec<String> {
    let mut set = HashSet::new();
    for label in ours.iter().chain(theirs.iter()) {
        set.insert(label.clone());
    }
    let mut labels: Vec<String> = set.into_iter().collect();
    labels.sort();
    labels
}

fn union_deps(ours: &[Dependency], theirs: &[Dependency]) -> Vec<Dependency> {
    let mut set: HashSet<(String, DepType)> = HashSet::new();
    for dep in ours.iter().chain(theirs.iter()) {
        set.insert((dep.target_id.clone(), dep.dep_type));
    }
    let mut deps: Vec<Dependency> = set
        .into_iter()
        .map(|(target_id, dep_type)| Dependency {
            target_id,
            dep_type,
        })
        .collect();
    deps.sort_by(|a, b| a.target_id.cmp(&b.target_id));
    deps
}

fn merge_metadata(
    ours: &HashMap<String, serde_json::Value>,
    theirs: &HashMap<String, serde_json::Value>,
    ours_ts: i64,
    theirs_ts: i64,
) -> Result<HashMap<String, serde_json::Value>> {
    let mut merged = HashMap::new();
    let keys: HashSet<&String> = ours.keys().chain(theirs.keys()).collect();
    for key in keys {
        match (ours.get(key), theirs.get(key)) {
            (Some(a), Some(b)) => {
                if a == b {
                    merged.insert(key.clone(), a.clone());
                } else if ours_ts == theirs_ts {
                    anyhow::bail!("Metadata conflict on key {}", key);
                } else if ours_ts > theirs_ts {
                    merged.insert(key.clone(), a.clone());
                } else {
                    merged.insert(key.clone(), b.clone());
                }
            }
            (Some(a), None) => {
                merged.insert(key.clone(), a.clone());
            }
            (None, Some(b)) => {
                merged.insert(key.clone(), b.clone());
            }
            _ => {}
        }
    }
    Ok(merged)
}
