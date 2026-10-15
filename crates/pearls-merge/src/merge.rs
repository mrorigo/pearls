// Rust guideline compliant 2026-02-06

//! Three-way merge algorithm for Pearls JSONL files.

use anyhow::Result;
use pearls_core::{Comment, DepType, Dependency, Pearl};
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
            (ancestor, Some(ours), Some(theirs)) => {
                if ours == theirs {
                    merged.push(ours.clone());
                } else if let Ok(result) = merge_pearl(ancestor, ours, theirs) {
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

fn merge_pearl(ancestor: Option<&Pearl>, ours: &Pearl, theirs: &Pearl) -> Result<Pearl> {
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
    merged.deps = match ancestor {
        Some(ancestor) => merge_deps(&ancestor.deps, &ours.deps, &theirs.deps)?,
        None => union_deps(&ours.deps, &theirs.deps),
    };
    merged.comments = union_comments(&ours.comments, &theirs.comments);
    merged.metadata = merge_metadata(
        ancestor.map(|pearl| &pearl.metadata),
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

fn merge_deps(
    ancestor: &[Dependency],
    ours: &[Dependency],
    theirs: &[Dependency],
) -> Result<Vec<Dependency>> {
    let to_map = |deps: &[Dependency]| {
        deps.iter()
            .map(|dep| (dep.target_id.clone(), dep.dep_type))
            .collect::<HashMap<_, _>>()
    };
    let ancestor = to_map(ancestor);
    let ours = to_map(ours);
    let theirs = to_map(theirs);
    let targets: HashSet<String> = ancestor
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .cloned()
        .collect();

    let mut merged = Vec::new();
    for target in targets {
        let base = ancestor.get(&target).copied();
        let our = ours.get(&target).copied();
        let their = theirs.get(&target).copied();
        let result = if our == their {
            our
        } else if our == base {
            their
        } else if their == base {
            our
        } else {
            anyhow::bail!("Dependency conflict on target {}", target);
        };

        if let Some(dep_type) = result {
            merged.push(Dependency {
                target_id: target,
                dep_type,
            });
        }
    }
    merged.sort_by(|a, b| a.target_id.cmp(&b.target_id));
    Ok(merged)
}

/// Unions comments by comment ID so a comment added on either branch survives.
///
/// Comments are append-only, so both sides contribute. Ordering is by
/// `(created_at, id)`, which is total and independent of hash iteration, so
/// identical input always produces the identical list.
fn union_comments(ours: &[Comment], theirs: &[Comment]) -> Vec<Comment> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut comments: Vec<Comment> = ours
        .iter()
        .chain(theirs.iter())
        .filter(|comment| seen.insert(comment.id.as_str()))
        .cloned()
        .collect();
    comments.sort_by_key(|comment| (comment.created_at, comment.id.clone()));
    comments
}

fn merge_metadata(
    ancestor: Option<&HashMap<String, serde_json::Value>>,
    ours: &HashMap<String, serde_json::Value>,
    theirs: &HashMap<String, serde_json::Value>,
    ours_ts: i64,
    theirs_ts: i64,
) -> Result<HashMap<String, serde_json::Value>> {
    let mut merged = HashMap::new();
    let keys: HashSet<&String> = ancestor
        .into_iter()
        .flat_map(HashMap::keys)
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();
    for key in keys {
        let base = ancestor.and_then(|values| values.get(key));
        let our = ours.get(key);
        let their = theirs.get(key);
        let result = if our == their {
            our
        } else if our == base {
            their
        } else if their == base {
            our
        } else if base.is_none() && ours_ts != theirs_ts {
            if ours_ts > theirs_ts {
                our
            } else {
                their
            }
        } else {
            anyhow::bail!("Metadata conflict on key {}", key);
        };

        if let Some(value) = result {
            merged.insert(key.clone(), value.clone());
        }
    }
    Ok(merged)
}
