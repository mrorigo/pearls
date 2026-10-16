// Rust guideline compliant 2026-10-15

//! Application batch acceptance and failure contracts.

use pearls_app::{service::*, ErrorCode};
use pearls_core::{repository::RepositoryStore, DepType, Status};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{Arc, Barrier},
};
use tempfile::TempDir;

fn repository() -> (TempDir, GraphService) {
    let directory = TempDir::new().unwrap();
    std::fs::create_dir(directory.path().join(".pearls")).unwrap();
    let service = GraphService::open(directory.path()).unwrap();
    (directory, service)
}

fn create(name: &str) -> Mutation {
    Mutation::Create {
        local_ref: name.into(),
        task: CreateTask {
            title: name.into(),
            author: "tester".into(),
            description: String::new(),
            priority: None,
            labels: vec![],
            metadata: BTreeMap::new(),
            external_key: Some(format!("import:{name}")),
        },
    }
}

fn local(name: &str) -> TaskRef {
    TaskRef::Local(name.into())
}
fn id(name: &str) -> TaskRef {
    TaskRef::Id(name.into())
}
fn link(from: &str, to: &str) -> Mutation {
    Mutation::Link {
        from: local(from),
        to: local(to),
        dep_type: DepType::Blocks,
    }
}
fn request(service: &GraphService, key: &str, mutations: Vec<Mutation>) -> BatchRequest {
    BatchRequest {
        operation_key: key.into(),
        expected_revision: service.snapshot().unwrap().revision,
        mutations,
    }
}

#[test]
fn import_reopen_and_replay_return_original_mapping() {
    let (directory, service) = repository();
    let request = request(
        &service,
        "import",
        vec![
            create("a"),
            create("b"),
            create("c"),
            link("a", "b"),
            link("b", "c"),
        ],
    );
    let receipt = service.apply(&request).unwrap();
    drop(service);
    let service = GraphService::open(directory.path()).unwrap();
    assert_eq!(service.apply(&request).unwrap(), receipt);
    assert_eq!(service.receipt("import").unwrap(), Some(receipt.clone()));
    assert_eq!(service.snapshot().unwrap().data.active.len(), 3);
    assert_eq!(receipt.created_ids.len(), 3);
    let mut different = request.clone();
    different.mutations.reverse();
    assert_eq!(
        service.apply(&different).unwrap_err().code(),
        ErrorCode::IdempotencyConflict
    );
    let mut changed_revision = request;
    changed_revision.expected_revision = receipt.after_revision;
    assert_eq!(
        service.apply(&changed_revision).unwrap_err().code(),
        ErrorCode::IdempotencyConflict
    );
}

#[test]
fn cycle_rejects_every_task_and_receipt() {
    let (_directory, service) = repository();
    let before = service.snapshot().unwrap();
    let request = request(
        &service,
        "cycle",
        vec![create("a"), create("b"), link("a", "b"), link("b", "a")],
    );
    assert_eq!(
        service.apply(&request).unwrap_err().code(),
        ErrorCode::InvalidGraph
    );
    assert!(service.snapshot().unwrap().data.active.is_empty());
    assert_eq!(service.snapshot().unwrap().revision, before.revision);
    assert!(service.receipt("cycle").unwrap().is_none());
}

#[test]
fn close_and_opaque_reference_replay_together() {
    let (_directory, service) = repository();
    let receipt = service
        .apply(&request(&service, "create", vec![create("a")]))
        .unwrap();
    let task = &receipt.created_ids["a"];
    let close = request(
        &service,
        "close",
        vec![
            Mutation::SetMetadata {
                task: id(task),
                key: "execution.report".into(),
                value: json!({"digest":"opaque", "verified":false}),
            },
            Mutation::SetStatus {
                task: id(task),
                status: Status::Closed,
            },
        ],
    );
    let receipt = service.apply(&close).unwrap();
    assert_eq!(service.apply(&close).unwrap(), receipt);
    let snapshot = service.snapshot().unwrap();
    assert_eq!(snapshot.data.active[0].status, Status::Closed);
    assert_eq!(
        snapshot.data.active[0].metadata["execution.report"]["verified"],
        false
    );
}

#[test]
fn ordered_transitions_expose_blockers_and_roll_back_metadata() {
    let (_directory, service) = repository();
    let receipt = service
        .apply(&request(
            &service,
            "create",
            vec![create("a"), create("b"), link("a", "b")],
        ))
        .unwrap();
    let a = &receipt.created_ids["a"];
    let b = &receipt.created_ids["b"];
    let fail = request(
        &service,
        "fail",
        vec![
            Mutation::SetMetadata {
                task: id(a),
                key: "report".into(),
                value: json!(42),
            },
            Mutation::SetStatus {
                task: id(a),
                status: Status::Closed,
            },
        ],
    );
    let error = service.apply(&fail).unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidTransition);
    assert_eq!(error.details().unwrap()["blockers"][0]["id"], *b);
    assert!(!service
        .snapshot()
        .unwrap()
        .data
        .active
        .iter()
        .find(|p| p.id == *a)
        .unwrap()
        .metadata
        .contains_key("report"));
    assert!(service.receipt("fail").unwrap().is_none());
    service
        .apply(&request(
            &service,
            "close",
            vec![
                Mutation::SetStatus {
                    task: id(b),
                    status: Status::Closed,
                },
                Mutation::SetStatus {
                    task: id(a),
                    status: Status::Closed,
                },
            ],
        ))
        .unwrap();
}

#[test]
fn external_creation_uses_original_definition_after_edits_and_archiving() {
    let (directory, service) = repository();
    let original = create("a");
    let receipt = service
        .apply(&request(&service, "create", vec![original.clone()]))
        .unwrap();
    let a = receipt.created_ids["a"].clone();
    service
        .apply(&request(
            &service,
            "edit",
            vec![
                Mutation::Update {
                    task: id(&a),
                    patch: TaskPatch {
                        title: Some("Edited".into()),
                        ..Default::default()
                    },
                },
                Mutation::SetStatus {
                    task: id(&a),
                    status: Status::Closed,
                },
            ],
        ))
        .unwrap();
    // Use the transaction layer to stand in for the later compaction adapter.
    let store = RepositoryStore::open(directory.path()).unwrap();
    let revision = store.snapshot().unwrap().revision;
    store
        .transact::<_, pearls_core::repository::RepositoryError>(
            &pearls_core::repository::OperationRequest {
                operation_key: "archive".into(),
                request_digest: "test".into(),
                expected_revision: revision,
            },
            |data| {
                data.archived.append(&mut data.active);
                Ok(BTreeMap::new())
            },
        )
        .unwrap();
    let repeat = request(&service, "reimport", vec![original.clone()]);
    assert_eq!(service.apply(&repeat).unwrap().created_ids["a"], a);
    assert_eq!(service.snapshot().unwrap().data.archived[0].title, "Edited");
    assert_eq!(
        service
            .apply(&request(
                &service,
                "conflict",
                vec![match original {
                    Mutation::Create {
                        local_ref,
                        mut task,
                    } => {
                        task.title = "Different".into();
                        Mutation::Create { local_ref, task }
                    }
                    _ => unreachable!(),
                }]
            ))
            .unwrap_err()
            .code(),
        ErrorCode::ExternalKeyConflict
    );
    assert_eq!(service.receipt("create").unwrap(), Some(receipt));
}

#[test]
fn duplicate_external_keys_across_files_are_conflicts() {
    let (directory, service) = repository();
    service
        .apply(&request(&service, "create", vec![create("a")]))
        .unwrap();
    let mut duplicate = service.snapshot().unwrap().data.active.remove(0);
    duplicate.id = "prl-abcdef12".into();
    std::fs::write(
        directory.path().join(".pearls/archive.jsonl"),
        format!("{}\n", serde_json::to_string(&duplicate).unwrap()),
    )
    .unwrap();
    let error = service.snapshot().unwrap_err();
    assert_eq!(error.code(), ErrorCode::ExternalKeyConflict);
    assert_eq!(
        error.details().unwrap()["task_ids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn simultaneous_requests_have_one_winner_and_a_typed_revision_conflict() {
    let (directory, service) = repository();
    let first = request(&service, "first", vec![create("a")]);
    let second = BatchRequest {
        operation_key: "second".into(),
        mutations: vec![create("b")],
        ..first.clone()
    };
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = [first, second]
        .into_iter()
        .map(|request| {
            let root = directory.path().to_path_buf();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let service = GraphService::open(root).unwrap();
                barrier.wait();
                service.apply(&request)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    let error = results.into_iter().find_map(Result::err).unwrap();
    assert_eq!(error.code(), ErrorCode::RevisionConflict);
    let details = error.details().unwrap();
    assert_ne!(details["expected"], details["actual"]);
    assert_eq!(service.snapshot().unwrap().data.active.len(), 1);
}

#[test]
fn nested_json_object_key_order_does_not_change_digest() {
    let (_directory, service) = repository();
    let mut first = request(&service, "canonical", vec![create("a")]);
    if let Mutation::Create { task, .. } = &mut first.mutations[0] {
        task.metadata.insert(
            "nested".into(),
            serde_json::from_str(r#"{"b":{"z":1,"a":2},"a":[2,1]}"#).unwrap(),
        );
    }
    let receipt = service.apply(&first).unwrap();
    let mut second = first.clone();
    if let Mutation::Create { task, .. } = &mut second.mutations[0] {
        task.metadata.insert(
            "nested".into(),
            serde_json::from_str(r#"{"a":[2,1],"b":{"a":2,"z":1}}"#).unwrap(),
        );
    }
    assert_eq!(service.apply(&second).unwrap(), receipt);
    if let Mutation::Create { task, .. } = &mut second.mutations[0] {
        task.metadata
            .insert("nested".into(), json!({"a":[1,2],"b":{"a":2,"z":1}}));
    }
    assert_eq!(
        service.apply(&second).unwrap_err().code(),
        ErrorCode::IdempotencyConflict
    );
}

#[test]
fn invalid_references_and_inputs_leave_no_receipt() {
    let (_directory, service) = repository();
    let cases = vec![
        vec![],
        vec![create("a"), create("a")],
        vec![
            Mutation::Link {
                from: local("future"),
                to: local("future"),
                dep_type: DepType::Related,
            },
            create("future"),
        ],
        vec![
            create("a"),
            Mutation::SetMetadata {
                task: local("a"),
                key: EXTERNAL_KEY.into(),
                value: json!("overwrite"),
            },
        ],
        vec![
            create("a"),
            Mutation::Update {
                task: local("a"),
                patch: TaskPatch {
                    priority: Some(32),
                    ..Default::default()
                },
            },
        ],
        vec![
            create("a"),
            Mutation::Update {
                task: local("a"),
                patch: TaskPatch {
                    description: Some("x".repeat(65537)),
                    ..Default::default()
                },
            },
        ],
    ];
    for (index, mutations) in cases.into_iter().enumerate() {
        let key = index.to_string();
        assert_eq!(
            service
                .apply(&request(&service, &key, mutations))
                .unwrap_err()
                .code(),
            ErrorCode::InvalidInput
        );
        assert!(service.receipt(&key).unwrap().is_none());
        assert!(service.snapshot().unwrap().data.active.is_empty());
    }
    let missing = request(
        &service,
        "missing",
        vec![Mutation::SetStatus {
            task: id("prl-000001"),
            status: Status::Closed,
        }],
    );
    let error = service.apply(&missing).unwrap_err();
    assert_eq!(error.code(), ErrorCode::NotFound);
    assert_eq!(error.details().unwrap()["id"], "prl-000001");
}

#[test]
fn comments_links_and_updates_share_the_batch() {
    let (_directory, service) = repository();
    let receipt = service
        .apply(&request(
            &service,
            "all",
            vec![
                create("a"),
                create("b"),
                Mutation::AddComment {
                    task: local("a"),
                    author: "alice".into(),
                    body: "hello".into(),
                },
                Mutation::Update {
                    task: local("a"),
                    patch: TaskPatch {
                        labels: Some(vec!["label".into()]),
                        description: Some("description".into()),
                        priority: Some(1),
                        ..Default::default()
                    },
                },
                link("a", "b"),
                Mutation::Unlink {
                    from: local("a"),
                    to: local("b"),
                },
            ],
        ))
        .unwrap();
    let snapshot = service.snapshot().unwrap();
    let a = snapshot
        .data
        .active
        .iter()
        .find(|p| p.id == receipt.created_ids["a"])
        .unwrap();
    assert!(a.deps.is_empty());
    assert_eq!(a.comments.len(), 1);
    assert_eq!(a.priority, 1);
    assert_eq!(a.labels, vec!["label"]);
    let delete = request(
        &service,
        "delete",
        vec![Mutation::DeleteComment {
            task: id(&a.id),
            comment_id: a.comments[0].id.clone(),
        }],
    );
    service.apply(&delete).unwrap();
    service.apply(&delete).unwrap();
    assert!(service
        .snapshot()
        .unwrap()
        .data
        .active
        .iter()
        .find(|p| p.id == a.id)
        .unwrap()
        .comments
        .is_empty());
}

#[test]
fn opening_requires_an_explicit_initialized_path_and_does_not_write() {
    assert_eq!(
        GraphService::open("").err().unwrap().code(),
        ErrorCode::InvalidInput
    );
    let directory = TempDir::new().unwrap();
    assert_eq!(
        GraphService::open(directory.path()).err().unwrap().code(),
        ErrorCode::RepoNotInitialized
    );
    std::fs::create_dir(directory.path().join(".pearls")).unwrap();
    GraphService::open(directory.path()).unwrap();
    assert_eq!(
        std::fs::read_dir(directory.path().join(".pearls"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn concurrent_replays_create_one_task_and_share_one_receipt() {
    let (directory, service) = repository();
    let batch = request(&service, "same", vec![create("a")]);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let root = directory.path().to_path_buf();
            let barrier = barrier.clone();
            let batch = batch.clone();
            std::thread::spawn(move || {
                let service = GraphService::open(root).unwrap();
                barrier.wait();
                service.apply(&batch).unwrap()
            })
        })
        .collect();
    let receipts: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(receipts[0], receipts[1]);
    assert_eq!(service.snapshot().unwrap().data.active.len(), 1);
}

#[test]
fn replay_precedes_changed_configuration_and_external_edits_stale_new_requests() {
    let (directory, service) = repository();
    let batch = request(&service, "original", vec![create("a")]);
    let receipt = service.apply(&batch).unwrap();
    std::fs::write(
        directory.path().join(".pearls/config.toml"),
        "not valid toml = [",
    )
    .unwrap();
    assert_eq!(service.apply(&batch).unwrap(), receipt);
    std::fs::remove_file(directory.path().join(".pearls/config.toml")).unwrap();
    let next = request(&service, "next", vec![create("b")]);
    let mut pearl = service.snapshot().unwrap().data.active.remove(0);
    pearl.description = "Edited outside Pearls".into();
    std::fs::write(
        directory.path().join(".pearls/issues.jsonl"),
        format!("{}\n", serde_json::to_string(&pearl).unwrap()),
    )
    .unwrap();
    assert_eq!(
        service.apply(&next).unwrap_err().code(),
        ErrorCode::RevisionConflict
    );
    assert_eq!(
        service.snapshot().unwrap().data.active[0].description,
        "Edited outside Pearls"
    );
    assert!(service.receipt("next").unwrap().is_none());
}

#[test]
fn invalid_external_identity_is_a_structured_integrity_error() {
    let (directory, service) = repository();
    service
        .apply(&request(&service, "create", vec![create("a")]))
        .unwrap();
    let original = service.snapshot().unwrap().data.active.remove(0);
    for value in [json!(42), json!({"bad":"definition"})] {
        let mut pearl = original.clone();
        if value.is_number() {
            pearl.metadata.insert(EXTERNAL_KEY.into(), value);
        } else {
            pearl.metadata.insert(EXTERNAL_DEFINITION.into(), value);
        }
        std::fs::write(
            directory.path().join(".pearls/issues.jsonl"),
            format!("{}\n", serde_json::to_string(&pearl).unwrap()),
        )
        .unwrap();
        let error = service.snapshot().unwrap_err();
        assert_eq!(error.code(), ErrorCode::CorruptStorage);
        assert_eq!(error.details().unwrap()["task_id"], original.id);
    }
}

#[test]
fn archived_prerequisites_are_validated_and_cannot_be_edited() {
    let (directory, service) = repository();
    let receipt = service
        .apply(&request(
            &service,
            "create",
            vec![
                create("a"),
                create("b"),
                link("a", "b"),
                Mutation::SetStatus {
                    task: local("b"),
                    status: Status::Closed,
                },
            ],
        ))
        .unwrap();
    let store = RepositoryStore::open(directory.path()).unwrap();
    store
        .transact::<_, pearls_core::repository::RepositoryError>(
            &pearls_core::repository::OperationRequest {
                operation_key: "archive".into(),
                request_digest: "test".into(),
                expected_revision: receipt.after_revision,
            },
            |data| {
                let position = data
                    .active
                    .iter()
                    .position(|p| p.id == receipt.created_ids["b"])
                    .unwrap();
                data.archived.push(data.active.remove(position));
                Ok(BTreeMap::new())
            },
        )
        .unwrap();
    service
        .apply(&request(
            &service,
            "close",
            vec![Mutation::SetStatus {
                task: id(&receipt.created_ids["a"]),
                status: Status::Closed,
            }],
        ))
        .unwrap();
    let error = service
        .apply(&request(
            &service,
            "edit",
            vec![Mutation::SetMetadata {
                task: id(&receipt.created_ids["b"]),
                key: "report".into(),
                value: json!(42),
            }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidInput);
}

#[test]
fn empty_operation_keys_and_unknown_full_comment_ids_are_rejected() {
    let (_directory, service) = repository();
    assert_eq!(
        service
            .apply(&request(&service, "", vec![create("a")]))
            .unwrap_err()
            .code(),
        ErrorCode::InvalidInput
    );
    service
        .apply(&request(&service, "create", vec![create("a")]))
        .unwrap();
    let a = service.snapshot().unwrap().data.active.remove(0);
    let error = service
        .apply(&request(
            &service,
            "comment",
            vec![Mutation::DeleteComment {
                task: id(&a.id),
                comment_id: "cmt-000001".into(),
            }],
        ))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::NotFound);
    assert_eq!(error.details().unwrap()["id"], "cmt-000001");
}

#[test]
fn content_updates_validate_only_supplied_fields_against_current_config() {
    let (directory, service) = repository();
    let receipt = service
        .apply(&request(&service, "create", vec![create("a")]))
        .unwrap();
    std::fs::write(
        directory.path().join(".pearls/config.toml"),
        "default_priority = 0\nmax_priority = 1\n",
    )
    .unwrap();
    service
        .apply(&request(
            &service,
            "title",
            vec![Mutation::Update {
                task: id(&receipt.created_ids["a"]),
                patch: TaskPatch {
                    title: Some("Renamed".into()),
                    ..Default::default()
                },
            }],
        ))
        .unwrap();
    let pearl = service.snapshot().unwrap().data.active.remove(0);
    assert_eq!(pearl.priority, 2);
    assert_eq!(pearl.title, "Renamed");
}
