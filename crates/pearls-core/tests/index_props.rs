// Rust guideline compliant 2026-02-06

//! Property-based tests for the index module.
//!
//! These tests validate index consistency and lookup correctness
//! across arbitrary valid input sets.

use pearls_core::{Pearl, Status, Storage};
use proptest::prelude::*;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use tempfile::TempDir;

fn arb_id_set() -> impl Strategy<Value = Vec<String>> {
    prop::collection::btree_set(
        prop::string::string_regex("prl-[0-9a-f]{6,8}").unwrap(),
        1..10,
    )
    .prop_map(|set| set.into_iter().collect())
}

fn pearls_from_ids(ids: Vec<String>) -> Vec<Pearl> {
    ids.into_iter()
        .enumerate()
        .map(|(i, id)| Pearl {
            id,
            title: format!("Pearl {}", i),
            description: String::new(),
            status: Status::Open,
            priority: 2,
            created_at: 1000 + i as i64,
            updated_at: 1000 + i as i64,
            author: "test-author".to_string(),
            labels: Vec::new(),
            deps: Vec::new(),
            metadata: Default::default(),
            comments: Vec::new(),
        })
        .collect()
}

fn read_pearl_at_offset(path: &std::path::Path, offset: u64) -> Pearl {
    let mut file = File::open(path).expect("Failed to open JSONL file");
    file.seek(SeekFrom::Start(offset))
        .expect("Failed to seek to offset");

    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).expect("Failed to read line");
    let line_trimmed = line.trim_end_matches(['\n', '\r']);
    serde_json::from_str(line_trimmed).expect("Failed to parse Pearl JSON")
}

proptest! {
    /// **Property 31: Index Consistency**
    ///
    /// **Validates: Requirements 10.7, 30.2, 30.4**
    #[test]
    fn test_index_consistency(ids in arb_id_set()) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("issues.jsonl");
        let index_path = temp_dir.path().join("index.bin");

        let mut storage = Storage::with_index(storage_path.clone(), Some(index_path))
            .expect("Failed to create storage with index");

        let pearls = pearls_from_ids(ids);
        storage.save_all(&pearls).expect("Failed to save pearls");

        let index = storage.index().expect("Index should be enabled");
        prop_assert_eq!(index.len(), pearls.len(), "Index should contain all Pearls");

        for (id, offset) in index.entries() {
            let pearl = read_pearl_at_offset(&storage_path, *offset);
            prop_assert_eq!(&pearl.id, id, "Index offset should map to correct Pearl");
        }
    }

    /// **Property 32: Index Lookup Correctness**
    ///
    /// **Validates: Requirements 30.2**
    #[test]
    fn test_index_lookup_correctness(ids in arb_id_set()) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("issues.jsonl");
        let index_path = temp_dir.path().join("index.bin");

        let mut storage = Storage::with_index(storage_path.clone(), Some(index_path))
            .expect("Failed to create storage with index");

        let pearls = pearls_from_ids(ids);
        storage.save_all(&pearls).expect("Failed to save pearls");

        let index = storage.index().expect("Index should be enabled");

        for pearl in &pearls {
            let offset = index.get(&pearl.id).expect("Index should contain Pearl ID");
            let loaded = read_pearl_at_offset(&storage_path, offset);
            prop_assert_eq!(
                &loaded.id,
                &pearl.id,
                "Index lookup should resolve correct Pearl"
            );
        }
    }
}
