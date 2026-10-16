// Rust guideline compliant 2026-10-15

//! Consistent graph selection and scheduling over validated repository data.

use super::{GraphRevision, RepositorySnapshot};
use crate::{AppError, Result};
use pearls_core::{IssueGraph, Pearl, Status, TransitionBlocker};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeSet};

/// Explicit task IDs required by a graph read or scheduling request.
///
/// IDs must be exact. Duplicates are evaluated once. An empty list selects no
/// tasks; it does not imply that any application-specific work is complete.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSelection {
    /// Required full task IDs, including archived IDs when needed.
    pub required_ids: Vec<String>,
}

/// A consistent selected graph with its transitive outgoing relationship targets.
///
/// All relationship types are followed, so every returned dependency endpoint is
/// present. Incoming dependents are not implicitly selected. Each task appears
/// once. Active and archived lists use priority, recency, and full-ID ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphSnapshot {
    /// Repository-wide revision, including changes outside the selection.
    pub revision: GraphRevision,
    /// Deduplicated requested IDs, sorted by full ID.
    pub required_ids: Vec<String>,
    /// Selected active tasks and referenced active tasks.
    pub active: Vec<Pearl>,
    /// Selected archived tasks and referenced archived tasks.
    pub archived: Vec<Pearl>,
}

/// Fact explaining why a selected task is in the blocked set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleBlockReason {
    /// The task explicitly has Blocked status, even if no dependencies remain.
    BlockedStatus,
    /// At least one direct blocking dependency is not Closed.
    Dependencies,
}

/// A selected blocked task with machine-readable reasons and dependency facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockedTask {
    /// Requested task that cannot start.
    pub task: Pearl,
    /// Explicit status first, then dependency reason, when applicable.
    pub reasons: Vec<ScheduleBlockReason>,
    /// Non-closed direct blocking dependencies, including outside the selection.
    ///
    /// IDs and statuses are sorted by full ID and deduplicated.
    pub blockers: Vec<TransitionBlocker>,
}

/// Disjoint scheduling facts for exactly the requested task set.
///
/// Every requested task appears in exactly one set. Running, Deferred, and Closed
/// status take precedence over dependency facts. Archive membership alone does
/// not imply Closed. Every set is ordered by ascending priority, descending update
/// time, then ascending full ID. No completion inference is made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleResult {
    /// Revision shared by every task and blocker in this result.
    pub revision: GraphRevision,
    /// Open tasks with no non-closed blocking dependencies.
    pub ready: Vec<Pearl>,
    /// Tasks whose status is InProgress.
    pub running: Vec<Pearl>,
    /// Open tasks with blockers, and tasks explicitly marked Blocked.
    pub blocked: Vec<BlockedTask>,
    /// Tasks whose status is Deferred.
    pub deferred: Vec<Pearl>,
    /// Tasks whose status is Closed, including archived tasks.
    pub closed: Vec<Pearl>,
}

fn required<'a>(graph: &'a IssueGraph, selection: &TaskSelection) -> Result<Vec<&'a Pearl>> {
    selection
        .required_ids
        .iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|id| {
            graph
                .pearl(id)
                .ok_or_else(|| AppError::TaskNotFound { id: id.clone() })
        })
        .collect()
}

fn order(left: &Pearl, right: &Pearl) -> Ordering {
    left.priority
        .cmp(&right.priority)
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.id.cmp(&right.id))
}

pub(super) fn snapshot(
    snapshot: RepositorySnapshot,
    graph: &IssueGraph,
    selection: &TaskSelection,
) -> Result<GraphSnapshot> {
    let roots = required(graph, selection)?;
    let required_ids = roots.iter().map(|task| task.id.clone()).collect();
    let mut pending = roots;
    let mut included = BTreeSet::new();
    while let Some(task) = pending.pop() {
        if !included.insert(task.id.clone()) {
            continue;
        }
        for dep in &task.deps {
            // Strict validation already checked every endpoint in this snapshot.
            let target = graph
                .pearl(&dep.target_id)
                .ok_or_else(|| AppError::TaskNotFound {
                    id: dep.target_id.clone(),
                })?;
            pending.push(target);
        }
    }
    let mut active: Vec<_> = snapshot
        .data
        .active
        .into_iter()
        .filter(|task| included.contains(&task.id))
        .collect();
    let mut archived: Vec<_> = snapshot
        .data
        .archived
        .into_iter()
        .filter(|task| included.contains(&task.id))
        .collect();
    active.sort_by(order);
    archived.sort_by(order);
    Ok(GraphSnapshot {
        revision: snapshot.revision,
        required_ids,
        active,
        archived,
    })
}

pub(super) fn schedule(
    revision: GraphRevision,
    graph: &IssueGraph,
    selection: &TaskSelection,
) -> Result<ScheduleResult> {
    let mut tasks = required(graph, selection)?;
    tasks.sort_by(|left, right| order(left, right));
    let mut result = ScheduleResult {
        revision,
        ready: Vec::new(),
        running: Vec::new(),
        blocked: Vec::new(),
        deferred: Vec::new(),
        closed: Vec::new(),
    };
    for task in tasks {
        match task.status {
            Status::InProgress => result.running.push(task.clone()),
            Status::Deferred => result.deferred.push(task.clone()),
            Status::Closed => result.closed.push(task.clone()),
            Status::Open | Status::Blocked => {
                let mut blockers: Vec<_> = graph
                    .blocking_deps(&task.id)
                    .into_iter()
                    .map(|dependency| TransitionBlocker {
                        id: dependency.id.clone(),
                        status: dependency.status,
                    })
                    .collect();
                blockers.sort_by(|left, right| left.id.cmp(&right.id));
                blockers.dedup_by(|left, right| left.id == right.id);
                let mut reasons = Vec::new();
                if task.status == Status::Blocked {
                    reasons.push(ScheduleBlockReason::BlockedStatus);
                }
                if !blockers.is_empty() {
                    reasons.push(ScheduleBlockReason::Dependencies);
                }
                if reasons.is_empty() {
                    result.ready.push(task.clone());
                } else {
                    result.blocked.push(BlockedTask {
                        task: task.clone(),
                        reasons,
                        blockers,
                    });
                }
            }
        }
    }
    Ok(result)
}
