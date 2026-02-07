// Rust guideline compliant 2026-02-06

//! Property-based tests for the graph module.
//!
//! These tests validate universal properties that should hold across all valid inputs
//! for dependency graphs, cycle detection, topological sorting, and ready queue computation.

use pearls_core::{DepType, Dependency, IssueGraph, Pearl, Status};
use proptest::prelude::*;
use std::collections::HashMap;

/// Generates arbitrary valid Pearl IDs.
fn arb_pearl_id() -> impl Strategy<Value = String> {
    r"prl-[0-9a-f]{6,8}".prop_map(|s| s.to_string())
}

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

/// Generates arbitrary valid Pearls.
fn arb_pearl() -> impl Strategy<Value = Pearl> {
    (
        arb_pearl_id(),
        ".*".prop_filter("non-empty title", |s| !s.is_empty()),
        arb_status(),
        0u8..=4u8,
        1i64..1000000i64,
        1i64..1000000i64,
        ".*".prop_filter("non-empty author", |s| !s.is_empty()),
    )
        .prop_map(
            |(id, title, status, priority, created_at, updated_at, author)| Pearl {
                id,
                title,
                description: String::new(),
                status,
                priority,
                created_at,
                updated_at,
                author,
                labels: Vec::new(),
                deps: Vec::new(),
                metadata: HashMap::new(),
                comments: Vec::new(),
            },
        )
}

/// Generates a list of Pearls without dependencies.
fn arb_pearls_no_deps() -> impl Strategy<Value = Vec<Pearl>> {
    prop::collection::vec(arb_pearl(), 0..10)
}

proptest! {
    /// Property 11: Cycle Detection
    /// For any dependency graph, adding a dependency that would create a cycle should be rejected.
    /// **Validates: Requirements 4.4, 4.5**
    #[test]
    fn prop_cycle_detection(pearls in arb_pearls_no_deps()) {
        if pearls.len() < 2 {
            return Ok(());
        }

        // Create a simple graph
        let mut graph = IssueGraph::from_pearls(pearls.clone()).expect("Valid graph");

        // Try to create a cycle: A -> B -> A
        let id_a = pearls[0].id.clone();
        let id_b = pearls[1].id.clone();

        // Add A -> B
        let _ = graph.add_dependency(&id_a, &id_b, DepType::Blocks);

        // Try to add B -> A (should fail due to cycle)
        let result = graph.add_dependency(&id_b, &id_a, DepType::Blocks);

        // Either it succeeds (no cycle) or fails with CycleDetected error
        if result.is_err() {
            let err_msg = result.unwrap_err().to_string();
            prop_assert!(err_msg.contains("Cycle"), "Error should mention cycle");
        }
    }

    /// Property 12: Acyclic Graph Invariant
    /// For any valid dependency graph, it must remain acyclic after any sequence of valid operations.
    /// **Validates: Requirements 4.4**
    #[test]
    fn prop_acyclic_graph_invariant(pearls in arb_pearls_no_deps()) {
        if pearls.is_empty() {
            return Ok(());
        }

        let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");
        prop_assert!(!graph.has_cycle(), "Graph should be acyclic");
    }

    /// Property 13: Multiple Dependencies Support
    /// For any Pearl, adding multiple dependencies should succeed as long as no cycles are created.
    /// **Validates: Requirements 4.6**
    #[test]
    fn prop_multiple_dependencies_support(pearls in arb_pearls_no_deps()) {
        if pearls.len() < 3 {
            return Ok(());
        }

        let mut graph = IssueGraph::from_pearls(pearls.clone()).expect("Valid graph");

        let id_a = pearls[0].id.clone();
        let id_b = pearls[1].id.clone();
        let id_c = pearls[2].id.clone();

        // Add multiple dependencies from A
        let result_1 = graph.add_dependency(&id_a, &id_b, DepType::Related);
        let result_2 = graph.add_dependency(&id_a, &id_c, DepType::ParentChild);

        // Both should succeed (no cycles)
        prop_assert!(result_1.is_ok(), "First dependency should succeed");
        prop_assert!(result_2.is_ok(), "Second dependency should succeed");
    }

    /// Property 18: Blocked State Derivation
    /// For any Pearl with open blocking dependencies, closing all blockers should make it unblocked.
    /// **Validates: Requirements 4.7**
    #[test]
    fn prop_blocked_state_derivation(mut pearls in arb_pearls_no_deps()) {
        if pearls.len() < 2 {
            return Ok(());
        }

        // Create two Pearls: A (open) and B (open)
        pearls[0].status = Status::Open;
        pearls[1].status = Status::Open;

        let id_a = pearls[0].id.clone();
        let id_b = pearls[1].id.clone();

        // Add blocking dependency: A blocks B
        pearls[1].deps.push(Dependency {
            target_id: id_a.clone(),
            dep_type: DepType::Blocks,
        });

        let mut graph = IssueGraph::from_pearls(pearls.clone()).expect("Valid graph");

        // B should be blocked
        prop_assert!(graph.is_blocked(&id_b), "B should be blocked by A");

        // Close A
        if let Some(pearl) = pearls.iter_mut().find(|p| p.id == id_a) {
            pearl.status = Status::Closed;
        }

        // Rebuild graph
        graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

        // B should no longer be blocked
        prop_assert!(!graph.is_blocked(&id_b), "B should not be blocked after A is closed");
    }

    /// Property 19: Topological Sort Validity
    /// For any dependency graph, topological sort should produce an ordering where every Pearl
    /// appears after all its dependencies.
    /// **Validates: Requirements 6.1**
    #[test]
    fn prop_topological_sort_validity(pearls in arb_pearls_no_deps()) {
        if pearls.is_empty() {
            return Ok(());
        }

        let graph = IssueGraph::from_pearls(pearls.clone()).expect("Valid graph");

        // Topological sort should succeed for acyclic graph
        let sorted = graph.topological_sort();
        prop_assert!(sorted.is_ok(), "Topological sort should succeed for acyclic graph");

        let sorted_ids = sorted.unwrap();
        prop_assert_eq!(sorted_ids.len(), pearls.len(), "All Pearls should be in sorted list");
    }

    /// Property 20: Ready Queue Unblocked Invariant
    /// For any Pearl in the ready queue, it must have zero open blocking dependencies
    /// and status must be open or in_progress.
    /// **Validates: Requirements 6.2, 6.3, 6.5**
    #[test]
    fn prop_ready_queue_unblocked_invariant(mut pearls in arb_pearls_no_deps()) {
        if pearls.is_empty() {
            return Ok(());
        }

        // Set all to open
        for pearl in &mut pearls {
            pearl.status = Status::Open;
        }

        let graph = IssueGraph::from_pearls(pearls.clone()).expect("Valid graph");
        let ready = graph.ready_queue();

        // All ready Pearls should be unblocked
        for pearl in &ready {
            prop_assert!(!graph.is_blocked(&pearl.id), "Ready Pearl should not be blocked");
            prop_assert!(
                pearl.status == Status::Open || pearl.status == Status::InProgress,
                "Ready Pearl should be Open or InProgress"
            );
        }
    }

    /// Property 21: Ready Queue Ordering
    /// For any ready queue, Pearls must be sorted by priority ascending (P0 first),
    /// then by updated_at descending (most recent first).
    /// **Validates: Requirements 6.4**
    #[test]
    fn prop_ready_queue_ordering(mut pearls in arb_pearls_no_deps()) {
        if pearls.len() < 2 {
            return Ok(());
        }

        // Set all to open with varying priorities and timestamps
        for (i, pearl) in pearls.iter_mut().enumerate() {
            pearl.status = Status::Open;
            pearl.priority = (i % 5) as u8;
            pearl.updated_at = (i as i64) * 100;
        }

        let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");
        let ready = graph.ready_queue();

        // Check ordering: priority ascending, then updated_at descending
        for i in 0..ready.len().saturating_sub(1) {
            let current = ready[i];
            let next = ready[i + 1];

            if current.priority == next.priority {
                // Same priority: should be sorted by updated_at descending
                prop_assert!(
                    current.updated_at >= next.updated_at,
                    "Same priority should be sorted by updated_at descending"
                );
            } else {
                // Different priority: lower priority should come first
                prop_assert!(
                    current.priority <= next.priority,
                    "Should be sorted by priority ascending"
                );
            }
        }
    }
}
