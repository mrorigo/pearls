// Rust guideline compliant 2026-10-15

//! One-shot adapter and maintenance contracts.

use pearls_app::{service::*, ErrorCode};
use pearls_core::{DepType, Dependency, Pearl, Status};
use tempfile::TempDir;

fn repository() -> (TempDir, GraphService) {
    let root = TempDir::new().unwrap();
    std::fs::create_dir(root.path().join(".pearls")).unwrap();
    let service = GraphService::open(root.path()).unwrap();
    (root, service)
}
fn task(id: &str) -> Pearl {
    let mut task = Pearl::new("Task".into(), "tester".into());
    task.id = id.into();
    task.created_at = 100;
    task.updated_at = 100;
    task
}
fn import(tasks: Vec<Pearl>) -> Vec<Mutation> {
    vec![Mutation::Import { tasks }]
}

#[test]
fn concurrent_adapters_serialize_without_overwriting_prior_changes() {
    let (root, service) = repository();
    service
        .execute(|_| Ok(import(vec![task("prl-000001")])))
        .unwrap();
    let path = root.path().to_path_buf();
    let mut writer = None;
    let first = service
        .execute(|snapshot| {
            let (started, sent) = std::sync::mpsc::channel();
            writer = Some(std::thread::spawn(move || {
                let service = GraphService::open(path).unwrap();
                started.send(()).unwrap();
                service
                    .execute(|_| {
                        Ok(vec![Mutation::SetMetadata {
                            task: TaskRef::Id("prl-000001".into()),
                            key: "winner".into(),
                            value: true.into(),
                        }])
                    })
                    .unwrap()
            }));
            sent.recv().unwrap();
            let task = resolve_active_task(&snapshot.data, "prl-000")?;
            Ok(vec![Mutation::Update {
                task: TaskRef::Id(task.id.clone()),
                patch: TaskPatch {
                    title: Some("First".into()),
                    ..Default::default()
                },
            }])
        })
        .unwrap();
    let second = writer.unwrap().join().unwrap();
    assert_eq!(second.receipt.before_revision, first.receipt.after_revision);
    assert_eq!(first.task("prl-000001").unwrap().title, "First");
    assert!(!first
        .task("prl-000001")
        .unwrap()
        .metadata
        .contains_key("winner"));
    let current = service.snapshot().unwrap();
    assert_eq!(current.data.active[0].title, "First");
    assert_eq!(current.data.active[0].metadata["winner"], true);
}

#[test]
fn adapter_response_is_the_committed_image_and_has_a_receipt() {
    let (_root, service) = repository();
    let result = service
        .execute(|_| Ok(import(vec![task("prl-000001")])))
        .unwrap();
    service
        .execute(|_| {
            Ok(vec![Mutation::Update {
                task: TaskRef::Id("prl-000001".into()),
                patch: TaskPatch {
                    title: Some("Later".into()),
                    ..Default::default()
                },
            }])
        })
        .unwrap();
    assert_eq!(result.task("prl-000001").unwrap().title, "Task");
    assert_eq!(service.snapshot().unwrap().data.active[0].title, "Later");
    assert_eq!(
        service.receipt(&result.receipt.operation_key).unwrap(),
        Some(result.receipt)
    );
}

#[test]
fn imported_updates_share_transition_and_cycle_validation() {
    let (_root, service) = repository();
    let mut dependent = task("prl-000001");
    dependent.deps.push(Dependency {
        target_id: "prl-000002".into(),
        dep_type: DepType::Blocks,
    });
    service
        .execute(|_| Ok(import(vec![dependent.clone(), task("prl-000002")])))
        .unwrap();
    let before = service.snapshot().unwrap();
    dependent.status = Status::Closed;
    assert_eq!(
        service
            .execute(|_| Ok(import(vec![dependent])))
            .unwrap_err()
            .code(),
        ErrorCode::InvalidTransition
    );
    let mut cycle = task("prl-000002");
    cycle.deps.push(Dependency {
        target_id: "prl-000001".into(),
        dep_type: DepType::Blocks,
    });
    assert_eq!(
        service
            .execute(|_| Ok(import(vec![cycle])))
            .unwrap_err()
            .code(),
        ErrorCode::InvalidGraph
    );
    assert_eq!(service.snapshot().unwrap(), before);
}

#[test]
fn real_compaction_preserves_external_identity_and_original_replay() {
    let (_root, service) = repository();
    let request = BatchRequest {
        operation_key: "import".into(),
        expected_revision: service.snapshot().unwrap().revision,
        mutations: vec![
            Mutation::Create {
                local_ref: "task".into(),
                task: CreateTask {
                    title: "External".into(),
                    author: "tester".into(),
                    description: String::new(),
                    priority: None,
                    labels: vec![],
                    metadata: Default::default(),
                    external_key: Some("external:1".into()),
                },
            },
            Mutation::SetStatus {
                task: TaskRef::Local("task".into()),
                status: Status::Closed,
            },
        ],
    };
    let receipt = service.apply(&request).unwrap();
    let compacted = service.compact(i64::MAX).unwrap();
    assert!(compacted.data.active.is_empty());
    assert_eq!(compacted.data.archived[0].id, receipt.created_ids["task"]);
    assert_eq!(service.apply(&request).unwrap(), receipt);
    let reimport = BatchRequest {
        operation_key: "reimport".into(),
        expected_revision: compacted.receipt.after_revision,
        mutations: vec![request.mutations[0].clone()],
    };
    assert_eq!(
        service.apply(&reimport).unwrap().created_ids,
        receipt.created_ids
    );
}

#[test]
fn doctor_uses_latest_locked_files_and_keeps_archived_endpoints_and_receipts() {
    let (root, service) = repository();
    let mut dependent = task("prl-000001");
    dependent.deps.push(Dependency {
        target_id: "prl-000002".into(),
        dep_type: DepType::Blocks,
    });
    let mut archived = task("prl-000002");
    archived.status = Status::Closed;
    let created = service
        .execute(|_| Ok(import(vec![dependent.clone(), archived])))
        .unwrap();
    service.compact(100).unwrap();
    let mut newer = dependent.clone();
    newer.title = "Newest".into();
    newer.updated_at = 200;
    newer.deps.push(Dependency {
        target_id: "prl-999999".into(),
        dep_type: DepType::Related,
    });
    std::fs::write(
        root.path().join(".pearls/issues.jsonl"),
        format!(
            "{}\nnot JSON\n{}\n",
            serde_json::to_string(&newer).unwrap(),
            serde_json::to_string(&dependent).unwrap()
        ),
    )
    .unwrap();
    let repaired = service.repair().unwrap();
    let snapshot = service.snapshot().unwrap();
    assert_eq!(snapshot.revision, repaired.after_revision);
    assert_eq!(snapshot.data.active.len(), 1);
    assert_eq!(snapshot.data.active[0].title, "Newest");
    assert_eq!(snapshot.data.active[0].deps.len(), 1);
    assert_eq!(snapshot.data.active[0].deps[0].target_id, "prl-000002");
    assert_eq!(
        service.receipt(&created.receipt.operation_key).unwrap(),
        Some(created.receipt)
    );
}

#[test]
fn maintenance_rejects_duplicate_external_keys_and_preserves_files() {
    let (root, service) = repository();
    let mut duplicate = task("prl-000001");
    duplicate.status = Status::Closed;
    duplicate
        .metadata
        .insert(EXTERNAL_KEY.into(), "external".into());
    let line = format!("{}\n", serde_json::to_string(&duplicate).unwrap());
    std::fs::write(root.path().join(".pearls/issues.jsonl"), &line).unwrap();
    std::fs::write(root.path().join(".pearls/archive.jsonl"), &line).unwrap();
    assert_eq!(
        service.compact(i64::MAX).unwrap_err().code(),
        ErrorCode::ExternalKeyConflict
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join(".pearls/issues.jsonl")).unwrap(),
        line
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join(".pearls/archive.jsonl")).unwrap(),
        line
    );
}
