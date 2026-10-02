// Rust guideline compliant 2026-02-06

//! Tests for the Pearls merge driver.

use pearls_core::{Comment, DepType, Dependency, Pearl, Status};
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
        comments: Vec::new(),
    }
}

fn comment(id: &str, body: &str, created_at: i64) -> Comment {
    Comment {
        id: id.to_string(),
        author: "author".to_string(),
        body: body.to_string(),
        created_at,
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

#[test]
fn test_merge_preserves_comments_from_older_side() {
    let dir = tempfile::TempDir::new().unwrap();
    let ancestor = base_pearl("prl-abc123");

    // Ours gets a comment, which bumps updated_at to wall-clock time.
    let mut ours = base_pearl("prl-abc123");
    let comment_id = ours
        .add_comment("author".to_string(), "keep me".to_string())
        .unwrap();

    // Theirs makes an unrelated edit later in wall-clock time, so theirs is newer.
    let mut theirs = base_pearl("prl-abc123");
    theirs.title = "Edited on the other branch".to_string();
    theirs.updated_at = ours.updated_at + 60;

    let ancestor_path = dir.path().join("ancestor.jsonl");
    let ours_path = dir.path().join("ours.jsonl");
    let theirs_path = dir.path().join("theirs.jsonl");
    std::fs::write(
        &ancestor_path,
        format!("{}\n", serde_json::to_string(&ancestor).unwrap()),
    )
    .unwrap();
    std::fs::write(
        &ours_path,
        format!("{}\n", serde_json::to_string(&ours).unwrap()),
    )
    .unwrap();
    std::fs::write(
        &theirs_path,
        format!("{}\n", serde_json::to_string(&theirs).unwrap()),
    )
    .unwrap();

    let merged = three_way_merge(vec![ancestor], vec![ours], vec![theirs]).unwrap();
    let merged = &merged[0];
    assert!(
        merged
            .comments
            .iter()
            .any(|comment| comment.id == comment_id),
        "comment {} was dropped by merge: merged pearl has {} comments",
        comment_id,
        merged.comments.len()
    );
}

#[test]
fn test_merge_keeps_comment_from_older_side_only() {
    let mut ours = base_pearl("prl-abc123");
    ours.updated_at = 1500;
    ours.comments
        .push(comment("cmt-older", "only on older side", 1400));

    let mut theirs = base_pearl("prl-abc123");
    theirs.title = "Edited later".to_string();
    theirs.updated_at = 2000;

    let merged = three_way_merge(vec![], vec![ours], vec![theirs]).unwrap();
    assert_eq!(
        merged[0].comments,
        vec![comment("cmt-older", "only on older side", 1400)],
        "comment present only on the older side must survive the merge"
    );
}

#[test]
fn test_merge_keeps_comment_from_newer_side_only() {
    let mut ours = base_pearl("prl-abc123");
    ours.updated_at = 1500;

    let mut theirs = base_pearl("prl-abc123");
    theirs.updated_at = 2000;
    theirs
        .comments
        .push(comment("cmt-newer", "only on newer side", 1900));

    let merged = three_way_merge(vec![], vec![ours], vec![theirs]).unwrap();
    assert_eq!(
        merged[0].comments,
        vec![comment("cmt-newer", "only on newer side", 1900)],
        "comment present only on the newer side must survive the merge"
    );
}

#[test]
fn test_merge_unions_comments_from_both_sides_without_duplicates() {
    let shared = comment("cmt-shared", "on both sides", 1200);
    let mut ours = base_pearl("prl-abc123");
    ours.updated_at = 1500;
    ours.comments.push(shared.clone());
    ours.comments.push(comment("cmt-ours", "only ours", 1400));

    let mut theirs = base_pearl("prl-abc123");
    theirs.updated_at = 2000;
    theirs.comments.push(shared.clone());
    theirs
        .comments
        .push(comment("cmt-theirs", "only theirs", 1600));

    let merged = three_way_merge(vec![], vec![ours.clone()], vec![theirs.clone()]).unwrap();
    let ids: Vec<&str> = merged[0].comments.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["cmt-shared", "cmt-ours", "cmt-theirs"],
        "each comment id must appear once, ordered by (created_at, id)"
    );

    let swapped = three_way_merge(vec![], vec![theirs], vec![ours]).unwrap();
    assert_eq!(
        swapped[0].comments, merged[0].comments,
        "comment order must not depend on which side is newer"
    );
}
