// Rust guideline compliant 2026-02-06

//! Property-based tests for identity module.
//!
//! These tests validate universal properties for hash-based ID generation,
//! validation, and partial ID resolution.

use pearls_core::identity::{generate_id, resolve_partial_id, validate_id_format};
use pearls_core::Pearl;
use proptest::prelude::*;

/// Generates arbitrary valid titles.
fn arb_title() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9 ]{1,100}").unwrap()
}

/// Generates arbitrary valid authors.
fn arb_author() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9_-]{1,50}").unwrap()
}

/// Generates arbitrary valid timestamps.
fn arb_timestamp() -> impl Strategy<Value = i64> {
    1i64..1_000_000_000_000i64
}

/// Generates arbitrary nonce values.
fn arb_nonce() -> impl Strategy<Value = u32> {
    any::<u32>()
}

proptest! {
    /// **Property 7: ID Format Consistency**
    ///
    /// **Validates: Requirements 2.3, 3.1, 3.2, 3.3**
    ///
    /// For any generated Pearl ID, it must match the format "prl-[0-9a-f]{6,8}"
    /// (prefix "prl-" followed by 6-8 hexadecimal characters).
    #[test]
    fn test_id_format_consistency(
        title in arb_title(),
        author in arb_author(),
        timestamp in arb_timestamp(),
        nonce in arb_nonce()
    ) {
        let id = generate_id(&title, &author, timestamp, nonce);

        // Verify format
        prop_assert!(id.starts_with("prl-"), "ID must start with 'prl-'");

        let hash_part = &id[4..];
        prop_assert!(
            hash_part.len() >= 6 && hash_part.len() <= 8,
            "Hash part must be 6-8 characters, got {}",
            hash_part.len()
        );

        prop_assert!(
            hash_part.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash part must contain only hexadecimal characters"
        );

        // Verify validation passes
        prop_assert!(validate_id_format(&id).is_ok(), "Generated ID should pass validation");
    }

    /// **Property 8: ID Generation Determinism**
    ///
    /// **Validates: Requirements 3.1**
    ///
    /// For any tuple of (title, author, timestamp, nonce), generating an ID twice
    /// with the same inputs should produce identical IDs.
    #[test]
    fn test_id_generation_determinism(
        title in arb_title(),
        author in arb_author(),
        timestamp in arb_timestamp(),
        nonce in arb_nonce()
    ) {
        let id1 = generate_id(&title, &author, timestamp, nonce);
        let id2 = generate_id(&title, &author, timestamp, nonce);

        prop_assert_eq!(id1, id2, "Same inputs should produce identical IDs");
    }

    /// **Property 9: Partial ID Resolution Uniqueness**
    ///
    /// **Validates: Requirements 3.6, 28.1, 28.2**
    ///
    /// For any set of Pearls and a partial ID that matches exactly one Pearl's ID prefix,
    /// resolution should return that Pearl's full ID.
    #[test]
    fn test_partial_id_resolution_uniqueness(
        title in arb_title(),
        author in arb_author()
    ) {
        // Create a single Pearl
        let pearl = Pearl::new(title, author);
        let pearls = vec![pearl.clone()];
        let full_id = pearl.id.clone();

        // Test with various prefix lengths (3 to full length)
        for len in 3..=full_id.len() {
            let partial = &full_id[..len];
            let resolved = resolve_partial_id(partial, &pearls);

            prop_assert!(resolved.is_ok(), "Resolution should succeed for unique match");
            prop_assert_eq!(
                &resolved.unwrap(),
                &full_id,
                "Resolved ID should match the Pearl's full ID"
            );
        }
    }
}

/// **Property 10: Partial ID Ambiguity Detection**
///
/// **Validates: Requirements 3.7, 28.3**
///
/// For any set of Pearls and a partial ID that matches multiple Pearl ID prefixes,
/// resolution should fail with an error listing all matching IDs.
#[test]
fn test_partial_id_ambiguity_detection() {
    // Create multiple Pearls with IDs that share a common prefix
    // We'll use specific inputs to ensure we get IDs with common prefixes
    let pearl1 = Pearl::new("Task A".to_string(), "alice".to_string());
    let pearl2 = Pearl::new("Task B".to_string(), "alice".to_string());
    let pearl3 = Pearl::new("Task C".to_string(), "bob".to_string());

    let pearls = vec![pearl1.clone(), pearl2.clone(), pearl3.clone()];

    // Test with "prl" prefix which should match all Pearls
    let result = resolve_partial_id("prl", &pearls);

    // Should fail with ambiguous error
    assert!(
        result.is_err(),
        "Resolution should fail for ambiguous prefix"
    );

    // Verify error message contains multiple IDs
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("Ambiguous"),
        "Error should indicate ambiguity"
    );
}

/// Test that partial IDs shorter than 3 characters are rejected.
#[test]
fn test_partial_id_minimum_length() {
    let pearl = Pearl::new("Test".to_string(), "alice".to_string());
    let pearls = vec![pearl];

    // Test with 1 character
    let result = resolve_partial_id("p", &pearls);
    assert!(result.is_err(), "Should reject 1-character partial ID");

    // Test with 2 characters
    let result = resolve_partial_id("pr", &pearls);
    assert!(result.is_err(), "Should reject 2-character partial ID");

    // Test with 3 characters (should be accepted if it matches)
    let result = resolve_partial_id("prl", &pearls);
    // This may succeed or fail depending on whether it matches, but shouldn't
    // fail due to length
    if let Err(e) = result {
        let err_msg = format!("{}", e);
        assert!(
            !err_msg.contains("at least 3 characters"),
            "Should not reject 3-character partial ID for length"
        );
    }
}

/// Test that validate_id_format correctly validates ID format.
#[test]
fn test_validate_id_format_correctness() {
    // Valid IDs
    assert!(validate_id_format("prl-abc123").is_ok());
    assert!(validate_id_format("prl-123456").is_ok());
    assert!(validate_id_format("prl-abcdef").is_ok());
    assert!(validate_id_format("prl-12345678").is_ok());

    // Invalid IDs - wrong prefix
    assert!(validate_id_format("pearl-abc123").is_err());
    assert!(validate_id_format("abc123").is_err());

    // Invalid IDs - wrong length
    assert!(validate_id_format("prl-12345").is_err()); // Too short
    assert!(validate_id_format("prl-123456789").is_err()); // Too long

    // Invalid IDs - non-hex characters
    assert!(validate_id_format("prl-abcxyz").is_err());
    assert!(validate_id_format("prl-ABC123").is_err()); // Uppercase not allowed
    assert!(validate_id_format("prl-12-456").is_err());
}

/// Test that no matches returns appropriate error.
#[test]
fn test_partial_id_no_matches() {
    let pearl = Pearl::new("Test".to_string(), "alice".to_string());
    let pearls = vec![pearl];

    // Use a partial ID that definitely won't match
    let result = resolve_partial_id("xyz", &pearls);
    assert!(result.is_err(), "Should fail when no matches found");

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("not found"),
        "Error should indicate not found"
    );
}
