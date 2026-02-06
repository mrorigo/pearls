// Rust guideline compliant 2026-02-06

//! Unit tests for the FSM module.
//!
//! These tests validate specific examples, edge cases, and error conditions
//! for state transitions and FSM rules.

use pearls_core::Status;

#[test]
fn test_open_to_in_progress_unblocked() {
    let status = Status::Open;
    assert!(
        status.can_transition_to(Status::InProgress, false).is_ok(),
        "Should allow Open → InProgress when unblocked"
    );
}

#[test]
fn test_open_to_in_progress_blocked() {
    let status = Status::Open;
    assert!(
        status.can_transition_to(Status::InProgress, true).is_err(),
        "Should reject Open → InProgress when blocked"
    );
}

#[test]
fn test_in_progress_to_closed_unblocked() {
    let status = Status::InProgress;
    assert!(
        status.can_transition_to(Status::Closed, false).is_ok(),
        "Should allow InProgress → Closed when unblocked"
    );
}

#[test]
fn test_in_progress_to_closed_blocked() {
    let status = Status::InProgress;
    assert!(
        status.can_transition_to(Status::Closed, true).is_err(),
        "Should reject InProgress → Closed when blocked"
    );
}

#[test]
fn test_in_progress_to_open() {
    let status = Status::InProgress;
    assert!(
        status.can_transition_to(Status::Open, false).is_ok(),
        "Should allow InProgress → Open"
    );
    assert!(
        status.can_transition_to(Status::Open, true).is_ok(),
        "Should allow InProgress → Open even when blocked"
    );
}

#[test]
fn test_any_to_deferred() {
    for status in &[
        Status::Open,
        Status::InProgress,
        Status::Blocked,
        Status::Deferred,
        Status::Closed,
    ] {
        assert!(
            status.can_transition_to(Status::Deferred, false).is_ok(),
            "Should allow {:?} → Deferred when unblocked",
            status
        );
        assert!(
            status.can_transition_to(Status::Deferred, true).is_ok(),
            "Should allow {:?} → Deferred when blocked",
            status
        );
    }
}

#[test]
fn test_closed_to_open() {
    let status = Status::Closed;
    assert!(
        status.can_transition_to(Status::Open, false).is_ok(),
        "Should allow Closed → Open when unblocked"
    );
    assert!(
        status.can_transition_to(Status::Open, true).is_ok(),
        "Should allow Closed → Open even when blocked"
    );
}

#[test]
fn test_invalid_transition_open_to_closed_blocked() {
    let status = Status::Open;
    let result = status.can_transition_to(Status::Closed, true);
    assert!(result.is_err(), "Should reject Open → Closed when blocked");
    assert!(
        result.unwrap_err().to_string().contains("blocking"),
        "Error message should mention blocking"
    );
}

#[test]
fn test_invalid_transition_open_to_blocked() {
    let status = Status::Open;
    let result = status.can_transition_to(Status::Blocked, false);
    assert!(
        result.is_err(),
        "Should reject Open → Blocked (not a valid transition)"
    );
}

#[test]
fn test_invalid_transition_deferred_to_in_progress_blocked() {
    let status = Status::Deferred;
    let result = status.can_transition_to(Status::InProgress, true);
    assert!(
        result.is_err(),
        "Should reject Deferred → InProgress when blocked"
    );
}

#[test]
fn test_valid_transitions_open_unblocked() {
    let status = Status::Open;
    let transitions = status.valid_transitions(false);
    assert!(
        transitions.contains(&Status::InProgress),
        "Open unblocked should allow InProgress"
    );
    assert!(
        transitions.contains(&Status::Deferred),
        "Open should always allow Deferred"
    );
    assert!(
        transitions.contains(&Status::Closed),
        "Open unblocked should allow Closed"
    );
}

#[test]
fn test_valid_transitions_open_blocked() {
    let status = Status::Open;
    let transitions = status.valid_transitions(true);
    assert!(
        !transitions.contains(&Status::InProgress),
        "Open blocked should not allow InProgress"
    );
    assert!(
        transitions.contains(&Status::Deferred),
        "Open should always allow Deferred"
    );
    assert!(
        !transitions.contains(&Status::Closed),
        "Open blocked should not allow Closed"
    );
}

#[test]
fn test_valid_transitions_in_progress_unblocked() {
    let status = Status::InProgress;
    let transitions = status.valid_transitions(false);
    assert!(
        transitions.contains(&Status::Open),
        "InProgress should allow Open"
    );
    assert!(
        transitions.contains(&Status::Closed),
        "InProgress unblocked should allow Closed"
    );
    assert!(
        transitions.contains(&Status::Deferred),
        "InProgress should always allow Deferred"
    );
}

#[test]
fn test_valid_transitions_in_progress_blocked() {
    let status = Status::InProgress;
    let transitions = status.valid_transitions(true);
    assert!(
        transitions.contains(&Status::Open),
        "InProgress should allow Open"
    );
    assert!(
        !transitions.contains(&Status::Closed),
        "InProgress blocked should not allow Closed"
    );
    assert!(
        transitions.contains(&Status::Deferred),
        "InProgress should always allow Deferred"
    );
}

#[test]
fn test_valid_transitions_closed() {
    let status = Status::Closed;
    let transitions = status.valid_transitions(false);
    assert!(
        transitions.contains(&Status::Open),
        "Closed should allow Open (reopen)"
    );
    assert!(
        transitions.contains(&Status::Deferred),
        "Closed should allow Deferred"
    );
}

#[test]
fn test_error_message_blocked_transition() {
    let status = Status::Open;
    let result = status.can_transition_to(Status::InProgress, true);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("blocking"),
        "Error message should explain blocking: {}",
        error_msg
    );
}

#[test]
fn test_error_message_invalid_transition() {
    let status = Status::Open;
    let result = status.can_transition_to(Status::Blocked, false);
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("transition"),
        "Error message should mention transition: {}",
        error_msg
    );
}

#[test]
fn test_transition_consistency_all_statuses() {
    // For each status and blocking state, verify that valid_transitions
    // matches can_transition_to results
    for status in &[
        Status::Open,
        Status::InProgress,
        Status::Deferred,
        Status::Closed,
    ] {
        for is_blocked in &[false, true] {
            let valid_list = status.valid_transitions(*is_blocked);

            // All statuses in valid_list should succeed
            for target in &valid_list {
                assert!(
                    status.can_transition_to(*target, *is_blocked).is_ok(),
                    "valid_transitions returned {:?} but can_transition_to failed for {:?} (blocked={})",
                    target,
                    status,
                    is_blocked
                );
            }
        }
    }
}
