// Rust guideline compliant 2026-02-06

//! Unit tests for the storage module.
//!
//! These tests validate specific examples, edge cases, and error conditions.

use pearls_core::{identity::generate_id, storage::Index, Error, Pearl, Status, Storage};
use std::fs;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
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
        comments: Vec::new(),
    }
}

fn create_collision_pearl(title: &str, author: &str, timestamp: i64) -> Pearl {
    Pearl {
        id: generate_id(title, author, timestamp, 0),
        title: title.to_string(),
        description: String::new(),
        status: Status::Open,
        priority: 2,
        created_at: timestamp,
        updated_at: timestamp,
        author: author.to_string(),
        labels: vec![],
        deps: vec![],
        metadata: Default::default(),
        comments: Vec::new(),
    }
}

#[test]
fn test_create_new_retries_active_id_collision() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    let mut first = create_collision_pearl("Duplicate title", "author", 1704067200);
    let mut second = create_collision_pearl("Duplicate title", "author", 1704067200);
    let first_nonce_zero_id = first.id.clone();
    assert_eq!(first.id, second.id, "fixture should start with a collision");

    storage
        .create_new(&mut first, Some(&archive_path), 2)
        .expect("Failed to create first pearl");
    storage
        .create_new(&mut second, Some(&archive_path), 2)
        .expect("Failed to create second pearl");

    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(pearls.len(), 2, "create should append both pearls");
    assert_eq!(first.id, first_nonce_zero_id);
    assert_ne!(first.id, second.id, "second create should retry nonce");
    assert_eq!(
        second.id,
        generate_id("Duplicate title", "author", 1704067200, 1)
    );
}

#[test]
fn test_create_new_reserves_archived_ids() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");

    let archived = create_collision_pearl("Archived title", "author", 1704067200);
    let archived_id = archived.id.clone();
    let mut archive_storage = Storage::new(archive_path.clone()).expect("Failed to create archive");
    archive_storage
        .save(&archived)
        .expect("Failed to save archived pearl");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let mut active = create_collision_pearl("Archived title", "author", 1704067200);
    storage
        .create_new(&mut active, Some(&archive_path), 2)
        .expect("Failed to create active pearl");

    assert_ne!(
        active.id, archived_id,
        "create should not reuse archived IDs"
    );
    assert_eq!(
        active.id,
        generate_id("Archived title", "author", 1704067200, 1)
    );
}

#[test]
fn test_create_new_fails_explicitly_when_candidates_exhausted() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    let mut first = create_collision_pearl("Exhaust title", "author", 1704067200);
    let mut second = create_collision_pearl("Exhaust title", "author", 1704067200);
    storage
        .create_new(&mut first, Some(&archive_path), 1)
        .expect("Failed to create first pearl");

    let err = storage
        .create_new(&mut second, Some(&archive_path), 1)
        .expect_err("Exhausted candidate generation should fail");

    assert!(
        matches!(err, Error::InvalidPearl(ref message) if message.contains("candidate space exhausted")),
        "unexpected error: {err}"
    );

    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(
        pearls.len(),
        1,
        "failed create must not overwrite existing pearl"
    );
}

#[test]
fn test_create_new_waits_for_repository_lock() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");
    let lock_storage = Storage::new(storage_path.clone()).expect("Failed to create lock storage");
    let (locked_tx, locked_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        lock_storage
            .with_repository_lock(|| {
                locked_tx
                    .send(())
                    .expect("Failed to signal lock acquisition");
                thread::sleep(Duration::from_millis(250));
                Ok::<(), Error>(())
            })
            .expect("Failed to hold repository lock");
    });

    locked_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Timed out waiting for repository lock acquisition");

    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let mut pearl = create_collision_pearl("Repository lock", "author", 1704067200);
    let started = Instant::now();
    storage
        .create_new(&mut pearl, Some(&archive_path), 2)
        .expect("Failed to create pearl");

    assert!(
        started.elapsed() >= Duration::from_millis(200),
        "create_new should wait for repository-level multi-file operations"
    );
    handle.join().expect("Repository lock thread panicked");
}

#[test]
fn test_create_new_reserves_id_archived_while_waiting_for_repository_lock() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");
    let lock_storage = Storage::new(storage_path.clone()).expect("Failed to create lock storage");
    let archive_path_for_thread = archive_path.clone();
    let (locked_tx, locked_rx) = mpsc::channel();
    let (archive_tx, archive_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        lock_storage
            .with_repository_lock(|| {
                locked_tx
                    .send(())
                    .expect("Failed to signal lock acquisition");
                archive_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("Timed out waiting for archive signal");
                let archived = create_collision_pearl("Race title", "author", 1704067200);
                let mut archive_storage = Storage::new(archive_path_for_thread)
                    .expect("Failed to create archive storage");
                archive_storage
                    .save(&archived)
                    .expect("Failed to archive collision pearl");
                Ok::<(), Error>(())
            })
            .expect("Failed to hold repository lock");
    });

    locked_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Timed out waiting for repository lock acquisition");

    let create_thread = thread::spawn({
        let storage_path = storage_path.clone();
        let archive_path = archive_path.clone();
        move || {
            let mut storage = Storage::new(storage_path).expect("Failed to create storage");
            let mut active = create_collision_pearl("Race title", "author", 1704067200);
            storage
                .create_new(&mut active, Some(&archive_path), 2)
                .expect("Failed to create active pearl");
            active.id
        }
    });

    thread::sleep(Duration::from_millis(100));
    archive_tx.send(()).expect("Failed to signal archive write");
    let active_id = create_thread.join().expect("Create thread panicked");
    handle.join().expect("Repository lock thread panicked");

    assert_eq!(
        active_id,
        generate_id("Race title", "author", 1704067200, 1)
    );
}

#[test]
fn test_create_new_rejects_call_inside_storage_file_lock() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let mut pearl = create_collision_pearl("Nested lock", "author", 1704067200);

    let err = storage
        .with_lock(|storage| storage.create_new(&mut pearl, Some(&archive_path), 2))
        .expect_err("create_new inside with_lock should fail to avoid lock-order inversion");

    assert!(
        matches!(err, Error::InvalidPearl(ref message) if message.contains("storage file lock is already held")),
        "unexpected error: {err}"
    );
}

#[test]
fn test_save_fails_closed_on_malformed_active_jsonl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    fs::write(&storage_path, "{not valid json}\n").expect("Failed to write malformed issues");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let pearl = create_test_pearl("prl-111111", "Replacement");

    let err = storage
        .save(&pearl)
        .expect_err("save should fail closed on malformed active JSONL");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[test]
fn test_delete_fails_closed_on_malformed_active_jsonl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let original = "{not valid json}\n";
    fs::write(&storage_path, original).expect("Failed to write malformed issues");
    let mut storage = Storage::new(storage_path.clone()).expect("Failed to create storage");

    let err = storage
        .delete("prl-111111")
        .expect_err("delete should fail closed on malformed active JSONL");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string(storage_path).expect("Failed to reread issues"),
        original
    );
}

#[test]
fn test_load_all_strict_rejects_oversized_line() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let oversized = "{".to_string() + &"x".repeat(17 * 1024 * 1024);
    fs::write(&storage_path, oversized).expect("Failed to write oversized line");
    let storage = Storage::new(storage_path).expect("Failed to create storage");

    let err = storage
        .load_all_strict()
        .expect_err("strict load should reject oversized JSONL line");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[test]
fn test_load_all_rejects_oversized_line() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let oversized = "{".to_string() + &"x".repeat(17 * 1024 * 1024);
    fs::write(&storage_path, oversized).expect("Failed to write oversized line");
    let storage = Storage::new(storage_path).expect("Failed to create storage");

    let err = storage
        .load_all()
        .expect_err("permissive load should still reject oversized JSONL line");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[test]
fn test_load_by_id_rejects_oversized_line() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let oversized = "{".to_string() + &"x".repeat(17 * 1024 * 1024);
    fs::write(&storage_path, oversized).expect("Failed to write oversized line");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    let err = storage
        .load_by_id("prl-111111")
        .expect_err("load_by_id should reject oversized JSONL line");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_storage_rejects_symlink_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let target_path = temp_dir.path().join("target.jsonl");
    let symlink_path = temp_dir.path().join("issues.jsonl");
    fs::write(&target_path, "").expect("Failed to write target");
    std::os::unix::fs::symlink(&target_path, &symlink_path).expect("Failed to create symlink");

    let err = match Storage::new(symlink_path) {
        Ok(_) => panic!("Storage should reject symlinked JSONL path"),
        Err(err) => err,
    };

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidInput),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_storage_write_rejects_path_that_becomes_symlink() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let target_path = temp_dir.path().join("target.jsonl");
    let mut storage = Storage::new(storage_path.clone()).expect("Failed to create storage");
    fs::write(&target_path, "").expect("Failed to write target");
    std::os::unix::fs::symlink(&target_path, &storage_path).expect("Failed to create symlink");

    let pearl = create_test_pearl("prl-111111", "Symlink write");
    let err = storage
        .save(&pearl)
        .expect_err("Storage write should reject symlinked JSONL path");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidInput),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_storage_with_index_rejects_symlink_index_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let index_target = temp_dir.path().join("target.idx");
    let index_path = temp_dir.path().join("issues.idx");
    fs::write(&storage_path, "").expect("Failed to write storage");
    fs::write(&index_target, "").expect("Failed to write index target");
    std::os::unix::fs::symlink(&index_target, &index_path).expect("Failed to create symlink");

    let err = match Storage::with_index(storage_path, Some(index_path)) {
        Ok(_) => panic!("Storage should reject symlinked index path"),
        Err(err) => err,
    };

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidInput),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_enable_index_rejects_symlink_index_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let index_target = temp_dir.path().join("target.idx");
    let index_path = temp_dir.path().join("issues.idx");
    fs::write(&storage_path, "").expect("Failed to write storage");
    fs::write(&index_target, "").expect("Failed to write index target");
    std::os::unix::fs::symlink(&index_target, &index_path).expect("Failed to create symlink");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    let err = storage
        .enable_index(index_path)
        .expect_err("enable_index should reject symlinked index path");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidInput),
        "unexpected error: {err}"
    );
}

#[test]
fn test_index_load_rejects_huge_entry_count() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let index_path = temp_dir.path().join("issues.idx");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"PRLIDX1\0");
    bytes.push(1);
    bytes.extend_from_slice(&u64::MAX.to_le_bytes());
    fs::write(&index_path, bytes).expect("Failed to write malformed index");

    let err = Index::load(index_path).expect_err("Index should reject huge entry count");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[test]
fn test_index_load_rejects_huge_id_length() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let index_path = temp_dir.path().join("issues.idx");
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"PRLIDX1\0");
    bytes.push(1);
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    fs::write(&index_path, bytes).expect("Failed to write malformed index");

    let err = Index::load(index_path).expect_err("Index should reject huge ID length");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[test]
fn test_index_load_treats_truncated_file_as_invalid_data() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let index_path = temp_dir.path().join("issues.idx");
    fs::write(&index_path, b"PRLIDX1\0").expect("Failed to write truncated index");

    let err = Index::load(index_path).expect_err("Index should reject truncated file");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_storage_write_rejects_symlink_lock_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let lock_target = temp_dir.path().join("target.lock");
    let lock_path = storage_path.with_extension("lock");
    fs::write(&lock_target, "").expect("Failed to write lock target");
    std::os::unix::fs::symlink(&lock_target, &lock_path).expect("Failed to create lock symlink");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let pearl = create_test_pearl("prl-111111", "Symlink lock");

    let err = storage
        .save(&pearl)
        .expect_err("Storage write should reject symlinked lock path");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidInput),
        "unexpected error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn test_repository_lock_rejects_symlink_lock_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let repo_lock_target = temp_dir.path().join("target-repository.lock");
    let repo_lock_path = temp_dir.path().join("repository.lock");
    fs::write(&repo_lock_target, "").expect("Failed to write lock target");
    std::os::unix::fs::symlink(&repo_lock_target, &repo_lock_path)
        .expect("Failed to create repository lock symlink");
    let storage = Storage::new(storage_path).expect("Failed to create storage");

    let err = storage
        .with_repository_lock(|| Ok::<(), Error>(()))
        .expect_err("Repository lock should reject symlinked lock path");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidInput),
        "unexpected error: {err}"
    );
}

#[test]
fn test_load_all_strict_rejects_duplicate_ids() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let first = create_test_pearl("prl-111111", "First");
    let second = create_test_pearl("prl-111111", "Second");
    fs::write(
        &storage_path,
        format!(
            "{}\n{}\n",
            serde_json::to_string(&first).expect("serialize first"),
            serde_json::to_string(&second).expect("serialize second")
        ),
    )
    .expect("Failed to write duplicate IDs");
    let storage = Storage::new(storage_path).expect("Failed to create storage");

    let err = storage
        .load_all_strict()
        .expect_err("strict load should reject duplicate IDs");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[test]
fn test_repository_lock_rejects_call_inside_storage_file_lock() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");

    let err = storage
        .with_lock(|storage| storage.with_repository_lock(|| Ok::<(), Error>(())))
        .expect_err("repository lock inside with_lock should fail to avoid lock-order inversion");

    assert!(
        matches!(err, Error::InvalidPearl(ref message) if message.contains("repository lock cannot be acquired")),
        "unexpected error: {err}"
    );
}

#[test]
fn test_create_new_fails_closed_on_malformed_active_jsonl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");
    fs::write(&storage_path, "{not valid json}\n").expect("Failed to write malformed issues");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let mut pearl = create_collision_pearl("Malformed active", "author", 1704067200);

    let err = storage
        .create_new(&mut pearl, Some(&archive_path), 2)
        .expect_err("create_new should fail closed on malformed active JSONL");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
}

#[test]
fn test_create_new_fails_closed_on_malformed_archive_jsonl() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let storage_path = temp_dir.path().join("issues.jsonl");
    let archive_path = temp_dir.path().join("archive.jsonl");
    fs::write(&archive_path, "{not valid json}\n").expect("Failed to write malformed archive");
    let mut storage = Storage::new(storage_path).expect("Failed to create storage");
    let mut pearl = create_collision_pearl("Malformed archive", "author", 1704067200);

    let err = storage
        .create_new(&mut pearl, Some(&archive_path), 2)
        .expect_err("create_new should fail closed on malformed archive JSONL");

    assert!(
        matches!(err, Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::InvalidData),
        "unexpected error: {err}"
    );
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
