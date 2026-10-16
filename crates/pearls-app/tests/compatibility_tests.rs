// Rust guideline compliant 2026-10-16

//! Legacy JSONL, reserved keys, and process-boundary compatibility.

use pearls_app::{service::*, ErrorCode, ErrorEnvelope};
use pearls_core::Pearl;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn repository() -> (TempDir, GraphService) {
    let root = TempDir::new().unwrap();
    fs::create_dir(root.path().join(".pearls")).unwrap();
    let service = GraphService::open(root.path()).unwrap();
    (root, service)
}

#[test]
fn legacy_jsonl_defaults_remain_readable_without_rewriting_task_files() {
    let (root, service) = repository();
    let record = "{\"id\":\"prl-000001\",\"title\":\"Legacy\",\"status\":\"closed\",\"created_at\":1,\"updated_at\":1,\"author\":\"tester\"}\n";
    let path = root.path().join(".pearls/issues.jsonl");
    fs::write(&path, record).unwrap();
    let snapshot = service.snapshot().unwrap();
    assert_eq!(snapshot.data.active[0].priority, 2);
    assert!(snapshot.data.active[0].metadata.is_empty());
    assert!(snapshot.data.active[0].comments.is_empty());
    assert_eq!(fs::read_to_string(path).unwrap(), record);
    let state: serde_json::Value = serde_json::from_slice(
        &fs::read(root.path().join(".pearls/transaction-state.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(state["schema_version"], 2);
}

#[test]
fn service_operations_never_modify_hooks_or_git_or_repository_configuration() {
    let (root, service) = repository();
    fs::create_dir_all(root.path().join(".git/hooks")).unwrap();
    let git_config = root.path().join(".git/config");
    let hook = root.path().join(".git/hooks/pre-commit");
    let config = root.path().join(".pearls/config.toml");
    fs::write(&git_config, "[core]\nrepositoryformatversion = 0\n").unwrap();
    fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
    fs::write(&config, "# Preserve this comment\ndefault_priority = 1\n").unwrap();
    let originals: Vec<_> = [&git_config, &hook, &config]
        .iter()
        .map(|path| fs::read(path).unwrap())
        .collect();
    let reopened = GraphService::open(root.path()).unwrap();
    reopened.snapshot().unwrap();
    let mut task = Pearl::new("Compatibility".into(), "tester".into());
    task.id = "prl-000001".into();
    service
        .execute(|_| Ok(vec![Mutation::Import { tasks: vec![task] }]))
        .unwrap();
    for (path, original) in [&git_config, &hook, &config].iter().zip(originals) {
        assert_eq!(fs::read(path).unwrap(), original);
    }
}

#[test]
fn expired_service_keys_have_structured_errors_and_cannot_reapply() {
    let (root, service) = repository();
    let mut task = Pearl::new("Retained".into(), "tester".into());
    task.id = "prl-000001".into();
    let request = BatchRequest {
        operation_key: "retained".into(),
        expected_revision: service.snapshot().unwrap().revision,
        mutations: vec![Mutation::Import { tasks: vec![task] }],
    };
    service.apply(&request).unwrap();
    let before = service.snapshot().unwrap();
    // Model an aged stored receipt; clock-boundary behavior is tested in core.
    let path = root.path().join(".pearls/transaction-state.json");
    let mut state: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["receipt_deadlines"]["retained"] = json!(0);
    fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();
    for error in [
        service.receipt("retained").unwrap_err(),
        service.apply(&request).unwrap_err(),
    ] {
        assert_eq!(error.code(), ErrorCode::OperationExpired);
        let envelope = serde_json::to_value(ErrorEnvelope::from_error(&error)).unwrap();
        assert_eq!(envelope["code"], "operation_expired");
        assert_eq!(envelope["details"]["operation_key"], "retained");
    }
    assert_eq!(service.snapshot().unwrap(), before);
    assert_eq!(service.receipt("unknown").unwrap(), None);
}

#[test]
fn initialization_explicitly_migrates_legacy_state_without_losing_original_replay() {
    let (root, service) = repository();
    let mut task = Pearl::new("Legacy".into(), "tester".into());
    task.id = "prl-000001".into();
    let request = BatchRequest {
        operation_key: "legacy".into(),
        expected_revision: service.snapshot().unwrap().revision,
        mutations: vec![Mutation::Import { tasks: vec![task] }],
    };
    let receipt = service.apply(&request).unwrap();
    let path = root.path().join(".pearls/transaction-state.json");
    let mut state: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    state["schema_version"] = json!(1);
    state.as_object_mut().unwrap().remove("receipt_deadlines");
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    assert_eq!(
        service.snapshot().unwrap_err().code(),
        ErrorCode::CorruptStorage
    );
    service.initialize_files().unwrap();
    assert_eq!(service.apply(&request).unwrap(), receipt);
    assert_eq!(service.snapshot().unwrap().data.active.len(), 1);
}
