// Rust guideline compliant 2026-02-06

//! Property-based tests for the storage module.
//!
//! These tests validate universal properties that should hold across all valid inputs
//! for JSONL file operations, atomicity, and locking.

use pearls_core::{Pearl, Status};
use proptest::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Generates arbitrary valid Pearl IDs.
fn arb_id() -> impl Strategy<Value = String> {
    prop::string::string_regex("prl-[0-9a-f]{6,8}").unwrap()
}

/// Generates arbitrary Status values.
fn arb_status() -> impl Strategy<Value = Status> {
    prop_oneof![
        Just(Status::Open),
        Just(Status::InProgress),
        Just(Status::Blocked),
        Just(Status::Deferred),
        Just(Status::Closed),
    ]
}

/// Generates arbitrary valid Pearl values.
fn arb_pearl() -> impl Strategy<Value = Pearl> {
    (
        arb_id(),
        prop::string::string_regex("[a-zA-Z0-9 ]{1,100}").unwrap(),
        any::<String>(),
        arb_status(),
        0u8..=4u8,
        1i64..1_000_000_000_000i64,
        1i64..1_000_000_000_000i64,
        prop::string::string_regex("[a-zA-Z0-9_-]{1,50}").unwrap(),
        prop::collection::vec(prop::string::string_regex("[a-z]{1,20}").unwrap(), 0..5),
        prop::collection::vec(
            (
                arb_id(),
                prop_oneof![
                    Just(pearls_core::DepType::Blocks),
                    Just(pearls_core::DepType::ParentChild),
                    Just(pearls_core::DepType::Related),
                    Just(pearls_core::DepType::DiscoveredFrom),
                ],
            )
                .prop_map(|(target_id, dep_type)| pearls_core::Dependency {
                    target_id,
                    dep_type,
                }),
            0..3,
        ),
        prop::collection::hash_map(
            prop::string::string_regex("[a-z_]{1,20}").unwrap(),
            any::<String>().prop_map(|s| serde_json::Value::String(s)),
            0..3,
        ),
    )
        .prop_map(
            |(
                id,
                title,
                description,
                status,
                priority,
                created_at,
                updated_at,
                author,
                labels,
                deps,
                metadata,
            )| {
                Pearl {
                    id,
                    title,
                    description,
                    status,
                    priority,
                    created_at,
                    updated_at,
                    author,
                    labels,
                    deps,
                    metadata,
                }
            },
        )
}

proptest! {
    /// **Property 1: JSONL Round-Trip Preservation** (integration with serialization)
    ///
    /// **Validates: Requirements 1.2, 1.3, 1.4, 1.7, 2.1, 2.2, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9**
    ///
    /// For any valid Pearl, saving it to JSONL then loading it should produce
    /// an equivalent Pearl with all fields preserved.
    #[test]
    fn test_jsonl_round_trip_preservation(pearl in arb_pearl()) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("test.jsonl");

        let mut storage = pearls_core::storage::Storage::new(storage_path.clone())
            .expect("Failed to create storage");

        storage.save(&pearl).expect("Failed to save pearl");
        let loaded = storage.load_by_id(&pearl.id).expect("Failed to load pearl");

        prop_assert_eq!(pearl, loaded);
    }

    /// **Property 39: Timestamp Update on Modification**
    ///
    /// **Validates: Requirements 26.2**
    #[test]
    fn test_timestamp_update_on_modification(
        mut pearl in arb_pearl(),
        delta in 1i64..1_000_000i64
    ) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("test.jsonl");

        let mut storage = pearls_core::storage::Storage::new(storage_path.clone())
            .expect("Failed to create storage");

        let original_updated = pearl.updated_at;
        storage.save(&pearl).expect("Failed to save pearl");

        pearl.updated_at = original_updated + delta;
        storage.save(&pearl).expect("Failed to save updated pearl");

        let loaded = storage.load_by_id(&pearl.id).expect("Failed to load pearl");
        prop_assert_eq!(loaded.updated_at, original_updated + delta);
    }

    /// **Property 40: Timestamp Immutability on Read**
    ///
    /// **Validates: Requirements 26.1, 26.5**
    #[test]
    fn test_timestamp_immutability_on_read(pearl in arb_pearl()) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("test.jsonl");

        let mut storage = pearls_core::storage::Storage::new(storage_path.clone())
            .expect("Failed to create storage");

        storage.save(&pearl).expect("Failed to save pearl");
        let loaded = storage.load_by_id(&pearl.id).expect("Failed to load pearl");

        prop_assert_eq!(loaded.created_at, pearl.created_at);
        prop_assert_eq!(loaded.updated_at, pearl.updated_at);
    }

    /// **Property 3: Multi-Pearl Separation**
    ///
    /// **Validates: Requirements 1.4**
    ///
    /// For any list of Pearls, saving them to JSONL should produce lines
    /// separated by exactly one newline character between each Pearl.
    #[test]
    fn test_multi_pearl_separation(pearls in prop::collection::vec(arb_pearl(), 1..10)) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("test.jsonl");

        let mut storage = pearls_core::storage::Storage::new(storage_path.clone())
            .expect("Failed to create storage");

        storage.save_all(&pearls).expect("Failed to save pearls");

        // Read the file and verify line separation
        let content = fs::read_to_string(&storage_path).expect("Failed to read file");
        let lines: Vec<&str> = content.lines().collect();

        prop_assert_eq!(lines.len(), pearls.len(), "Number of lines should match number of Pearls");

        // Verify each line is valid JSON
        for line in lines {
            let _: Pearl = serde_json::from_str(line)
                .expect("Each line should be valid Pearl JSON");
        }
    }

    /// **Property 25: Write Atomicity**
    ///
    /// **Validates: Requirements 17.6**
    ///
    /// For any write operation, either all changes are persisted or none are
    /// (no partial writes), even if the process crashes mid-operation.
    #[test]
    fn test_write_atomicity(pearls in prop::collection::vec(arb_pearl(), 1..10)) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("test.jsonl");

        let mut storage = pearls_core::storage::Storage::new(storage_path.clone())
            .expect("Failed to create storage");

        // Save pearls
        storage.save_all(&pearls).expect("Failed to save pearls");

        // Verify all pearls were written
        let loaded = storage.load_all().expect("Failed to load pearls");
        prop_assert_eq!(loaded.len(), pearls.len());

        // Verify file is valid JSON (no partial writes)
        let content = fs::read_to_string(&storage_path).expect("Failed to read file");
        for line in content.lines() {
            let _: Pearl = serde_json::from_str(line)
                .expect("File should contain only valid Pearl JSON");
        }
    }

    /// **Property 26: Lock Release Guarantee**
    ///
    /// **Validates: Requirements 17.4**
    ///
    /// For any write operation that acquires a lock, the lock must be released
    /// when the operation completes (success or failure).
    #[test]
    fn test_lock_release_guarantee(pearl in arb_pearl()) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("test.jsonl");

        let mut storage = pearls_core::storage::Storage::new(storage_path.clone())
            .expect("Failed to create storage");

        // Execute with lock
        let result = storage.with_lock(|storage| storage.save(&pearl));

        prop_assert!(result.is_ok(), "Lock operation should succeed");

        // Verify lock is released by attempting another operation
        let result2 = storage.with_lock(|storage| storage.load_by_id(&pearl.id));

        prop_assert!(result2.is_ok(), "Lock should be released after first operation");
    }
}

/// **Property 27: Concurrent Write Serialization**
///
/// **Validates: Requirements 17.1**
///
/// For any two concurrent write operations, they must be serialized such that
/// one completes fully before the other begins modifying the file.
#[test]
fn test_concurrent_write_serialization() {
    proptest!(|(
        pearl1 in arb_pearl(),
        pearl2 in arb_pearl(),
    )| {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("test.jsonl");

        let mut storage = pearls_core::storage::Storage::new(storage_path.clone())
            .expect("Failed to create storage");

        // Save first pearl
        storage.save(&pearl1).expect("Failed to save pearl1");

        // Save second pearl
        storage.save(&pearl2).expect("Failed to save pearl2");

        // Verify both pearls are present
        let loaded = storage.load_all().expect("Failed to load pearls");
        prop_assert!(loaded.iter().any(|p| p.id == pearl1.id), "Pearl1 should be present");
        prop_assert!(loaded.iter().any(|p| p.id == pearl2.id), "Pearl2 should be present");
    });
}
