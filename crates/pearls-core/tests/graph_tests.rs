// Rust guideline compliant 2026-02-06

//! Unit tests for the graph module.
//!
//! These tests validate specific examples, edge cases, and error conditions
//! for dependency graphs, cycle detection, and ready queue computation.

use pearls_core::{DepType, Dependency, IssueGraph, Pearl, Status};
use std::collections::HashMap;

/// Helper to create a Pearl with given ID and status.
fn create_pearl(id: &str, status: Status) -> Pearl {
    Pearl {
        id: id.to_string(),
        title: format!("Pearl {}", id),
        description: String::new(),
        status,
        priority: 2,
        created_at: 1000,
        updated_at: 1000,
        author: "test".to_string(),
        labels: Vec::new(),
        deps: Vec::new(),
        metadata: HashMap::new(),
    }
}

#[test]
fn test_empty_graph() {
    let pearls = vec![];
    let graph = IssueGraph::from_pearls(pearls).expect("Empty graph should be valid");

    assert!(!graph.has_cycle(), "Empty graph should not have cycles");
    assert!(
        graph.ready_queue().is_empty(),
        "Empty graph should have empty ready queue"
    );
}

#[test]
fn test_single_node_graph() {
    let pearls = vec![create_pearl("prl-a1b2c3", Status::Open)];
    let graph = IssueGraph::from_pearls(pearls).expect("Single node graph should be valid");

    assert!(
        !graph.has_cycle(),
        "Single node graph should not have cycles"
    );
    assert_eq!(
        graph.ready_queue().len(),
        1,
        "Single open Pearl should be in ready queue"
    );
}

#[test]
fn test_single_node_closed() {
    let pearls = vec![create_pearl("prl-a1b2c3", Status::Closed)];
    let graph = IssueGraph::from_pearls(pearls).expect("Single closed Pearl should be valid");

    assert!(
        graph.ready_queue().is_empty(),
        "Closed Pearl should not be in ready queue"
    );
}

#[test]
fn test_disconnected_components() {
    let pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
        create_pearl("prl-g7h8i9", Status::Open),
    ];

    let graph = IssueGraph::from_pearls(pearls).expect("Disconnected graph should be valid");

    assert!(
        !graph.has_cycle(),
        "Disconnected graph should not have cycles"
    );
    assert_eq!(
        graph.ready_queue().len(),
        3,
        "All open Pearls should be in ready queue"
    );
}

#[test]
fn test_simple_dependency_chain() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
        create_pearl("prl-g7h8i9", Status::Open),
    ];

    // Create chain: A -> B -> C (A blocks B, B blocks C)
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });
    pearls[2].deps.push(Dependency {
        target_id: "prl-d4e5f6".to_string(),
        dep_type: DepType::Blocks,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Dependency chain should be valid");

    assert!(
        !graph.has_cycle(),
        "Dependency chain should not have cycles"
    );

    // Only A should be in ready queue (B and C are blocked)
    let ready = graph.ready_queue();
    assert_eq!(
        ready.len(),
        1,
        "Only unblocked Pearl should be in ready queue"
    );
    assert_eq!(ready[0].id, "prl-a1b2c3", "A should be in ready queue");
}

#[test]
fn test_blocking_dependency_detection() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
    ];

    // B is blocked by A
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    assert!(graph.is_blocked("prl-d4e5f6"), "B should be blocked by A");
    assert!(!graph.is_blocked("prl-a1b2c3"), "A should not be blocked");
}

#[test]
fn test_blocking_deps_list() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
        create_pearl("prl-g7h8i9", Status::Open),
    ];

    // C is blocked by both A and B
    pearls[2].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });
    pearls[2].deps.push(Dependency {
        target_id: "prl-d4e5f6".to_string(),
        dep_type: DepType::Blocks,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    let blockers = graph.blocking_deps("prl-g7h8i9");
    assert_eq!(blockers.len(), 2, "C should have 2 blockers");
}

#[test]
fn test_non_blocking_dependencies_dont_block() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
    ];

    // B has a "related" dependency on A (not blocking)
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Related,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    assert!(
        !graph.is_blocked("prl-d4e5f6"),
        "Related dependency should not block"
    );
}

#[test]
fn test_closed_blocker_doesnt_block() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Closed),
        create_pearl("prl-d4e5f6", Status::Open),
    ];

    // B is blocked by A, but A is closed
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    assert!(
        !graph.is_blocked("prl-d4e5f6"),
        "Closed blocker should not block"
    );
}

#[test]
fn test_ready_queue_excludes_closed() {
    let pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Closed),
        create_pearl("prl-g7h8i9", Status::Open),
    ];

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");
    let ready = graph.ready_queue();

    assert_eq!(ready.len(), 2, "Closed Pearl should not be in ready queue");
    assert!(
        !ready.iter().any(|p| p.status == Status::Closed),
        "No closed Pearls in ready queue"
    );
}

#[test]
fn test_ready_queue_excludes_deferred() {
    let pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Deferred),
        create_pearl("prl-g7h8i9", Status::Open),
    ];

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");
    let ready = graph.ready_queue();

    assert_eq!(
        ready.len(),
        2,
        "Deferred Pearl should not be in ready queue"
    );
    assert!(
        !ready.iter().any(|p| p.status == Status::Deferred),
        "No deferred Pearls in ready queue"
    );
}

#[test]
fn test_ready_queue_priority_ordering() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
        create_pearl("prl-g7h8i9", Status::Open),
    ];

    // Set different priorities
    pearls[0].priority = 3; // Low priority
    pearls[1].priority = 0; // Critical
    pearls[2].priority = 1; // High

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");
    let ready = graph.ready_queue();

    // Should be sorted: P0, P1, P3
    assert_eq!(ready[0].priority, 0, "P0 should be first");
    assert_eq!(ready[1].priority, 1, "P1 should be second");
    assert_eq!(ready[2].priority, 3, "P3 should be third");
}

#[test]
fn test_ready_queue_timestamp_ordering() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
        create_pearl("prl-g7h8i9", Status::Open),
    ];

    // Same priority, different timestamps
    for pearl in &mut pearls {
        pearl.priority = 2;
    }
    pearls[0].updated_at = 1000;
    pearls[1].updated_at = 3000; // Most recent
    pearls[2].updated_at = 2000;

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");
    let ready = graph.ready_queue();

    // Should be sorted by updated_at descending: 3000, 2000, 1000
    assert_eq!(ready[0].updated_at, 3000, "Most recent should be first");
    assert_eq!(
        ready[1].updated_at, 2000,
        "Second most recent should be second"
    );
    assert_eq!(ready[2].updated_at, 1000, "Oldest should be last");
}

#[test]
fn test_topological_sort_simple() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
    ];

    // B depends on A (A blocks B)
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");
    let sorted = graph.topological_sort().expect("Should succeed");

    assert_eq!(sorted.len(), 2, "Should have 2 Pearls");
    // Verify both Pearls are in the sorted list
    assert!(
        sorted.contains(&"prl-a1b2c3".to_string()),
        "A should be in sorted list"
    );
    assert!(
        sorted.contains(&"prl-d4e5f6".to_string()),
        "B should be in sorted list"
    );
}

#[test]
fn test_add_dependency_success() {
    let pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
    ];

    let mut graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    let result = graph.add_dependency("prl-a1b2c3", "prl-d4e5f6", DepType::Blocks);
    assert!(result.is_ok(), "Adding dependency should succeed");
}

#[test]
fn test_add_dependency_nonexistent_pearl() {
    let pearls = vec![create_pearl("prl-a1b2c3", Status::Open)];

    let mut graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    let result = graph.add_dependency("prl-a1b2c3", "prl-nonexistent", DepType::Blocks);
    assert!(
        result.is_err(),
        "Adding dependency to nonexistent Pearl should fail"
    );
}

#[test]
fn test_remove_dependency() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
    ];

    // Add dependency
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });

    let mut graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    // B should be blocked
    assert!(graph.is_blocked("prl-d4e5f6"), "B should be blocked");

    // Remove dependency
    let result = graph.remove_dependency("prl-d4e5f6", "prl-a1b2c3");
    assert!(result.is_ok(), "Removing dependency should succeed");

    // B should no longer be blocked
    assert!(
        !graph.is_blocked("prl-d4e5f6"),
        "B should not be blocked after removing dependency"
    );
}

#[test]
fn test_complex_dependency_chain() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
        create_pearl("prl-g7h8i9", Status::Open),
        create_pearl("prl-j0k1l2", Status::Open),
    ];

    // Create chain: A -> B -> C -> D
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });
    pearls[2].deps.push(Dependency {
        target_id: "prl-d4e5f6".to_string(),
        dep_type: DepType::Blocks,
    });
    pearls[3].deps.push(Dependency {
        target_id: "prl-g7h8i9".to_string(),
        dep_type: DepType::Blocks,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    // Only A should be ready
    let ready = graph.ready_queue();
    assert_eq!(ready.len(), 1, "Only A should be ready");
    assert_eq!(ready[0].id, "prl-a1b2c3", "A should be ready");

    // Check blocking relationships
    assert!(!graph.is_blocked("prl-a1b2c3"), "A should not be blocked");
    assert!(graph.is_blocked("prl-d4e5f6"), "B should be blocked");
    assert!(graph.is_blocked("prl-g7h8i9"), "C should be blocked");
    assert!(graph.is_blocked("prl-j0k1l2"), "D should be blocked");
}

#[test]
fn test_multiple_independent_chains() {
    let mut pearls = vec![
        create_pearl("prl-a1b2c3", Status::Open),
        create_pearl("prl-d4e5f6", Status::Open),
        create_pearl("prl-g7h8i9", Status::Open),
        create_pearl("prl-j0k1l2", Status::Open),
    ];

    // Chain 1: A -> B
    pearls[1].deps.push(Dependency {
        target_id: "prl-a1b2c3".to_string(),
        dep_type: DepType::Blocks,
    });

    // Chain 2: C -> D
    pearls[3].deps.push(Dependency {
        target_id: "prl-g7h8i9".to_string(),
        dep_type: DepType::Blocks,
    });

    let graph = IssueGraph::from_pearls(pearls).expect("Valid graph");

    // A and C should be ready
    let ready = graph.ready_queue();
    assert_eq!(ready.len(), 2, "A and C should be ready");
}
