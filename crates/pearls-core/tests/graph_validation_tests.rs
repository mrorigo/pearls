// Rust guideline compliant 2026-10-15

//! Complete-graph validation and structured transition behavior.

use pearls_core::{
    validate_transition, DepType, Dependency, Error, GraphError, IssueGraph, Pearl, Status,
    TransitionDenialReason,
};

fn task(index: usize, status: Status) -> Pearl {
    let mut pearl = Pearl::new(format!("Task {index}"), "tester".to_string());
    pearl.id = format!("prl-{index:06x}");
    pearl.status = status;
    pearl
}

fn dependency(target: usize, dep_type: DepType) -> Dependency {
    Dependency {
        target_id: format!("prl-{target:06x}"),
        dep_type,
    }
}

fn assert_cycle(error: Error, tasks: &[Pearl]) -> Vec<String> {
    let Error::CycleDetected(cycle) = error else {
        panic!("Expected a blocking cycle, got {error}");
    };
    assert!(cycle.len() >= 2);
    assert_eq!(cycle.first(), cycle.last());
    for pair in cycle.windows(2) {
        let from = tasks.iter().find(|task| task.id == pair[0]).unwrap();
        assert!(from
            .deps
            .iter()
            .any(|dep| dep.target_id == pair[1] && dep.dep_type == DepType::Blocks));
    }
    cycle
}

#[test]
fn nonblocking_cycles_preserve_relationships_without_restricting_scheduling() {
    for dep_type in [
        DepType::Related,
        DepType::ParentChild,
        DepType::DiscoveredFrom,
    ] {
        let mut first = task(1, Status::Open);
        let mut second = task(2, Status::Open);
        first.deps.push(dependency(2, dep_type));
        second.deps.push(dependency(1, dep_type));
        for graph in [
            IssueGraph::from_pearls(vec![first.clone(), second.clone()]).unwrap(),
            IssueGraph::from_pearls_strict(vec![first.clone(), second.clone()]).unwrap(),
        ] {
            assert!(!graph.has_cycle());
            assert!(graph.find_cycle().is_none());
            assert_eq!(graph.topological_sort().unwrap().len(), 2);
            assert_eq!(graph.ready_queue().len(), 2);
            assert_eq!(
                graph.dependencies_by_type(&first.id, dep_type)[0].id,
                second.id
            );
            assert_eq!(
                graph.dependencies_by_type(&second.id, dep_type)[0].id,
                first.id
            );
        }
    }
}

#[test]
fn mixed_relationship_cycle_has_blocking_prerequisites_first() {
    let mut first = task(1, Status::Open);
    let mut second = task(2, Status::Open);
    first.deps.push(dependency(2, DepType::Blocks));
    second.deps.push(dependency(1, DepType::Related));
    let graph = IssueGraph::from_pearls_strict(vec![first.clone(), second.clone()]).unwrap();
    assert!(graph.is_blocked(&first.id));
    assert!(!graph.is_blocked(&second.id));
    assert_eq!(graph.topological_sort().unwrap(), vec![second.id, first.id]);
}

#[test]
fn blocking_cycle_reports_a_closed_path_of_actual_edges() {
    let mut tasks = vec![
        task(1, Status::Open),
        task(2, Status::Open),
        task(3, Status::Open),
    ];
    tasks[0].deps.push(dependency(2, DepType::Blocks));
    tasks[1].deps.push(dependency(3, DepType::Blocks));
    tasks[2].deps.push(dependency(1, DepType::Blocks));
    let error = IssueGraph::from_pearls_strict(tasks.clone())
        .err()
        .expect("cycle must fail");
    assert_eq!(assert_cycle(error, &tasks).len(), 4);
}

#[test]
fn blocking_self_dependency_reports_the_task_twice() {
    let mut first = task(1, Status::Open);
    first.deps.push(dependency(1, DepType::Blocks));
    let error = IssueGraph::from_pearls_strict(vec![first.clone()])
        .err()
        .unwrap();
    assert_eq!(
        assert_cycle(error, &[first.clone()]),
        vec![first.id.clone(), first.id]
    );
}

#[test]
fn deep_graph_cycle_detection_does_not_use_recursive_traversal() {
    let mut tasks: Vec<_> = (0..10_000).map(|index| task(index, Status::Open)).collect();
    for (index, task) in tasks.iter_mut().enumerate().take(9_999) {
        task.deps.push(dependency(index + 1, DepType::Blocks));
    }
    let graph = IssueGraph::from_pearls_strict(tasks.clone()).unwrap();
    assert_eq!(graph.topological_sort().unwrap().len(), 10_000);
    tasks[9_999].deps.push(dependency(0, DepType::Blocks));
    let error = IssueGraph::from_pearls_strict(tasks.clone()).err().unwrap();
    let Error::CycleDetected(cycle) = error else {
        panic!("expected cycle")
    };
    assert_eq!(cycle.len(), 10_001);
    assert_eq!(cycle.first(), cycle.last());
}

#[test]
fn strict_graph_rejects_missing_endpoints_for_every_relationship_type() {
    for dep_type in [
        DepType::Blocks,
        DepType::Related,
        DepType::ParentChild,
        DepType::DiscoveredFrom,
    ] {
        let mut first = task(1, Status::Open);
        first.deps.push(dependency(2, dep_type));
        let error = IssueGraph::from_pearls_strict(vec![first.clone()])
            .err()
            .unwrap();
        assert!(
            matches!(error, Error::InvalidGraph(GraphError::MissingEndpoint {
            from_id, to_id, dep_type: actual_type,
        }) if from_id == first.id && to_id == "prl-000002" && actual_type == dep_type)
        );
        // Existing active-only readers remain compatible until service migration.
        let legacy = IssueGraph::from_pearls(vec![first.clone()]).unwrap();
        assert_eq!(legacy.pearl(&first.id).unwrap().deps, first.deps);
    }
}

#[test]
fn strict_graph_rejects_duplicate_ids_and_invalid_tasks() {
    let first = task(1, Status::Open);
    let mut duplicate = first.clone();
    duplicate.title = "Conflicting title".into();
    assert!(
        matches!(IssueGraph::from_pearls_strict(vec![first.clone(), duplicate]),
        Err(Error::InvalidGraph(GraphError::DuplicateId { id })) if id == first.id)
    );
    let mut invalid = first;
    invalid.title.clear();
    assert!(matches!(
        IssueGraph::from_pearls_strict(vec![invalid]),
        Err(Error::InvalidPearl(_))
    ));
}

#[test]
fn closed_archived_endpoint_satisfies_blocking_dependency() {
    let mut active = task(1, Status::Open);
    active.deps.push(dependency(2, DepType::Blocks));
    let archived = task(2, Status::Closed);
    let graph = IssueGraph::from_pearls_strict(vec![active.clone(), archived.clone()]).unwrap();
    assert_eq!(graph.pearl(&archived.id).unwrap(), &archived);
    assert!(!graph.is_blocked(&active.id));
    assert!(validate_transition(&active, Status::InProgress, &graph).is_ok());
    assert!(validate_transition(&active, Status::Closed, &graph).is_ok());
    assert!(matches!(
        IssueGraph::from_pearls_strict(vec![active]),
        Err(Error::InvalidGraph(_))
    ));
}

#[test]
fn rejected_blocking_addition_preserves_the_nonblocking_relationship() {
    let mut first = task(1, Status::Open);
    let second = task(2, Status::Open);
    first.deps.push(dependency(2, DepType::Blocks));
    let mut graph = IssueGraph::from_pearls_strict(vec![first.clone(), second.clone()]).unwrap();
    graph
        .add_dependency(&second.id, &first.id, DepType::Related)
        .unwrap();
    let error = graph
        .add_dependency(&second.id, &first.id, DepType::Blocks)
        .unwrap_err();
    let mut proposed_second = second.clone();
    proposed_second.deps.push(dependency(1, DepType::Blocks));
    assert_cycle(error, &[first.clone(), proposed_second]);
    assert!(!graph.has_cycle());
    assert!(graph
        .dependencies_by_type(&second.id, DepType::Blocks)
        .is_empty());
    assert_eq!(
        graph
            .dependencies_by_type(&second.id, DepType::Related)
            .len(),
        1
    );
}

#[test]
fn removing_dependencies_removes_all_parallel_blocking_edges() {
    let first = task(1, Status::Open);
    let second = task(2, Status::Open);
    let mut graph = IssueGraph::from_pearls_strict(vec![first.clone(), second.clone()]).unwrap();
    for _ in 0..2 {
        graph
            .add_dependency(&first.id, &second.id, DepType::Blocks)
            .unwrap();
    }
    graph
        .add_dependency(&first.id, &second.id, DepType::Related)
        .unwrap();
    graph.remove_dependency(&first.id, &second.id).unwrap();
    assert!(graph.pearl(&first.id).unwrap().deps.is_empty());
    graph
        .add_dependency(&second.id, &first.id, DepType::Blocks)
        .expect("no stale edge remains");
}

#[test]
fn denied_transition_has_sorted_unique_blockers_and_statuses() {
    let mut first = task(1, Status::Open);
    first.deps = vec![
        dependency(4, DepType::Blocks),
        dependency(3, DepType::Blocks),
        dependency(2, DepType::Blocks),
        dependency(2, DepType::Blocks),
        dependency(5, DepType::Blocks),
        dependency(6, DepType::Related),
    ];
    let graph = IssueGraph::from_pearls_strict(vec![
        first.clone(),
        task(2, Status::Open),
        task(3, Status::Deferred),
        task(4, Status::InProgress),
        task(5, Status::Closed),
        task(6, Status::Open),
    ])
    .unwrap();
    for target in [Status::InProgress, Status::Closed] {
        let Error::TransitionDenied(denial) =
            validate_transition(&first, target, &graph).unwrap_err()
        else {
            panic!("expected structured denial")
        };
        assert_eq!(denial.task_id, first.id);
        assert_eq!(denial.from_status, Status::Open);
        assert_eq!(denial.to_status, target);
        assert_eq!(denial.reason, TransitionDenialReason::Blocked);
        assert_eq!(
            denial
                .blockers
                .iter()
                .map(|blocker| (blocker.id.as_str(), blocker.status))
                .collect::<Vec<_>>(),
            vec![
                ("prl-000002", Status::Open),
                ("prl-000003", Status::Deferred),
                ("prl-000004", Status::InProgress)
            ]
        );
    }
}

#[test]
fn unsupported_transition_does_not_misrepresent_blockers_as_the_cause() {
    let mut first = task(1, Status::Deferred);
    first.deps.push(dependency(2, DepType::Blocks));
    let graph = IssueGraph::from_pearls_strict(vec![first.clone(), task(2, Status::Open)]).unwrap();
    let Error::TransitionDenied(denial) =
        validate_transition(&first, Status::InProgress, &graph).unwrap_err()
    else {
        panic!("expected structured denial")
    };
    assert_eq!(denial.reason, TransitionDenialReason::Unsupported);
    assert!(denial.blockers.is_empty());
}

#[test]
fn graph_aware_validation_preserves_existing_status_rules() {
    let statuses = [
        Status::Open,
        Status::InProgress,
        Status::Blocked,
        Status::Deferred,
        Status::Closed,
    ];
    for current in statuses {
        for blocker_status in statuses {
            let mut first = task(1, current);
            first.deps.push(dependency(2, DepType::Blocks));
            let graph =
                IssueGraph::from_pearls_strict(vec![first.clone(), task(2, blocker_status)])
                    .unwrap();
            for target in statuses {
                assert_eq!(
                    validate_transition(&first, target, &graph).is_ok(),
                    current
                        .can_transition_to(target, blocker_status != Status::Closed)
                        .is_ok(),
                    "{current:?} -> {target:?}, blocker={blocker_status:?}"
                );
            }
        }
    }
}

#[test]
fn transition_of_unknown_task_is_not_assumed_unblocked() {
    let graph = IssueGraph::from_pearls_strict(Vec::new()).unwrap();
    let first = task(1, Status::Open);
    assert!(
        matches!(validate_transition(&first, Status::Closed, &graph), Err(Error::NotFound(id)) if id == first.id)
    );
}

#[test]
fn graph_validation_inside_transaction_rejects_complete_batch_without_receipt() {
    use pearls_core::repository::{OperationRequest, RepositoryError, RepositoryStore};
    use std::collections::BTreeMap;
    let root = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(root.path().join(".pearls")).unwrap();
    let store = RepositoryStore::open(root.path()).unwrap();
    let before = store.snapshot().unwrap();
    let request = OperationRequest {
        operation_key: "cyclic-import".into(),
        request_digest: "digest".into(),
        expected_revision: before.revision.clone(),
    };
    let result: Result<_, RepositoryError> = store.transact(&request, |data| {
        let mut first = task(1, Status::Open);
        let mut second = task(2, Status::Open);
        first.deps.push(dependency(2, DepType::Blocks));
        second.deps.push(dependency(1, DepType::Blocks));
        data.active.extend([first, second]);
        IssueGraph::from_pearls_strict(
            data.active.iter().chain(&data.archived).cloned().collect(),
        )?;
        Ok(BTreeMap::new())
    });
    assert!(matches!(
        result,
        Err(RepositoryError::InvalidTask(Error::CycleDetected(_)))
    ));
    assert_eq!(store.snapshot().unwrap(), before);
    assert!(store.receipt("cyclic-import").unwrap().is_none());
}

#[test]
fn every_three_task_graph_agrees_with_reference_cycle_detection() {
    for mask in 0..512 {
        let mut tasks: Vec<_> = (0..3).map(|index| task(index, Status::Open)).collect();
        let mut reference = petgraph::graph::DiGraph::<(), ()>::new();
        let nodes: Vec<_> = (0..3).map(|_| reference.add_node(())).collect();
        for (from, pearl) in tasks.iter_mut().enumerate() {
            for to in 0..3 {
                if mask & (1 << (from * 3 + to)) != 0 {
                    pearl.deps.push(dependency(to, DepType::Blocks));
                    reference.add_edge(nodes[from], nodes[to], ());
                }
            }
        }
        let cyclic = petgraph::algo::is_cyclic_directed(&reference);
        match IssueGraph::from_pearls_strict(tasks.clone()) {
            Ok(graph) => {
                assert!(!cyclic, "Cycle accepted for edge mask {mask}");
                let sorted = graph.topological_sort().unwrap();
                for pearl in &tasks {
                    let position = sorted.iter().position(|id| *id == pearl.id).unwrap();
                    for dep in &pearl.deps {
                        let dependency_position =
                            sorted.iter().position(|id| *id == dep.target_id).unwrap();
                        assert!(
                            dependency_position < position,
                            "Wrong prerequisite order for edge mask {mask}"
                        );
                    }
                }
            }
            Err(error) => {
                assert!(cyclic, "Acyclic edge mask {mask} was rejected: {error}");
                assert_cycle(error, &tasks);
            }
        }
    }
}
