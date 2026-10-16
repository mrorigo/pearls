// Rust guideline compliant 2026-10-16

//! Receipt expiration, schema migration, and recovery contracts.

use super::tests::{request, setup};
use super::*;
use std::fs;

const DAY: i64 = 86_400;

#[test]
fn expiry_reserves_keys_and_does_not_change_graph_revision() {
    let (root, store, before) = setup();
    store.now_override.set(Some(1000));
    let operation = request(&before, "retained");
    let receipt = store
        .transact(&operation, |_| -> Result<_> { Ok(BTreeMap::new()) })
        .unwrap();
    store.now_override.set(Some(1000 + 30 * DAY - 1));
    assert_eq!(store.receipt("retained").unwrap(), Some(receipt.clone()));
    let before_expiration = store.snapshot().unwrap();
    store.now_override.set(Some(1000 + 30 * DAY));
    assert!(matches!(
        store.receipt("retained"),
        Err(RepositoryError::OperationExpired { .. })
    ));
    assert!(matches!(
        store.transact(&operation, |_| -> Result<_> {
            panic!("Expired replay must skip validation")
        }),
        Err(RepositoryError::OperationExpired { .. })
    ));
    let mut different = operation;
    different.request_digest = "changed".into();
    different.expected_revision = receipt.after_revision;
    assert!(matches!(
        store.transact(&different, |_| -> Result<_> {
            panic!("Expired key must never run")
        }),
        Err(RepositoryError::OperationExpired { .. })
    ));
    assert_eq!(store.snapshot().unwrap(), before_expiration);
    assert_eq!(store.receipt("unknown").unwrap(), None);
    let bytes = fs::read(store.files.path(STATE)).unwrap();
    let state: State = serde_json::from_slice(&bytes).unwrap();
    assert!(state.receipts.is_empty() && state.receipt_deadlines.is_empty());
    assert!(state.expired_keys.contains("retained"));
    let reopened = RepositoryStore::open(root.path()).unwrap();
    // A clock rollback or increased retention cannot resurrect an expired key.
    reopened.now_override.set(Some(1));
    assert!(matches!(
        reopened.receipt("retained"),
        Err(RepositoryError::OperationExpired { .. })
    ));
}

#[test]
fn changed_retention_applies_only_to_new_receipts_and_compaction_preserves_deadlines() {
    let (_root, store, before) = setup();
    store.now_override.set(Some(1000));
    fs::write(
        store.files.path("config.toml"),
        "receipt_retention_days = 7\n",
    )
    .unwrap();
    let first = store
        .transact(&request(&before, "short"), |_| -> Result<_> {
            Ok(BTreeMap::new())
        })
        .unwrap();
    fs::write(
        store.files.path("config.toml"),
        "receipt_retention_days = 90\n",
    )
    .unwrap();
    store
        .repair("archive", "test", |_, _| -> Result<_> {
            let mut data = before.data.clone();
            data.active[0].status = crate::Status::Closed;
            data.archived.append(&mut data.active);
            Ok(data)
        })
        .unwrap();
    let state: State = serde_json::from_slice(&fs::read(store.files.path(STATE)).unwrap()).unwrap();
    assert_eq!(state.receipt_deadlines["short"], 1000 + 7 * DAY);
    assert_eq!(state.receipt_deadlines["archive"], 1000 + 90 * DAY);
    store.now_override.set(Some(1000 + 7 * DAY));
    assert!(matches!(
        store.receipt("short"),
        Err(RepositoryError::OperationExpired { .. })
    ));
    assert!(store.receipt("archive").unwrap().is_some());
    assert!(store.snapshot().unwrap().data.active.is_empty());
    assert_ne!(store.snapshot().unwrap().revision, first.after_revision);
}

#[test]
fn invalid_retention_never_prevents_replay_of_a_retained_receipt() {
    let (_root, store, before) = setup();
    store.now_override.set(Some(1000));
    let operation = request(&before, "retained");
    let receipt = store
        .transact(&operation, |_| -> Result<_> { Ok(BTreeMap::new()) })
        .unwrap();
    fs::write(
        store.files.path("config.toml"),
        "receipt_retention_days = 0\n",
    )
    .unwrap();
    assert_eq!(
        store
            .transact(&operation, |_| -> Result<_> {
                panic!("Replay must skip callback")
            })
            .unwrap(),
        receipt
    );
    assert_eq!(store.receipt("retained").unwrap(), Some(receipt));
}

fn legacy_state(store: &RepositoryStore) -> State {
    let mut state: State =
        serde_json::from_slice(&fs::read(store.files.path(STATE)).unwrap()).unwrap();
    state.schema_version = 1;
    state.receipt_deadlines.clear();
    state.expired_keys.clear();
    state
}

#[test]
fn v1_migration_is_explicit_and_preserves_receipts_and_task_data() {
    let (_root, store, before) = setup();
    store.now_override.set(Some(1000));
    let receipt = store
        .transact(&request(&before, "legacy"), |_| -> Result<_> {
            Ok(BTreeMap::new())
        })
        .unwrap();
    let legacy = serde_json::to_vec(&legacy_state(&store)).unwrap();
    assert!(!std::str::from_utf8(&legacy)
        .unwrap()
        .contains("receipt_deadlines"));
    fs::write(store.files.path(STATE), &legacy).unwrap();
    assert!(matches!(
        store.snapshot(),
        Err(RepositoryError::UnsupportedSchema { found: 1, .. })
    ));
    assert!(matches!(
        store.repair("ordinary", "test", |_, _| Ok(before.data.clone())),
        Err(RepositoryError::UnsupportedSchema { found: 1, .. })
    ));
    assert_eq!(fs::read(store.files.path(STATE)).unwrap(), legacy);
    store.now_override.set(Some(2000));
    let migration = store
        .migrate_and_repair("migration", "v2", |_, _| {
            Ok::<_, RepositoryError>(before.data.clone())
        })
        .unwrap();
    assert_eq!(store.snapshot().unwrap().data, before.data);
    assert_eq!(store.receipt("legacy").unwrap(), Some(receipt.clone()));
    assert_eq!(migration.before_revision, receipt.after_revision);
    let state: State = serde_json::from_slice(&fs::read(store.files.path(STATE)).unwrap()).unwrap();
    assert_eq!(state.schema_version, 2);
    assert_eq!(state.receipt_deadlines["legacy"], 2000 + 30 * DAY);
}

#[test]
fn explicit_migration_recovers_v1_journal_at_every_partial_publication_image() {
    for installed_targets in 0..=3 {
        let (_root, store, before) = setup();
        store.now_override.set(Some(1000));
        let legacy = legacy_state(&store);
        let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(store.files.path(STATE), &legacy_bytes).unwrap();
        let images = store.files.images().unwrap();
        let mut after_data = before.data.clone();
        after_data.active[0].title = "Legacy committed edit".into();
        let after = store.encode(&after_data).unwrap();
        let mut state = legacy;
        state.advance().unwrap();
        state.files = after.fingerprints();
        let receipt = OperationReceipt {
            operation_key: "legacy".into(),
            request_digest: "legacy-digest".into(),
            before_revision: before.revision.clone(),
            after_revision: state.revision(),
            created_ids: BTreeMap::new(),
        };
        state.receipts.insert("legacy".into(), receipt.clone());
        let state_after = serde_json::to_vec(&state).unwrap();
        let journal = Journal {
            schema_version: 1,
            before: images.fingerprints(),
            before_state: fingerprint(Some(&legacy_bytes)),
            after: after.clone(),
            state,
        };
        fs::write(
            store.files.path(JOURNAL),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        for (name, bytes) in [
            (ISSUES, after.issues.unwrap().into_bytes()),
            (ARCHIVE, after.archive.unwrap().into_bytes()),
            (STATE, state_after),
        ]
        .into_iter()
        .take(installed_targets)
        {
            fs::write(store.files.path(name), bytes).unwrap();
        }
        let pending = fs::read(store.files.path(JOURNAL)).unwrap();
        assert!(matches!(
            store.snapshot(),
            Err(RepositoryError::UnsupportedSchema { found: 1, .. })
        ));
        assert_eq!(fs::read(store.files.path(JOURNAL)).unwrap(), pending);
        store
            .migrate_and_repair("migration", "v2", |active, archived| -> Result<_> {
                assert!(active.contains("Legacy committed edit"));
                Ok(RepositoryData {
                    active: active
                        .lines()
                        .map(|line| serde_json::from_str(line).unwrap())
                        .collect(),
                    archived: archived
                        .lines()
                        .map(|line| serde_json::from_str(line).unwrap())
                        .collect(),
                })
            })
            .unwrap();
        assert_eq!(store.receipt("legacy").unwrap(), Some(receipt));
        assert_eq!(store.snapshot().unwrap().data, after_data);
        assert!(store.files.read(JOURNAL).unwrap().is_none());
    }
}

#[test]
fn expiration_housekeeping_recovers_at_every_durable_boundary() {
    let (_root, store, before) = setup();
    store.now_override.set(Some(1000));
    let operation = request(&before, "retained");
    store
        .transact(&operation, |_| -> Result<_> { Ok(BTreeMap::new()) })
        .unwrap();
    store.files.steps.set(0);
    store.now_override.set(Some(1000 + 30 * DAY));
    let current = store.snapshot().unwrap();
    let steps = store.files.steps.get();
    for step in 1..=steps {
        let (root, store, before) = setup();
        store.now_override.set(Some(1000));
        store
            .transact(&request(&before, "retained"), |_| -> Result<_> {
                Ok(BTreeMap::new())
            })
            .unwrap();
        store.now_override.set(Some(1000 + 30 * DAY));
        store.files.remaining_steps.set(Some(step));
        assert!(store.snapshot().is_err(), "step {step}");
        let reopened = RepositoryStore::open(root.path()).unwrap();
        reopened.now_override.set(Some(1000 + 30 * DAY));
        let recovered = reopened.snapshot().unwrap();
        assert_eq!(recovered.revision, current.revision);
        assert_eq!(recovered.data, before.data);
        assert!(matches!(
            reopened.receipt("retained"),
            Err(RepositoryError::OperationExpired { .. })
        ));
        assert!(reopened.files.read(JOURNAL).unwrap().is_none());
    }
}

#[test]
fn corrupt_retention_metadata_is_rejected_without_rewriting() {
    for mutation in 0..3 {
        let (_root, store, before) = setup();
        store.now_override.set(Some(1000));
        store
            .transact(&request(&before, "record"), |_| -> Result<_> {
                Ok(BTreeMap::new())
            })
            .unwrap();
        let mut state: serde_json::Value =
            serde_json::from_slice(&fs::read(store.files.path(STATE)).unwrap()).unwrap();
        match mutation {
            0 => {
                state.as_object_mut().unwrap().remove("receipt_deadlines");
            }
            1 => {
                state["receipt_deadlines"]["record"] = (-1).into();
            }
            _ => {
                state["expired_keys"] = serde_json::json!(["record"]);
            }
        }
        let bytes = serde_json::to_vec(&state).unwrap();
        fs::write(store.files.path(STATE), &bytes).unwrap();
        assert!(matches!(
            store.snapshot(),
            Err(RepositoryError::CorruptStorage { .. })
        ));
        assert_eq!(fs::read(store.files.path(STATE)).unwrap(), bytes);
    }
}
