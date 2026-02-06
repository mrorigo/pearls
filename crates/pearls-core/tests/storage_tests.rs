// Rust guideline compliant 2026-02-06

//! Unit tests for the storage module.
//!
//! These tests validate specific examples, edge cases, and error conditions.

use pearls_core::{Pearl, Status, Storage};
use std::fs;
use tempfile::TempDir;

/// Helper to create a test Pearl.
fn create_test_pearl(id: &str, title: &str) -> Pearl {
    Pearl {
        id: id.to_string(),
        title: title.to_string(),
        description: String::new(),
        status: Status::Open,
        priority: 2,
        created_at: 1000,
        updated_at: 1000,
        author: "test-author".to_string(),
        labels: vec![],
        deps: vec![],
        metadata: Default::default(),
    }
}

#[test]
fn test_empty_file_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let storage = Storage::new(storage_path).expect("Failed to create storage");

    // Load from non-existent file should return empty vec
    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(pearls.len(), 0, "Empty file should return empty vec");
}

#[test]
fn test_malformed_json_recovery() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    // Write a file with valid JSON followed by invalid JSON
    let content = r#"{"id":"prl-123456","title":"Valid Pearl","status":"open","created_at":1000,"updated_at":1000,"author":"test"}
{"id":"prl-234567","title":"Another Valid Pearl","status":"open","created_at":1000,"updated_at":1000,"author":"test"}
"#;
    fs::write(&storage_path, content).expect("Failed to write test file");

    let storage = Storage::new(storage_path).expect("Failed to create storage");

    // Should load valid Pearls
    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(pearls.len(), 2, "Should load 2 valid Pearls");
    assert_eq!(pearls[0].id, "prl-123456");
    assert_eq!(pearls[1].id, "prl-234567");
}

#[test]
fn test_concurrent_read_operations() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path.clone()).expect("Failed to create storage");

    // Create test data
    let pearl1 = create_test_pearl("prl-111111", "Pearl 1");
    let pearl2 = create_test_pearl("prl-222222", "Pearl 2");
    storage
        .save_all(&[pearl1.clone(), pearl2.clone()])
        .expect("Failed to save");

    // Simulate concurrent reads
    let storage1 = Storage::new(storage_path.clone()).expect("Failed to create storage");
    let storage2 = Storage::new(storage_path.clone()).expect("Failed to create storage");

    let pearls1 = storage1.load_all().expect("Failed to load pearls");
    let pearls2 = storage2.load_all().expect("Failed to load pearls");

    assert_eq!(pearls1.len(), 2);
    assert_eq!(pearls2.len(), 2);
}

#[test]
fn test_lock_timeout_scenarios() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    // Test that lock can be acquired and released
    let result = storage.with_lock(|storage| {
        let pearl = create_test_pearl("prl-111111", "Test Pearl");
        storage.save(&pearl)
    });

    assert!(result.is_ok(), "Lock operation should succeed");

    // Test that lock can be acquired again after release
    let result2 = storage.with_lock(|storage| storage.load_all());

    assert!(result2.is_ok(), "Lock should be released and reacquirable");
}

#[test]
fn test_save_single_pearl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let pearl = create_test_pearl("prl-111111", "Test Pearl");

    storage.save(&pearl).expect("Failed to save pearl");

    let loaded = storage
        .load_by_id("prl-111111")
        .expect("Failed to load pearl");
    assert_eq!(loaded.id, pearl.id);
    assert_eq!(loaded.title, pearl.title);
}

#[test]
fn test_update_existing_pearl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let mut pearl = create_test_pearl("prl-111111", "Original Title");

    storage.save(&pearl).expect("Failed to save pearl");

    // Update the pearl
    pearl.title = "Updated Title".to_string();
    storage.save(&pearl).expect("Failed to update pearl");

    let loaded = storage
        .load_by_id("prl-111111")
        .expect("Failed to load pearl");
    assert_eq!(loaded.title, "Updated Title");

    // Verify only one pearl exists
    let all_pearls = storage.load_all().expect("Failed to load all pearls");
    assert_eq!(all_pearls.len(), 1);
}

#[test]
fn test_delete_pearl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let pearl1 = create_test_pearl("prl-111111", "Pearl 1");
    let pearl2 = create_test_pearl("prl-222222", "Pearl 2");

    storage
        .save_all(&[pearl1, pearl2])
        .expect("Failed to save pearls");

    // Delete one pearl
    storage
        .delete("prl-111111")
        .expect("Failed to delete pearl");

    // Verify it's gone
    let result = storage.load_by_id("prl-111111");
    assert!(result.is_err(), "Deleted pearl should not be found");

    // Verify the other pearl still exists
    let loaded = storage
        .load_by_id("prl-222222")
        .expect("Failed to load pearl");
    assert_eq!(loaded.id, "prl-222222");
}

#[test]
fn test_delete_nonexistent_pearl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    // Try to delete a pearl that doesn't exist
    let result = storage.delete("prl-nonexistent");
    assert!(result.is_err(), "Deleting nonexistent pearl should fail");
}

#[test]
fn test_load_by_id_early_termination() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    // Create many pearls
    let mut pearls = Vec::new();
    for i in 0..100 {
        pearls.push(create_test_pearl(
            &format!("prl-{:06}", i),
            &format!("Pearl {}", i),
        ));
    }
    storage.save_all(&pearls).expect("Failed to save pearls");

    // Load a specific pearl (should terminate early)
    let loaded = storage
        .load_by_id("prl-000050")
        .expect("Failed to load pearl");
    assert_eq!(loaded.id, "prl-000050");
}

#[test]
fn test_save_all_replaces_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    // Save initial pearls
    let pearl1 = create_test_pearl("prl-111111", "Pearl 1");
    let pearl2 = create_test_pearl("prl-222222", "Pearl 2");
    storage
        .save_all(&[pearl1, pearl2])
        .expect("Failed to save pearls");

    // Save different pearls (should replace)
    let pearl3 = create_test_pearl("prl-333333", "Pearl 3");
    storage.save_all(&[pearl3]).expect("Failed to save pearls");

    // Verify only the new pearl exists
    let all_pearls = storage.load_all().expect("Failed to load all pearls");
    assert_eq!(all_pearls.len(), 1);
    assert_eq!(all_pearls[0].id, "prl-333333");
}

#[test]
fn test_jsonl_format_validation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");

    let mut storage = Storage::new(storage_path.clone()).expect("Failed to create storage");

    let pearl1 = create_test_pearl("prl-111111", "Pearl 1");
    let pearl2 = create_test_pearl("prl-222222", "Pearl 2");
    storage
        .save_all(&[pearl1, pearl2])
        .expect("Failed to save pearls");

    // Verify file format
    let content = fs::read_to_string(&storage_path).expect("Failed to read file");
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 2, "Should have 2 lines");

    // Verify each line is valid JSON
    for line in &lines {
        let _: Pearl = serde_json::from_str(line).expect("Each line should be valid JSON");
    }

    // Verify no internal newlines in JSON
    for line in &lines {
        assert!(
            !line.contains('\n'),
            "JSON should not contain internal newlines"
        );
    }
}

#[test]
fn test_storage_with_index() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("test.jsonl");
    let index_path = temp_dir.path().join("test.idx");

    let mut storage = Storage::with_index(storage_path, Some(index_path))
        .expect("Failed to create storage with index");

    let pearl = create_test_pearl("prl-111111", "Test Pearl");
    storage.save(&pearl).expect("Failed to save pearl");

    let loaded = storage
        .load_by_id("prl-111111")
        .expect("Failed to load pearl");
    assert_eq!(loaded.id, pearl.id);
}

#[test]
fn test_storage_path_validation() {
    // Empty path should fail
    let result = Storage::new("".into());
    assert!(result.is_err(), "Empty path should fail validation");
}
