// Rust guideline compliant 2026-10-15

//! Transaction, concurrency, and crash-recovery tests.

use super::*;
use crate::Status;
use std::fs;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Barrier};
use tempfile::TempDir;

fn task(id: &str, title: &str) -> Pearl {
    let mut pearl = Pearl::new(title.to_string(), "tester".to_string());
    pearl.id = id.to_string();
    pearl.created_at = 100;
    pearl.updated_at = 100;
    pearl
}

pub(super) fn setup() -> (TempDir, RepositoryStore, RepositorySnapshot) {
    let root = TempDir::new().expect("temporary repository");
    fs::create_dir(root.path().join(".pearls")).expect("create .pearls");
    let store = RepositoryStore::open(root.path()).expect("open store");
    let original = task("prl-000001", "Original");
    let mut archived = task("prl-000002", "Archived");
    archived.status = Status::Closed;
    fs::write(
        store.files.path(ISSUES),
        format!("{}\n", serde_json::to_string(&original).unwrap()),
    )
    .unwrap();
    fs::write(
        store.files.path(ARCHIVE),
        format!("{}\n", serde_json::to_string(&archived).unwrap()),
    )
    .unwrap();
    let snapshot = store.snapshot().expect("bootstrap metadata");
    fs::write(store.files.path("index.bin"), b"derived index").unwrap();
    store.files.steps.set(0);
    (root, store, snapshot)
}

pub(super) fn request(snapshot: &RepositorySnapshot, key: &str) -> OperationRequest {
    OperationRequest {
        operation_key: key.to_string(),
        request_digest: "canonical-digest".to_string(),
        expected_revision: snapshot.revision.clone(),
    }
}

fn proposal(data: &mut RepositoryData) -> Result<BTreeMap<String, String>> {
    let mut archived = data.active.remove(0);
    archived.status = Status::Closed;
    data.archived.push(archived);
    let created = task("prl-000003", "Created");
    let ids = BTreeMap::from([("new".to_string(), created.id.clone())]);
    data.active.push(created);
    Ok(ids)
}

#[test]
fn explicit_open_does_not_initialize_or_fall_back() {
    let root = TempDir::new().unwrap();
    assert!(matches!(
        RepositoryStore::open(root.path()),
        Err(RepositoryError::NotInitialized(_))
    ));
    assert!(!root.path().join(".pearls").exists());
    assert!(matches!(
        RepositoryStore::open(""),
        Err(RepositoryError::InvalidInput(_))
    ));
}

#[test]
fn replay_precedes_revision_check_and_does_not_reapply() {
    let (root, store, before) = setup();
    let request = request(&before, "import");
    let receipt = store.transact(&request, proposal).unwrap();
    assert_ne!(receipt.before_revision, receipt.after_revision);
    assert_eq!(receipt.created_ids["new"], "prl-000003");
    let reopened = RepositoryStore::open(root.path()).unwrap();
    let replay = reopened
        .transact(&request, |_| {
            Err(RepositoryError::InvalidInput("Must not execute".into()))
        })
        .unwrap();
    assert_eq!(receipt, replay);
    assert_eq!(reopened.receipt("import").unwrap(), Some(receipt.clone()));
    let after = reopened.snapshot().unwrap();
    assert_eq!(after.data.active.len(), 1);
    assert_eq!(after.data.archived.len(), 2);
    assert_eq!(after.revision, receipt.after_revision);
    assert!(!store.files.path("index.bin").exists());

    let mut conflicting = request.clone();
    conflicting.request_digest = "different".to_string();
    assert!(matches!(
        reopened.transact(&conflicting, proposal),
        Err(RepositoryError::IdempotencyConflict { .. })
    ));
    conflicting = request;
    conflicting.expected_revision = after.revision;
    assert!(matches!(
        reopened.transact(&conflicting, proposal),
        Err(RepositoryError::IdempotencyConflict { .. })
    ));
}

#[test]
fn stale_or_failed_proposals_do_not_publish() {
    let (_root, store, before) = setup();
    store
        .transact(&request(&before, "first"), proposal)
        .unwrap();
    let bytes = store.files.read(STATE).unwrap();
    assert!(matches!(
        store.transact(&request(&before, "second"), proposal),
        Err(RepositoryError::RevisionConflict { .. })
    ));
    let current = store.snapshot().unwrap();
    assert!(store
        .transact(
            &request(&current, "invalid"),
            |data| -> Result<BTreeMap<String, String>> {
                data.active.clear();
                Err(RepositoryError::InvalidInput("invalid proposal".into()))
            }
        )
        .is_err());
    assert_eq!(store.files.read(STATE).unwrap(), bytes);
    assert_eq!(store.snapshot().unwrap(), current);
    assert!(store.receipt("invalid").unwrap().is_none());
}

#[test]
fn generation_changes_even_if_task_contents_return_to_original() {
    let (_root, store, original) = setup();
    store
        .transact(
            &request(&original, "change"),
            |data| -> Result<BTreeMap<String, String>> {
                data.active[0].title = "Edited".into();
                Ok(BTreeMap::new())
            },
        )
        .unwrap();
    let changed = store.snapshot().unwrap();
    store
        .transact(
            &request(&changed, "restore"),
            |data| -> Result<BTreeMap<String, String>> {
                data.active[0].title = original.data.active[0].title.clone();
                Ok(BTreeMap::new())
            },
        )
        .unwrap();
    let restored = store.snapshot().unwrap();
    assert_eq!(original.data, restored.data);
    assert_ne!(original.revision, restored.revision);
    assert!(matches!(
        store.transact(&request(&original, "stale"), proposal),
        Err(RepositoryError::RevisionConflict { .. })
    ));
}

#[test]
fn observed_external_edits_advance_generation_including_reverts() {
    let (_root, store, original) = setup();
    let original_bytes = store.files.read(ARCHIVE).unwrap().unwrap();
    let mut edited = original_bytes.clone();
    edited.push(b'\n');
    fs::write(store.files.path(ARCHIVE), edited).unwrap();
    let changed = store.snapshot().unwrap();
    assert_ne!(changed.revision, original.revision);
    assert_eq!(changed.data, original.data);
    fs::write(store.files.path(ARCHIVE), original_bytes).unwrap();
    let restored = store.snapshot().unwrap();
    assert_ne!(restored.revision, original.revision);
    assert_ne!(restored.revision, changed.revision);
    assert_eq!(store.snapshot().unwrap(), restored);
}

#[test]
fn corrupt_external_files_do_not_update_metadata() {
    let (_root, store, _) = setup();
    let original_state = store.files.read(STATE).unwrap();
    for bytes in [b"{truncated".as_slice(), b"\xff\n".as_slice()] {
        fs::write(store.files.path(ARCHIVE), bytes).unwrap();
        assert!(matches!(
            store.snapshot(),
            Err(RepositoryError::CorruptStorage { .. })
        ));
        assert_eq!(store.files.read(STATE).unwrap(), original_state);
    }
}

#[test]
fn invalid_task_sets_are_not_published() {
    let (_root, store, before) = setup();
    assert!(store
        .transact(
            &request(&before, "duplicate"),
            |data| -> Result<BTreeMap<String, String>> {
                data.archived.push(data.active[0].clone());
                Ok(BTreeMap::new())
            }
        )
        .is_err());
    assert_eq!(store.snapshot().unwrap(), before);
    assert!(store
        .transact(
            &request(&before, "oversized"),
            |data| -> Result<BTreeMap<String, String>> {
                data.active[0].description = "x".repeat(MAX_JSONL_LINE_BYTES);
                Ok(BTreeMap::new())
            }
        )
        .is_err());
    assert_eq!(store.snapshot().unwrap(), before);
}

#[test]
fn external_edit_during_proposal_is_not_overwritten() {
    let (_root, store, before) = setup();
    let mut edited = store.files.read(ARCHIVE).unwrap().unwrap();
    edited.push(b'\n');
    assert!(matches!(
        store.transact(
            &request(&before, "edit"),
            |data| -> Result<BTreeMap<String, String>> {
                proposal(data)?;
                fs::write(store.files.path(ARCHIVE), &edited)?;
                Ok(BTreeMap::new())
            }
        ),
        Err(RepositoryError::CorruptStorage { .. })
    ));
    assert_eq!(store.files.read(ARCHIVE).unwrap().unwrap(), edited);
    assert!(store.receipt("edit").unwrap().is_none());
    assert_eq!(store.snapshot().unwrap().data, before.data);
}

#[test]
fn concurrent_writers_have_one_winner_per_revision() {
    let (root, _, before) = setup();
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for key in ["first", "second"] {
            let barrier = barrier.clone();
            let root = root.path();
            let request = request(&before, key);
            handles.push(scope.spawn(move || {
                let store = RepositoryStore::open(root).unwrap();
                barrier.wait();
                store.transact(&request, proposal)
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(RepositoryError::RevisionConflict { .. })))
            .count(),
        1
    );
}

#[test]
fn concurrent_identical_keys_execute_once() {
    let (root, _, before) = setup();
    let barrier = Arc::new(Barrier::new(2));
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let receipts = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let barrier = barrier.clone();
                let calls = calls.clone();
                let root = root.path();
                let request = request(&before, "shared");
                scope.spawn(move || {
                    let store = RepositoryStore::open(root).unwrap();
                    barrier.wait();
                    store
                        .transact(&request, |data| {
                            calls.fetch_add(1, Ordering::Relaxed);
                            proposal(data)
                        })
                        .unwrap()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(receipts[0], receipts[1]);
}

#[test]
fn receipts_survive_subsequent_archiving() {
    let (_root, store, before) = setup();
    let original_request = request(&before, "import");
    let receipt = store.transact(&original_request, proposal).unwrap();
    let current = store.snapshot().unwrap();
    store
        .transact(&request(&current, "compact"), |data| -> Result<_> {
            for mut pearl in data.active.drain(..) {
                pearl.status = Status::Closed;
                data.archived.push(pearl);
            }
            Ok(BTreeMap::new())
        })
        .unwrap();
    assert!(store.snapshot().unwrap().data.active.is_empty());
    assert_eq!(
        store.transact(&original_request, proposal).unwrap(),
        receipt
    );
}

#[test]
fn application_validation_errors_remain_typed() {
    #[derive(Debug)]
    enum ValidationError {
        Denied,
        Storage(RepositoryError),
    }
    impl From<RepositoryError> for ValidationError {
        fn from(error: RepositoryError) -> Self {
            Self::Storage(error)
        }
    }
    let (_root, store, before) = setup();
    let result = store.transact(&request(&before, "denied"), |data| {
        data.active.clear();
        Err(ValidationError::Denied)
    });
    assert!(matches!(result, Err(ValidationError::Denied)));
    assert_eq!(store.snapshot().unwrap(), before);
    // Persistence failures also preserve their structured variants.
    let empty = OperationRequest {
        operation_key: String::new(),
        ..request(&before, "")
    };
    let result = store.transact(&empty, |_| Err(ValidationError::Denied));
    assert!(matches!(
        result,
        Err(ValidationError::Storage(RepositoryError::InvalidInput(_)))
    ));
}

fn publication_steps() -> usize {
    let (_root, store, before) = setup();
    store
        .transact(&request(&before, "import"), proposal)
        .unwrap();
    store.files.steps.get()
}

#[test]
fn failed_nested_publication_cannot_be_overwritten_by_outer_operation() {
    let (root, store, before) = setup();
    let outer = store.transact(&request(&before, "outer"), |data| -> Result<_> {
        store.files.remaining_steps.set(Some(3));
        assert!(store
            .transact(&request(&before, "import"), proposal)
            .is_err());
        data.active[0].title = "Outer edit".into();
        Ok(BTreeMap::new())
    });
    assert!(matches!(outer, Err(RepositoryError::CorruptStorage { .. })));
    assert_recovered(root.path(), &before);
    assert!(store.receipt("outer").unwrap().is_none());
    assert!(store.receipt("import").unwrap().is_some());
}

#[test]
fn initialization_recovers_without_creating_missing_task_files() {
    let root = TempDir::new().unwrap();
    fs::create_dir(root.path().join(".pearls")).unwrap();
    let store = RepositoryStore::open(root.path()).unwrap();
    store.snapshot().unwrap();
    let steps = store.files.steps.get();
    for step in 1..=steps {
        let root = TempDir::new().unwrap();
        fs::create_dir(root.path().join(".pearls")).unwrap();
        let store = RepositoryStore::open(root.path()).unwrap();
        store.files.remaining_steps.set(Some(step));
        assert!(store.snapshot().is_err());
        let reopened = RepositoryStore::open(root.path()).unwrap();
        assert_eq!(reopened.snapshot().unwrap().data, RepositoryData::default());
        assert!(reopened.files.read(ISSUES).unwrap().is_none());
        assert!(reopened.files.read(ARCHIVE).unwrap().is_none());
        assert!(reopened.files.read(STATE).unwrap().is_some());
        assert!(reopened.files.read(JOURNAL).unwrap().is_none());
    }
}

fn assert_recovered(root: &Path, before: &RepositorySnapshot) {
    let store = RepositoryStore::open(root).unwrap();
    let recovered = store.snapshot().expect("recover publication");
    match store.receipt("import").unwrap() {
        Some(receipt) => {
            assert_eq!(recovered.revision, receipt.after_revision);
            assert_eq!(recovered.data.active[0].id, "prl-000003");
            assert_eq!(recovered.data.archived.len(), 2);
            assert_eq!(receipt.created_ids["new"], "prl-000003");
            assert_eq!(
                store
                    .transact(&request(before, "import"), proposal)
                    .unwrap(),
                receipt
            );
        }
        None => assert_eq!(recovered, *before),
    }
    assert!(!store.files.path(JOURNAL).exists());
}

#[test]
fn io_errors_at_every_durable_boundary_are_recoverable() {
    for step in 1..=publication_steps() {
        let (root, store, before) = setup();
        store.files.remaining_steps.set(Some(step));
        assert!(
            store
                .transact(&request(&before, "import"), proposal)
                .is_err(),
            "step {step}"
        );
        assert_recovered(root.path(), &before);
    }
}

#[test]
fn process_exit_at_every_durable_boundary_is_recoverable() {
    for step in 1..=publication_steps() {
        let (root, _, before) = setup();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "repository::tests::crash_worker", "--nocapture"])
            .env("PEARLS_CRASH_REPOSITORY", root.path())
            .env("PEARLS_CRASH_STEP", step.to_string())
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(86), "step {step}");
        assert_recovered(root.path(), &before);
    }
}

#[test]
fn crash_worker() {
    let Some(root) = std::env::var_os("PEARLS_CRASH_REPOSITORY") else {
        return;
    };
    let step = std::env::var("PEARLS_CRASH_STEP")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let store = RepositoryStore::open(root).unwrap();
    let before = store.snapshot().unwrap();
    store.files.remaining_steps.set(Some(step));
    store.files.crash.set(true);
    store
        .transact(&request(&before, "import"), proposal)
        .unwrap();
}

#[test]
fn conflicting_external_edit_blocks_recovery_without_overwriting_files() {
    let (root, store, before) = setup();
    store.files.remaining_steps.set(Some(3));
    assert!(store
        .transact(&request(&before, "import"), proposal)
        .is_err());
    assert!(store.files.path(JOURNAL).exists());
    let active = store.files.read(ISSUES).unwrap();
    let mut external = store.files.read(ARCHIVE).unwrap().unwrap();
    external.push(b'\n');
    fs::write(store.files.path(ARCHIVE), &external).unwrap();
    let reopened = RepositoryStore::open(root.path()).unwrap();
    assert!(matches!(
        reopened.snapshot(),
        Err(RepositoryError::CorruptStorage { .. })
    ));
    assert_eq!(reopened.files.read(ISSUES).unwrap(), active);
    assert_eq!(reopened.files.read(ARCHIVE).unwrap().unwrap(), external);
}

#[test]
fn future_schema_and_corrupt_journal_are_preserved_and_rejected() {
    let (_root, store, _) = setup();
    let original = store.files.read(STATE).unwrap().unwrap();
    let mut state: serde_json::Value = serde_json::from_slice(&original).unwrap();
    state["schema_version"] = 99.into();
    fs::write(store.files.path(STATE), serde_json::to_vec(&state).unwrap()).unwrap();
    assert!(matches!(
        store.snapshot(),
        Err(RepositoryError::UnsupportedSchema { found: 99, .. })
    ));
    fs::write(store.files.path(STATE), &original).unwrap();
    fs::write(store.files.path(JOURNAL), b"{truncated").unwrap();
    assert!(matches!(
        store.snapshot(),
        Err(RepositoryError::CorruptStorage { .. })
    ));
    assert_eq!(store.files.read(STATE).unwrap().unwrap(), original);
    assert!(store.files.path(JOURNAL).exists());
}

#[test]
#[cfg(unix)]
fn managed_symlinks_are_rejected() {
    let (root, store, _) = setup();
    let target = root.path().join("external");
    fs::write(&target, b"do not touch").unwrap();
    fs::remove_file(store.files.path(STATE)).unwrap();
    std::os::unix::fs::symlink(&target, store.files.path(STATE)).unwrap();
    assert!(store.snapshot().is_err());
    assert_eq!(fs::read(&target).unwrap(), b"do not touch");
}

#[test]
fn maintenance_preserves_receipts_and_replays_before_running_callback() {
    let (root, store, before) = setup();
    let receipt = store
        .transact(&request(&before, "original"), |_| -> Result<_> {
            Ok(BTreeMap::new())
        })
        .unwrap();
    fs::write(store.files.path(ISSUES), "not JSON\n").unwrap();
    let repaired = store
        .repair(
            "repair",
            "doctor:v1",
            |active, _| -> Result<RepositoryData> {
                assert_eq!(active, "not JSON\n");
                Ok(before.data.clone())
            },
        )
        .unwrap();
    let reopened = RepositoryStore::open(root.path()).unwrap();
    assert_eq!(reopened.receipt("original").unwrap(), Some(receipt));
    assert_eq!(
        reopened.snapshot().unwrap().revision,
        repaired.after_revision
    );
    assert_eq!(
        reopened
            .repair("repair", "doctor:v1", |_, _| -> Result<RepositoryData> {
                Err(RepositoryError::InvalidInput(
                    "Replay must skip callback".into(),
                ))
            })
            .unwrap(),
        repaired
    );
    assert!(matches!(
        reopened.repair("repair", "other", |_, _| Ok(before.data.clone())),
        Err(RepositoryError::IdempotencyConflict { .. })
    ));
}

#[test]
fn maintenance_failure_boundaries_preserve_corrupt_input_or_complete_repair() {
    for step in 1..=publication_steps() {
        let (root, store, before) = setup();
        fs::write(store.files.path(ISSUES), "not JSON\n").unwrap();
        let archive_before = fs::read(store.files.path(ARCHIVE)).unwrap();
        store.files.remaining_steps.set(Some(step));
        let result = store.repair("repair", "test", |_, _| -> Result<RepositoryData> {
            let mut data = before.data.clone();
            proposal(&mut data)?;
            Ok(data)
        });
        assert!(result.is_err(), "step {step}");
        let reopened = RepositoryStore::open(root.path()).unwrap();
        match reopened.snapshot() {
            Ok(snapshot) => {
                let receipt = reopened.receipt("repair").unwrap().unwrap();
                assert_eq!(snapshot.revision, receipt.after_revision);
                assert_eq!(snapshot.data.active[0].id, "prl-000003");
                assert_eq!(snapshot.data.archived.len(), 2);
            }
            Err(RepositoryError::CorruptStorage { .. }) => {
                assert_eq!(
                    fs::read_to_string(reopened.files.path(ISSUES)).unwrap(),
                    "not JSON\n"
                );
                assert_eq!(
                    fs::read(reopened.files.path(ARCHIVE)).unwrap(),
                    archive_before
                );
                let state: State =
                    serde_json::from_slice(&fs::read(reopened.files.path(STATE)).unwrap()).unwrap();
                assert!(!state.receipts.contains_key("repair"));
            }
            other => panic!("Unexpected recovery result: {other:?}"),
        }
        assert!(reopened.files.read(JOURNAL).unwrap().is_none());
    }
}
