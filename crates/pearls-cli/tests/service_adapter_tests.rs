// Rust guideline compliant 2026-10-15

//! CLI and embedded consumers share revisions, validation, and receipts.

use pearls_app::{service::*, ErrorCode};
use pearls_core::{DepType, Dependency, Pearl, Status};
use std::{
    path::Path,
    process::{Command, Output},
};
use tempfile::TempDir;

fn repository() -> (TempDir, GraphService) {
    let root = TempDir::new().unwrap();
    std::fs::create_dir(root.path().join(".pearls")).unwrap();
    let service = GraphService::open(root.path()).unwrap();
    service.initialize_files().unwrap();
    (root, service)
}
fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn task(id: &str) -> Pearl {
    let mut task = Pearl::new("Task".into(), "tester".into());
    task.id = id.into();
    task
}

#[test]
fn cli_write_changes_service_revision_without_losing_metadata() {
    let (root, service) = repository();
    let mut task = task("prl-000001");
    task.metadata
        .insert("opaque.report".into(), "reference".into());
    service
        .execute(|_| Ok(vec![Mutation::Import { tasks: vec![task] }]))
        .unwrap();
    let before = service.snapshot().unwrap();
    let output = cli(
        root.path(),
        &["update", "prl-000001", "--title", "CLI title", "--json"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["pearl"]["title"], "CLI title");
    let after = service.snapshot().unwrap();
    assert_ne!(after.revision, before.revision);
    assert_eq!(after.data.active[0].metadata["opaque.report"], "reference");
    assert_eq!(
        service
            .apply(&BatchRequest {
                operation_key: "stale".into(),
                expected_revision: before.revision,
                mutations: vec![Mutation::SetStatus {
                    task: TaskRef::Id("prl-000001".into()),
                    status: Status::Closed
                }],
            })
            .unwrap_err()
            .code(),
        ErrorCode::RevisionConflict
    );
}

#[test]
fn cli_import_rejects_cycles_atomically() {
    let (root, service) = repository();
    let mut first = task("prl-000001");
    let mut second = task("prl-000002");
    first.deps.push(Dependency {
        target_id: second.id.clone(),
        dep_type: DepType::Blocks,
    });
    second.deps.push(Dependency {
        target_id: first.id.clone(),
        dep_type: DepType::Blocks,
    });
    let path = root.path().join("incoming.jsonl");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n",
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        ),
    )
    .unwrap();
    let before = service.snapshot().unwrap();
    let output = cli(root.path(), &["import", "beads", path.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .to_lowercase()
        .contains("cycle"));
    assert_eq!(service.snapshot().unwrap(), before);
}

#[test]
fn cli_close_respects_archived_prerequisites_and_preserves_original_receipt() {
    let (root, service) = repository();
    let mut first = task("prl-000001");
    let mut second = task("prl-000002");
    first.deps.push(Dependency {
        target_id: second.id.clone(),
        dep_type: DepType::Blocks,
    });
    second.status = Status::Closed;
    second.updated_at = 1;
    let created = service
        .execute(|_| {
            Ok(vec![Mutation::Import {
                tasks: vec![first, second],
            }])
        })
        .unwrap();
    let output = cli(root.path(), &["compact", "--threshold-days", "1", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(service.snapshot().unwrap().data.archived.len(), 1);
    let output = cli(root.path(), &["close", "prl-000001", "--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        service.snapshot().unwrap().data.active[0].status,
        Status::Closed
    );
    assert_eq!(
        service.receipt(&created.receipt.operation_key).unwrap(),
        Some(created.receipt)
    );
}
