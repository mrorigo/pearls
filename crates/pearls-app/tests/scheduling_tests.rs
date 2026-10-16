// Rust guideline compliant 2026-10-15

//! Selected snapshot and explicit scheduling contracts.

use pearls_app::{service::*, ErrorCode};
use pearls_core::{DepType, Dependency, Pearl, Status};
use serde_json::json;
use std::collections::BTreeSet;
use tempfile::TempDir;

fn task(number: u32, status: Status) -> Pearl {
    let mut task = Pearl::new(format!("Task {number}"), "tester".into());
    task.id = format!("prl-{number:06x}");
    task.status = status;
    task.created_at = 100;
    task.updated_at = 200;
    task
}

fn dependency(target: u32, dep_type: DepType) -> Dependency {
    Dependency {
        target_id: format!("prl-{target:06x}"),
        dep_type,
    }
}

fn write(path: &std::path::Path, tasks: &[Pearl]) {
    let content: String = tasks
        .iter()
        .map(|task| format!("{}\n", serde_json::to_string(task).unwrap()))
        .collect();
    std::fs::write(path, content).unwrap();
}

fn repository(active: &[Pearl], archived: &[Pearl]) -> (TempDir, GraphService) {
    let directory = TempDir::new().unwrap();
    std::fs::create_dir(directory.path().join(".pearls")).unwrap();
    write(&directory.path().join(".pearls/issues.jsonl"), active);
    write(&directory.path().join(".pearls/archive.jsonl"), archived);
    let service = GraphService::open(directory.path()).unwrap();
    (directory, service)
}

fn selection(numbers: &[u32]) -> TaskSelection {
    TaskSelection {
        required_ids: numbers
            .iter()
            .map(|number| format!("prl-{number:06x}"))
            .collect(),
    }
}
fn ids(tasks: &[Pearl]) -> Vec<&str> {
    tasks.iter().map(|task| task.id.as_str()).collect()
}

#[test]
fn snapshots_follow_all_outgoing_types_transitively_through_archives() {
    let mut a = task(1, Status::Open);
    a.deps = vec![
        dependency(2, DepType::Blocks),
        dependency(3, DepType::Related),
    ];
    let mut b = task(2, Status::Closed);
    b.deps = vec![dependency(4, DepType::DiscoveredFrom)];
    let mut c = task(3, Status::Closed);
    c.deps = vec![dependency(1, DepType::ParentChild)];
    let mut incoming = task(5, Status::Open);
    incoming.deps = vec![dependency(1, DepType::Blocks)];
    let (_directory, service) = repository(
        &[a, incoming, task(6, Status::Open)],
        &[c, b, task(4, Status::Closed)],
    );
    let snapshot = service.snapshot_for(&selection(&[1, 1])).unwrap();
    assert_eq!(snapshot.required_ids, vec!["prl-000001"]);
    assert_eq!(ids(&snapshot.active), vec!["prl-000001"]);
    assert_eq!(
        ids(&snapshot.archived),
        vec!["prl-000002", "prl-000003", "prl-000004"]
    );
    assert_eq!(snapshot.revision, service.snapshot().unwrap().revision);
    let present: BTreeSet<_> = snapshot
        .active
        .iter()
        .chain(&snapshot.archived)
        .map(|task| &task.id)
        .collect();
    for task in snapshot.active.iter().chain(&snapshot.archived) {
        for dependency in &task.deps {
            assert!(present.contains(&dependency.target_id));
        }
    }
    // Selected graph and scheduling results are public serialization contracts.
    let json = serde_json::to_string(&snapshot).unwrap();
    assert_eq!(
        serde_json::from_str::<GraphSnapshot>(&json).unwrap(),
        snapshot
    );
}

#[test]
fn scheduler_partitions_requested_ids_and_reports_external_blockers() {
    let mut blocked = task(2, Status::Open);
    blocked.deps = vec![
        dependency(10, DepType::Blocks),
        dependency(9, DepType::Blocks),
        dependency(10, DepType::Blocks),
        dependency(11, DepType::Related),
    ];
    let mut explicit = task(3, Status::Blocked);
    explicit.deps = vec![dependency(9, DepType::Blocks)];
    let (_directory, service) = repository(
        &[
            task(1, Status::Open),
            blocked,
            explicit,
            task(4, Status::InProgress),
            task(5, Status::Deferred),
            task(9, Status::Deferred),
            task(10, Status::InProgress),
            task(11, Status::Open),
        ],
        &[task(6, Status::Closed)],
    );
    let selected = selection(&[6, 5, 4, 3, 2, 1, 2]);
    let result = service.schedule(&selected).unwrap();
    assert_eq!(ids(&result.ready), vec!["prl-000001"]);
    assert_eq!(ids(&result.running), vec!["prl-000004"]);
    assert_eq!(ids(&result.deferred), vec!["prl-000005"]);
    assert_eq!(ids(&result.closed), vec!["prl-000006"]);
    assert_eq!(
        result
            .blocked
            .iter()
            .map(|entry| entry.task.id.as_str())
            .collect::<Vec<_>>(),
        vec!["prl-000002", "prl-000003"]
    );
    assert_eq!(
        result.blocked[0].reasons,
        vec![ScheduleBlockReason::Dependencies]
    );
    assert_eq!(
        result.blocked[1].reasons,
        vec![
            ScheduleBlockReason::BlockedStatus,
            ScheduleBlockReason::Dependencies
        ]
    );
    assert_eq!(
        result.blocked[0]
            .blockers
            .iter()
            .map(|blocker| (blocker.id.as_str(), blocker.status))
            .collect::<Vec<_>>(),
        vec![
            ("prl-000009", Status::Deferred),
            ("prl-00000a", Status::InProgress)
        ]
    );
    assert_eq!(
        result.revision,
        service.snapshot_for(&selected).unwrap().revision
    );
    let json = serde_json::to_string(&result).unwrap();
    assert_eq!(
        serde_json::from_str::<ScheduleResult>(&json).unwrap(),
        result
    );
}

#[test]
fn no_ready_tasks_preserves_blocked_and_deferred_facts() {
    let (_directory, service) =
        repository(&[task(1, Status::Blocked), task(2, Status::Deferred)], &[]);
    let result = service.schedule(&selection(&[1, 2])).unwrap();
    assert!(result.ready.is_empty());
    assert!(result.closed.is_empty());
    assert_eq!(
        result.blocked[0].reasons,
        vec![ScheduleBlockReason::BlockedStatus]
    );
    assert!(result.blocked[0].blockers.is_empty());
    assert_eq!(ids(&result.deferred), vec!["prl-000002"]);
}

#[test]
fn explicit_status_takes_precedence_and_closed_archives_unblock_open_tasks() {
    let mut tasks: Vec<_> = [
        Status::Open,
        Status::InProgress,
        Status::Deferred,
        Status::Closed,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, status)| {
        let mut task = task(index as u32 + 1, status);
        task.deps.push(dependency(10, DepType::Blocks));
        task
    })
    .collect();
    tasks[0].deps = vec![dependency(11, DepType::Blocks)];
    tasks.push(task(10, Status::Open));
    let (_directory, service) =
        repository(&tasks, &[task(11, Status::Closed), task(12, Status::Open)]);
    let result = service.schedule(&selection(&[1, 2, 3, 4, 12])).unwrap();
    assert_eq!(ids(&result.ready), vec!["prl-000001", "prl-00000c"]);
    assert_eq!(ids(&result.running), vec!["prl-000002"]);
    assert_eq!(ids(&result.deferred), vec!["prl-000003"]);
    assert_eq!(ids(&result.closed), vec!["prl-000004"]);
    assert!(result.blocked.is_empty());
}

#[test]
fn every_set_uses_priority_then_recency_then_full_id() {
    let mut tasks = Vec::new();
    let mut expected = Vec::new();
    for (group, status) in [
        Status::Open,
        Status::InProgress,
        Status::Blocked,
        Status::Deferred,
        Status::Closed,
    ]
    .into_iter()
    .enumerate()
    {
        let base = group as u32 * 10;
        let mut group_tasks: Vec<_> = (1..=4).map(|number| task(base + number, status)).collect();
        group_tasks[0].priority = 0;
        group_tasks[0].updated_at = 1;
        group_tasks[1].priority = 1;
        group_tasks[1].updated_at = 300;
        group_tasks[2].priority = 1;
        group_tasks[2].updated_at = 300;
        group_tasks[3].priority = 1;
        group_tasks[3].updated_at = 200;
        expected.push(
            group_tasks
                .iter()
                .map(|task| task.id.clone())
                .collect::<Vec<_>>(),
        );
        tasks.extend(group_tasks.into_iter().rev());
    }
    let selected = TaskSelection {
        required_ids: tasks.iter().rev().map(|task| task.id.clone()).collect(),
    };
    let (_directory, service) = repository(&tasks, &[]);
    let result = service.schedule(&selected).unwrap();
    for (actual, expected) in [
        ids(&result.ready),
        ids(&result.running),
        result
            .blocked
            .iter()
            .map(|entry| entry.task.id.as_str())
            .collect(),
        ids(&result.deferred),
        ids(&result.closed),
    ]
    .into_iter()
    .zip(expected)
    {
        assert_eq!(actual, expected);
    }
    let first = service.snapshot_for(&selected).unwrap();
    let reversed = TaskSelection {
        required_ids: selected.required_ids.into_iter().rev().collect(),
    };
    assert_eq!(service.snapshot_for(&reversed).unwrap(), first);
    for pair in first.active.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        assert!(
            (left.priority, std::cmp::Reverse(left.updated_at), &left.id)
                <= (
                    right.priority,
                    std::cmp::Reverse(right.updated_at),
                    &right.id
                )
        );
    }
}

#[test]
fn unknown_exact_ids_fail_and_empty_selection_returns_empty_facts() {
    let (_directory, service) = repository(&[task(1, Status::Open)], &[]);
    for required_ids in [
        vec!["prl-000001".into(), "prl-000002".into()],
        vec!["prl-000".into()],
    ] {
        let selection = TaskSelection { required_ids };
        for error in [
            service.schedule(&selection).unwrap_err(),
            service.snapshot_for(&selection).unwrap_err(),
        ] {
            assert_eq!(error.code(), ErrorCode::NotFound);
            assert!(error.details().unwrap()["id"].is_string());
        }
    }
    let empty = TaskSelection::default();
    let result = service.schedule(&empty).unwrap();
    assert!(
        result.ready.is_empty()
            && result.running.is_empty()
            && result.blocked.is_empty()
            && result.deferred.is_empty()
            && result.closed.is_empty()
    );
    let snapshot = service.snapshot_for(&empty).unwrap();
    assert!(snapshot.active.is_empty() && snapshot.archived.is_empty());
    assert_eq!(snapshot.revision, result.revision);
}

#[test]
fn corruption_outside_selection_is_rejected_even_for_empty_requests() {
    let mut corrupt = task(2, Status::Open);
    corrupt.deps.push(dependency(99, DepType::Related));
    let (_directory, service) = repository(&[task(1, Status::Open), corrupt], &[]);
    for selection in [selection(&[1]), TaskSelection::default()] {
        assert_eq!(
            service.schedule(&selection).unwrap_err().code(),
            ErrorCode::InvalidGraph
        );
        assert_eq!(
            service.snapshot_for(&selection).unwrap_err().code(),
            ErrorCode::InvalidGraph
        );
    }
}

#[test]
fn selected_revision_tracks_changes_outside_selection_including_archives() {
    let (directory, service) = repository(&[task(1, Status::Open)], &[task(2, Status::Closed)]);
    let before = service.snapshot_for(&selection(&[1])).unwrap();
    let mut archived = task(2, Status::Closed);
    archived.description = "Changed without changing timestamps".into();
    write(&directory.path().join(".pearls/archive.jsonl"), &[archived]);
    let after = service.snapshot_for(&selection(&[1])).unwrap();
    assert_eq!(before.active, after.active);
    assert_ne!(before.revision, after.revision);
    let result = service.schedule(&selection(&[1])).unwrap();
    assert_eq!(result.revision, after.revision);
}

#[test]
fn metadata_and_close_batch_changes_scheduling_with_one_revision() {
    let mut dependent = task(1, Status::Open);
    dependent.deps.push(dependency(2, DepType::Blocks));
    let (_directory, service) = repository(&[dependent, task(2, Status::Open)], &[]);
    let before = service.schedule(&selection(&[1, 2])).unwrap();
    assert_eq!(ids(&before.ready), vec!["prl-000002"]);
    let receipt = service
        .apply(&BatchRequest {
            operation_key: "close-prerequisite".into(),
            expected_revision: before.revision,
            mutations: vec![
                Mutation::SetMetadata {
                    task: TaskRef::Id("prl-000002".into()),
                    key: "execution.report".into(),
                    value: json!("opaque"),
                },
                Mutation::SetStatus {
                    task: TaskRef::Id("prl-000002".into()),
                    status: Status::Closed,
                },
            ],
        })
        .unwrap();
    let after = service.schedule(&selection(&[1, 2])).unwrap();
    assert_eq!(after.revision, receipt.after_revision);
    assert_eq!(ids(&after.ready), vec!["prl-000001"]);
    assert_eq!(ids(&after.closed), vec!["prl-000002"]);
    assert_eq!(
        after.closed[0].metadata["execution.report"],
        json!("opaque")
    );
}

#[test]
fn selection_handles_deep_relationships_and_nonblocking_cycles_iteratively() {
    let count = 1500;
    let tasks: Vec<_> = (1..=count)
        .map(|number| {
            let mut task = task(number, Status::Open);
            task.deps.push(dependency(
                if number == count { 1 } else { number + 1 },
                DepType::Related,
            ));
            task
        })
        .collect();
    let (_directory, service) = repository(&tasks, &[]);
    let snapshot = service.snapshot_for(&selection(&[1])).unwrap();
    assert_eq!(snapshot.active.len(), count as usize);
    let result = service.schedule(&selection(&[1])).unwrap();
    assert_eq!(ids(&result.ready), vec!["prl-000001"]);
}

#[test]
fn concurrent_batches_never_split_selected_tasks_and_blocker_facts() {
    let mut dependent = task(1, Status::Open);
    dependent.deps.push(dependency(2, DepType::Blocks));
    dependent.metadata.insert("step".into(), json!(0));
    let mut prerequisite = task(2, Status::Open);
    prerequisite.metadata.insert("step".into(), json!(0));
    let (directory, service) = repository(&[dependent, prerequisite], &[]);
    let root = directory.path().to_path_buf();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let writer_barrier = barrier.clone();
    let writer = std::thread::spawn(move || {
        let service = GraphService::open(root).unwrap();
        writer_barrier.wait();
        for step in 1..=12 {
            service
                .apply(&BatchRequest {
                    operation_key: format!("step-{step}"),
                    expected_revision: service.snapshot().unwrap().revision,
                    mutations: vec![
                        Mutation::SetMetadata {
                            task: TaskRef::Id("prl-000001".into()),
                            key: "step".into(),
                            value: json!(step),
                        },
                        Mutation::SetMetadata {
                            task: TaskRef::Id("prl-000002".into()),
                            key: "step".into(),
                            value: json!(step),
                        },
                        Mutation::SetStatus {
                            task: TaskRef::Id("prl-000002".into()),
                            status: if step % 2 == 1 {
                                Status::Closed
                            } else {
                                Status::Open
                            },
                        },
                    ],
                })
                .unwrap();
        }
    });
    barrier.wait();
    for _ in 0..24 {
        let snapshot = service.snapshot_for(&selection(&[1])).unwrap();
        let dependent = snapshot
            .active
            .iter()
            .find(|task| task.id == "prl-000001")
            .unwrap();
        let prerequisite = snapshot
            .active
            .iter()
            .find(|task| task.id == "prl-000002")
            .unwrap();
        assert_eq!(dependent.metadata["step"], prerequisite.metadata["step"]);
        let result = service.schedule(&selection(&[1])).unwrap();
        if let Some(ready) = result.ready.first() {
            assert_eq!(ready.metadata["step"].as_u64().unwrap() % 2, 1);
            assert!(result.blocked.is_empty());
        } else {
            let blocked = &result.blocked[0];
            assert_eq!(blocked.task.metadata["step"].as_u64().unwrap() % 2, 0);
            assert_eq!(blocked.blockers[0].status, Status::Open);
        }
    }
    writer.join().unwrap();
}
