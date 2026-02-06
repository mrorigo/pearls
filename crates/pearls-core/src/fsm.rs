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
pub fn validate_transition(
    pearl: &Pearl,
    new_status: Status,
    graph: &crate::graph::IssueGraph,
) -> Result<()> {
    let is_blocked = graph.is_blocked(&pearl.id);
    pearl.status.can_transition_to(new_status, is_blocked)
}
