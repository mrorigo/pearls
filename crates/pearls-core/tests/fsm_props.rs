// Rust guideline compliant 2026-02-06

//! Property-based tests for the FSM module.
//!
//! These tests validate universal properties that should hold across all valid inputs
//! for state transitions, blocking dependencies, and FSM rules.

use pearls_core::Status;
use proptest::prelude::*;

/// Generates arbitrary Status values.
fn arb_status() -> impl Strategy<Value = Status> {
    prop_oneof![
        Just(Status::Open),
        Just(Status::InProgress),
        Just(Status::Blocked),
        Just(Status::Deferred),
        Just(Status::Closed),
    ]
}

proptest! {
    /// Property 14: Valid Transition Enforcement
    /// For any Pearl and target status, a transition should succeed only if it follows
    /// the valid transition rules.
    /// **Validates: Requirements 5.1, 5.6**
    #[test]
    fn prop_valid_transition_enforcement(
        current_status in arb_status(),
        is_blocked in any::<bool>(),
    ) {
        // Deferred should always be reachable
        assert!(current_status.can_transition_to(Status::Deferred, is_blocked).is_ok());

        // Closed should be reachable unless blocked
        let closed_result = current_status.can_transition_to(Status::Closed, is_blocked);
        if is_blocked {
            assert!(closed_result.is_err(), "Cannot close blocked Pearl");
        } else {
            assert!(closed_result.is_ok(), "Should be able to close unblocked Pearl");
        }

        // Open → InProgress should fail if blocked
        if current_status == Status::Open {
            let result = current_status.can_transition_to(Status::InProgress, is_blocked);
            if is_blocked {
                assert!(result.is_err(), "Cannot start blocked Pearl");
            } else {
                assert!(result.is_ok(), "Should be able to start unblocked Pearl");
            }
        }
    }

    /// Property 16: Deferred Transition Universality
    /// For any Pearl in any status, transitioning to deferred should always succeed.
    /// **Validates: Requirements 5.4**
    #[test]
    fn prop_deferred_transition_universality(
        current_status in arb_status(),
        is_blocked in any::<bool>(),
    ) {
        // Deferred should always be reachable from any state
        assert!(
            current_status.can_transition_to(Status::Deferred, is_blocked).is_ok(),
            "Should always be able to transition to Deferred from {:?}",
            current_status
        );
    }

    /// Property 17: Reopen Capability
    /// For any closed Pearl, transitioning to open should always succeed.
    /// **Validates: Requirements 5.5**
    #[test]
    fn prop_reopen_capability(is_blocked in any::<bool>()) {
        // Closed → Open should always succeed
        assert!(
            Status::Closed.can_transition_to(Status::Open, is_blocked).is_ok(),
            "Should always be able to reopen a closed Pearl"
        );
    }

    /// Property 14 (extended): Valid transitions list should match can_transition_to
    /// For any status and blocking state, the valid_transitions list should contain
    /// exactly those statuses that can_transition_to returns Ok for.
    /// **Validates: Requirements 5.1**
    #[test]
    fn prop_valid_transitions_consistency(
        current_status in arb_status(),
        is_blocked in any::<bool>(),
    ) {
        let valid_list = current_status.valid_transitions(is_blocked);

        // Check that all statuses in valid_list can actually transition
        for target in &valid_list {
            assert!(
                current_status.can_transition_to(*target, is_blocked).is_ok(),
                "valid_transitions returned {:?} but can_transition_to failed",
                target
            );
        }

        // Check that all other statuses cannot transition (except those that should always work)
        for target in &[Status::Open, Status::InProgress, Status::Blocked, Status::Deferred, Status::Closed] {
            if !valid_list.contains(target) && *target != current_status {
                assert!(
                    current_status.can_transition_to(*target, is_blocked).is_err(),
                    "can_transition_to succeeded for {:?} but it's not in valid_transitions",
                    target
                );
            }
        }
    }
}
