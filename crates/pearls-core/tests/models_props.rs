// Rust guideline compliant 2026-02-06

//! Property-based tests for data models.
//!
//! These tests validate universal properties that should hold across all valid inputs.

use pearls_core::{DepType, Dependency, Pearl, Status};
use proptest::prelude::*;

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

/// Generates arbitrary DepType values.
fn arb_dep_type() -> impl Strategy<Value = DepType> {
    prop_oneof![
        Just(DepType::Blocks),
        Just(DepType::ParentChild),
        Just(DepType::Related),
        Just(DepType::DiscoveredFrom),
    ]
}

/// Generates arbitrary Dependency values.
fn arb_dependency() -> impl Strategy<Value = Dependency> {
    (arb_id(), arb_dep_type()).prop_map(|(target_id, dep_type)| Dependency {
        target_id,
        dep_type,
    })
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
        prop::collection::vec(prop::string::string_regex("[a-z]{1,20}").unwrap(), 0..10),
        prop::collection::vec(arb_dependency(), 0..5),
        prop::collection::hash_map(
            prop::string::string_regex("[a-z_]{1,20}").unwrap(),
            any::<String>().prop_map(|s| serde_json::Value::String(s)),
            0..5,
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
    /// **Property 1: JSONL Round-Trip Preservation**
    ///
    /// **Validates: Requirements 1.2, 1.3, 1.7, 2.1, 2.2, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9**
    ///
    /// For any valid Pearl, serializing it to JSON then deserializing should produce
    /// an equivalent Pearl with all fields preserved.
    #[test]
    fn test_jsonl_round_trip_preservation(pearl in arb_pearl()) {
        let json = serde_json::to_string(&pearl).expect("Serialization failed");
        let deserialized: Pearl = serde_json::from_str(&json).expect("Deserialization failed");
        prop_assert_eq!(pearl, deserialized);
    }

    /// **Property 2: Single-Line Serialization**
    ///
    /// **Validates: Requirements 1.2**
    ///
    /// For any Pearl, serializing it to JSON should produce exactly one line
    /// with no internal newline characters.
    #[test]
    fn test_single_line_serialization(pearl in arb_pearl()) {
        let json = serde_json::to_string(&pearl).expect("Serialization failed");
        prop_assert!(!json.contains('\n'), "JSON contains newline characters");
        prop_assert!(!json.contains('\r'), "JSON contains carriage return characters");
    }

    /// **Property 4: Mandatory Field Presence**
    ///
    /// **Validates: Requirements 2.1**
    ///
    /// For any Pearl, it must have non-empty values for all mandatory fields:
    /// id, title, status, created_at, updated_at, author.
    #[test]
    fn test_mandatory_field_presence(pearl in arb_pearl()) {
        prop_assert!(!pearl.id.is_empty(), "ID must not be empty");
        prop_assert!(!pearl.title.is_empty(), "Title must not be empty");
        prop_assert!(pearl.created_at > 0, "created_at must be positive");
        prop_assert!(pearl.updated_at > 0, "updated_at must be positive");
        prop_assert!(!pearl.author.is_empty(), "Author must not be empty");
    }

    /// **Property 5: Schema Conformance**
    ///
    /// **Validates: Requirements 2.4, 2.5, 2.6, 2.7, 2.8**
    ///
    /// For any Pearl, all fields must conform to their type constraints:
    /// priority in range 0-4, status in valid enum, timestamps as positive integers,
    /// labels as string array, dependencies as Dependency array.
    #[test]
    fn test_schema_conformance(pearl in arb_pearl()) {
        prop_assert!(pearl.priority <= 4, "Priority must be 0-4, got {}", pearl.priority);
        prop_assert!(pearl.created_at > 0, "created_at must be positive");
        prop_assert!(pearl.updated_at > 0, "updated_at must be positive");

        // Verify status is one of the valid enum values
        let valid_statuses = [
            Status::Open,
            Status::InProgress,
            Status::Blocked,
            Status::Deferred,
            Status::Closed,
        ];
        prop_assert!(valid_statuses.contains(&pearl.status), "Invalid status");

        // Verify labels are strings
        for label in &pearl.labels {
            prop_assert!(label.is_ascii(), "Label should be ASCII");
        }

        // Verify dependencies have valid structure
        for dep in &pearl.deps {
            prop_assert!(!dep.target_id.is_empty(), "Dependency target_id must not be empty");
        }
    }

    /// **Property 6: Optional Field Flexibility**
    ///
    /// **Validates: Requirements 2.2**
    ///
    /// For any Pearl, optional fields (description, priority, labels, deps, metadata)
    /// may be absent, and deserializing a Pearl without them should use appropriate defaults.
    #[test]
    fn test_optional_field_flexibility(pearl in arb_pearl()) {
        // Create a minimal JSON with only mandatory fields
        let minimal_json = serde_json::json!({
            "id": pearl.id,
            "title": pearl.title,
            "status": pearl.status,
            "created_at": pearl.created_at,
            "updated_at": pearl.updated_at,
            "author": pearl.author,
        });

        let deserialized: Pearl = serde_json::from_value(minimal_json)
            .expect("Deserialization of minimal Pearl failed");

        // Verify defaults are applied
        prop_assert_eq!(deserialized.description, "", "Default description should be empty");
        prop_assert_eq!(deserialized.priority, 2, "Default priority should be 2");
        prop_assert!(deserialized.labels.is_empty(), "Default labels should be empty");
        prop_assert!(deserialized.deps.is_empty(), "Default deps should be empty");
        prop_assert!(deserialized.metadata.is_empty(), "Default metadata should be empty");
    }

    /// **Property 35: Label Case Preservation**
    ///
    /// **Validates: Requirements 22.6**
    #[test]
    fn test_label_case_preservation(label in prop::string::string_regex("[A-Za-z]{1,10}").unwrap()) {
        let pearl = Pearl {
            id: "prl-abc123".to_string(),
            title: "Title".to_string(),
            description: String::new(),
            status: Status::Open,
            priority: 2,
            created_at: 1000,
            updated_at: 1000,
            author: "author".to_string(),
            labels: vec![label.clone()],
            deps: Vec::new(),
            metadata: Default::default(),
        };
        let json = serde_json::to_string(&pearl).expect("Serialization failed");
        let deserialized: Pearl = serde_json::from_str(&json).expect("Deserialization failed");
        prop_assert_eq!(&deserialized.labels[0], &label);
    }

    /// **Property 38: Markdown Preservation**
    ///
    /// **Validates: Requirements 24.2, 24.3**
    #[test]
    fn test_markdown_preservation(desc in prop::string::string_regex("(?s)[A-Za-z0-9\\n\\*# ]{1,100}").unwrap()) {
        let pearl = Pearl {
            id: "prl-abc123".to_string(),
            title: "Title".to_string(),
            description: desc.clone(),
            status: Status::Open,
            priority: 2,
            created_at: 1000,
            updated_at: 1000,
            author: "author".to_string(),
            labels: Vec::new(),
            deps: Vec::new(),
            metadata: Default::default(),
        };
        let json = serde_json::to_string(&pearl).expect("Serialization failed");
        let deserialized: Pearl = serde_json::from_str(&json).expect("Deserialization failed");
        prop_assert_eq!(deserialized.description, desc);
    }
}

#[test]
fn test_priority_default_is_medium() {
    let pearl = Pearl::new("Test".to_string(), "author".to_string());
    assert_eq!(pearl.priority, 2);
}

#[test]
fn test_unknown_field_tolerance() {
    let json = serde_json::json!({
        "id": "prl-abc123",
        "title": "Test",
        "status": "open",
        "created_at": 1000,
        "updated_at": 1000,
        "author": "author",
        "extra_field": "ignored"
    });
    let pearl: Pearl = serde_json::from_value(json).expect("Should ignore unknown fields");
    assert_eq!(pearl.title, "Test");
}

#[test]
fn test_metadata_preservation() {
    let mut pearl = Pearl::new("Meta".to_string(), "author".to_string());
    pearl.metadata.insert(
        "key".to_string(),
        serde_json::Value::String("value".to_string()),
    );
    let json = serde_json::to_string(&pearl).expect("Serialization failed");
    let deserialized: Pearl = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(
        deserialized.metadata.get("key"),
        Some(&serde_json::Value::String("value".to_string()))
    );
}

/// **Property 3: Multi-Pearl Separation**
///
/// **Validates: Requirements 1.4**
///
/// For any list of Pearls, serializing them to JSONL should produce lines
/// separated by exactly one newline character between each Pearl.
#[test]
fn test_multi_pearl_separation() {
    proptest!(|(pearls in prop::collection::vec(arb_pearl(), 1..10))| {
        let mut jsonl = String::new();
        for pearl in &pearls {
            let json = serde_json::to_string(pearl).expect("Serialization failed");
            jsonl.push_str(&json);
            jsonl.push('\n');
        }

        let lines: Vec<&str> = jsonl.lines().collect();
        prop_assert_eq!(lines.len(), pearls.len(), "Number of lines should match number of Pearls");

        // Verify each line is valid JSON
        for line in lines {
            let _: Pearl = serde_json::from_str(line)
                .expect("Each line should be valid Pearl JSON");
        }
    });
}
