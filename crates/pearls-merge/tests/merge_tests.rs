// Rust guideline compliant 2026-02-06

//! Tests for the Pearls merge driver.

use pearls_core::{DepType, Dependency, Pearl, Status};
use pearls_merge::merge::{merge_with_conflicts, three_way_merge};

fn base_pearl(id: &str) -> Pearl {
    Pearl {
        id: id.to_string(),
        title: "Base".to_string(),
        description: String::new(),
        status: Status::Open,
        priority: 2,
        created_at: 1000,
        updated_at: 1000,
        author: "author".to_string(),
        labels: vec!["core".to_string()],
        deps: Vec::new(),
        metadata: Default::default(),
    }
}

#[test]
fn test_three_way_merge_preserves_single_side_change() {
    let mut ours = base_pearl("prl-abc123");
    let theirs = base_pearl("prl-abc123");
    ours.title = "Updated".to_string();
    ours.updated_at = 2000;

    let merged = three_way_merge(vec![], vec![ours.clone()], vec![theirs]).unwrap();
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].title, "Updated");
}

#[test]
fn test_merge_unions_lists() {
    let mut ours = base_pearl("prl-abc123");
    let mut theirs = base_pearl("prl-abc123");
    ours.updated_at = 2000;
    theirs.updated_at = 1500;
    ours.labels.push("alpha".to_string());
    theirs.labels.push("beta".to_string());
    ours.deps.push(Dependency {
        target_id: "prl-aaa111".to_string(),
        dep_type: DepType::Blocks,
    });
    theirs.deps.push(Dependency {
        target_id: "prl-bbb222".to_string(),
        dep_type: DepType::Related,
    });

    let merged = three_way_merge(vec![], vec![ours], vec![theirs]).unwrap();
    let merged = &merged[0];
    assert!(merged.labels.contains(&"alpha".to_string()));
    assert!(merged.labels.contains(&"beta".to_string()));
    assert!(merged.deps.iter().any(|dep| dep.target_id == "prl-aaa111"));
    assert!(merged.deps.iter().any(|dep| dep.target_id == "prl-bbb222"));
}

#[test]
fn test_conflict_detection() {
    let mut ours = base_pearl("prl-abc123");
    let mut theirs = base_pearl("prl-abc123");
    ours.title = "Ours".to_string();
    theirs.title = "Theirs".to_string();
    ours.updated_at = 2000;
    theirs.updated_at = 2000;

    let (_merged, conflicts) = merge_with_conflicts(vec![], vec![ours], vec![theirs]).unwrap();
    assert_eq!(conflicts.len(), 1);
}
