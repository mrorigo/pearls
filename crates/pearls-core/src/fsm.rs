// Rust guideline compliant 2026-02-06

//! Finite State Machine module for status transitions.
//!
//! This module provides functionality for validating state transitions
//! according to the Pearls FSM rules. The FSM enforces the following transitions:
//!
//! - Open → InProgress (only if not blocked)
//! - InProgress → Closed (only if not blocked)
//! - InProgress → Open
//! - Any → Deferred
//! - Closed → Open

use crate::{Error, Pearl, Result, Status};
use serde::{Deserialize, Serialize};

/// Reason a graph-aware status transition was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum TransitionDenialReason {
    /// Starting or closing is prevented by non-closed blocking dependencies.
    #[error("open blocking dependencies")]
    Blocked,
    /// The requested transition is absent from the existing status rules.
    #[error("transition is not allowed by the status rules")]
    Unsupported,
}

/// A non-closed task preventing a requested transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionBlocker {
    /// Full ID of the blocking dependency.
    pub id: String,
    /// Current status of that dependency.
    pub status: Status,
}

/// Structured explanation of a denied status transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error(
    "Invalid state transition for Pearl {task_id}: {from_status:?} -> {to_status:?} ({reason})"
)]
pub struct TransitionDenial {
    /// Full ID of the task whose status would change.
    pub task_id: String,
    /// Task status before the proposed change.
    pub from_status: Status,
    /// Requested target status.
    pub to_status: Status,
    /// Status-rule or dependency reason for the denial.
    pub reason: TransitionDenialReason,
    /// Blocking IDs and statuses, sorted by full ID with duplicates removed.
    ///
    /// Empty for an unsupported transition: resolving dependencies alone cannot
    /// make such a transition valid.
    pub blockers: Vec<TransitionBlocker>,
}

impl Status {
    /// Checks if a transition to the target status is valid.
    ///
    /// # Arguments
    ///
    /// * `target` - The target status to transition to
    /// * `is_blocked` - Whether the Pearl has open blocking dependencies
    ///
    /// # Returns
    ///
    /// Ok if the transition is valid, Err with descriptive message otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The transition is not in the valid transition set
    /// - The transition is blocked by dependencies
    pub fn can_transition_to(&self, target: Status, is_blocked: bool) -> Result<()> {
        // Deferred is always reachable from any state
        if target == Status::Deferred {
            return Ok(());
        }

        // Closed is always reachable from any state except when blocked
        if target == Status::Closed {
            if is_blocked {
                return Err(Error::InvalidTransition(
                    "Cannot close a Pearl with open blocking dependencies".to_string(),
                ));
            }
            return Ok(());
        }

        // Reopen (Closed → Open) is always allowed
        if *self == Status::Closed && target == Status::Open {
            return Ok(());
        }

        // InProgress → Open is always allowed
        if *self == Status::InProgress && target == Status::Open {
            return Ok(());
        }

        // Open → InProgress is allowed only if not blocked
        if *self == Status::Open && target == Status::InProgress {
            if is_blocked {
                return Err(Error::InvalidTransition(
                    "Cannot start a Pearl with open blocking dependencies".to_string(),
                ));
            }
            return Ok(());
        }

        // All other transitions are invalid
        Err(Error::InvalidTransition(format!(
            "Cannot transition from {:?} to {:?}",
            self, target
        )))
    }

    /// Returns the list of valid target states for the current status.
    ///
    /// # Arguments
    ///
    /// * `is_blocked` - Whether the Pearl has open blocking dependencies
    ///
    /// # Returns
    ///
    /// Vector of valid target statuses.
    pub fn valid_transitions(&self, is_blocked: bool) -> Vec<Status> {
        let mut transitions = vec![Status::Deferred];

        match self {
            Status::Open => {
                if !is_blocked {
                    transitions.push(Status::InProgress);
                    transitions.push(Status::Closed);
                }
            }
            Status::InProgress => {
                transitions.push(Status::Open);
                if !is_blocked {
                    transitions.push(Status::Closed);
                }
            }
            Status::Blocked => {
                if !is_blocked {
                    transitions.push(Status::Closed);
                }
            }
            Status::Deferred => {
                if !is_blocked {
                    transitions.push(Status::Closed);
                }
            }
            Status::Closed => {
                transitions.push(Status::Open);
            }
        }

        transitions
    }
}

/// Validates a state transition for a Pearl considering the dependency graph.
///
/// # Arguments
///
/// * `pearl` - The Pearl to transition
/// * `new_status` - The target status
/// * `graph` - The dependency graph to check for blocking dependencies
///
/// # Returns
///
/// Ok if the transition is valid, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The transition violates FSM rules
/// - The Pearl has blocking dependencies that prevent the transition
/// - The Pearl is absent from the supplied graph
///
/// Denied transitions return [`Error::TransitionDenied`] with structured details.
pub fn validate_transition(
    pearl: &Pearl,
    new_status: Status,
    graph: &crate::graph::IssueGraph,
) -> Result<()> {
    if graph.pearl(&pearl.id).is_none() {
        return Err(Error::NotFound(pearl.id.clone()));
    }
    let is_blocked = graph.is_blocked(&pearl.id);
    pearl
        .status
        .can_transition_to(new_status, is_blocked)
        .map_err(|_| {
            let dependency_denial = is_blocked
                && (new_status == Status::Closed
                    || (pearl.status == Status::Open && new_status == Status::InProgress));
            let mut blockers: Vec<_> = if dependency_denial {
                graph
                    .blocking_deps(&pearl.id)
                    .into_iter()
                    .map(|task| TransitionBlocker {
                        id: task.id.clone(),
                        status: task.status,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            blockers.sort_by(|a, b| a.id.cmp(&b.id));
            blockers.dedup_by(|a, b| a.id == b.id);
            TransitionDenial {
                task_id: pearl.id.clone(),
                from_status: pearl.status,
                to_status: new_status,
                reason: if dependency_denial {
                    TransitionDenialReason::Blocked
                } else {
                    TransitionDenialReason::Unsupported
                },
                blockers,
            }
            .into()
        })
}
