// Rust guideline compliant 2026-02-06

//! Integration tests for Pearls hooks.

use pearls_core::{DepType, Dependency, Pearl, Status, Storage};
use pearls_hooks::{post_merge_hook, pre_commit_hook};
use tempfile::TempDir;

fn create_pearl(id: &str) -> Pearl {
    Pearl {
        id: id.to_string(),
        title: "Test".to_string(),
        description: String::new(),
        status: Status::Open,
        priority: 2,
        created_at: 1000,
        updated_at: 1000,
        author: "tester".to_string(),
        labels: Vec::new(),
        deps: Vec::new(),
        metadata: Default::default(),
        comments: Vec::new(),
    }
}

#[test]
fn test_pre_commit_auto_close() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    std::fs::create_dir(repo_path.join(".git")).expect("Failed to create .git");
    std::fs::create_dir(repo_path.join(".pearls")).expect("Failed to create .pearls");

    let mut storage =
        Storage::new(repo_path.join(".pearls/issues.jsonl")).expect("Failed to create storage");
    let pearl = create_pearl("prl-abc123");
    storage.save(&pearl).expect("Failed to save pearl");

    std::fs::write(repo_path.join(".git/COMMIT_EDITMSG"), "Fixes (prl-abc123)")
        .expect("Failed to write commit message");

    pre_commit_hook(repo_path).expect("Pre-commit hook failed");

    let updated = storage
        .load_by_id("prl-abc123")
        .expect("Failed to load pearl");
    assert_eq!(updated.status, Status::Closed);
}

#[test]
fn test_post_merge_detects_orphaned_deps() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path();
    std::fs::create_dir(repo_path.join(".pearls")).expect("Failed to create .pearls");

    let mut pearl = create_pearl("prl-abc123");
    pearl.deps.push(Dependency {
        target_id: "prl-missing".to_string(),
        dep_type: DepType::Blocks,
    });

    let mut storage =
        Storage::new(repo_path.join(".pearls/issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl).expect("Failed to save pearl");

    // Supported reads reject missing endpoints after an external merge.
    let error = post_merge_hook(repo_path).expect_err("Missing endpoint must fail validation");
    assert!(error.to_string().contains("prl-missing"));
}

#[test]
fn auto_close_is_atomic_and_uses_shared_blocker_validation() {
    let root = TempDir::new().unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    std::fs::create_dir(root.path().join(".pearls")).unwrap();
    let service = pearls_app::service::GraphService::open(root.path()).unwrap();
    let mut dependent = create_pearl("prl-000001");
    dependent.deps.push(Dependency {
        target_id: "prl-000003".into(),
        dep_type: DepType::Blocks,
    });
    service
        .execute(|_| {
            Ok(vec![pearls_app::service::Mutation::Import {
                tasks: vec![
                    create_pearl("prl-000002"),
                    dependent,
                    create_pearl("prl-000003"),
                ],
            }])
        })
        .unwrap();
    std::fs::write(
        root.path().join(".git/COMMIT_EDITMSG"),
        "Fixes (prl-000002) Fixes (prl-000001)",
    )
    .unwrap();
    let before = service.snapshot().unwrap();
    assert!(pre_commit_hook(root.path()).is_err());
    assert_eq!(service.snapshot().unwrap(), before);
}

#[test]
fn auto_close_orders_prerequisites_and_skips_already_closed_tasks() {
    let root = TempDir::new().unwrap();
    std::fs::create_dir(root.path().join(".git")).unwrap();
    std::fs::create_dir(root.path().join(".pearls")).unwrap();
    let service = pearls_app::service::GraphService::open(root.path()).unwrap();
    let mut dependent = create_pearl("prl-000001");
    dependent.deps.push(Dependency {
        target_id: "prl-000002".into(),
        dep_type: DepType::Blocks,
    });
    service
        .execute(|_| {
            Ok(vec![pearls_app::service::Mutation::Import {
                tasks: vec![dependent, create_pearl("prl-000002")],
            }])
        })
        .unwrap();
    std::fs::write(
        root.path().join(".git/COMMIT_EDITMSG"),
        "Fixes (prl-000001) Fixes (prl-000002)",
    )
    .unwrap();
    pre_commit_hook(root.path()).unwrap();
    let closed = service.snapshot().unwrap();
    assert!(closed
        .data
        .active
        .iter()
        .all(|task| task.status == Status::Closed));
    pre_commit_hook(root.path()).unwrap();
    assert_eq!(service.snapshot().unwrap(), closed);
}
