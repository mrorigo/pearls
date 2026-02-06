// Rust guideline compliant 2026-02-06

//! Unit tests for error types and messages.
//!
//! These tests validate error formatting, context preservation, and agent-friendly messages.

use pearls_core::Error;

#[test]
fn test_io_error_formatting() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let error = Error::Io(io_err);
    let msg = error.to_string();
    assert!(
        msg.contains("IO error"),
        "IO error should contain 'IO error' prefix"
    );
}

#[test]
fn test_json_error_formatting() {
    let json_err = serde_json::from_str::<serde_json::Value>("invalid json")
        .expect_err("Should fail to parse invalid JSON");
    let error = Error::Json(json_err);
    let msg = error.to_string();
    assert!(
        msg.contains("JSON error"),
        "JSON error should contain 'JSON error' prefix"
    );
}

#[test]
fn test_invalid_pearl_error_formatting() {
    let error = Error::InvalidPearl("Title cannot be empty".to_string());
    let msg = error.to_string();
    assert_eq!(msg, "Invalid Pearl: Title cannot be empty");
    assert!(
        msg.contains("Title cannot be empty"),
        "Should preserve context message"
    );
}

#[test]
fn test_not_found_error_formatting() {
    let error = Error::NotFound("prl-a1b2c3".to_string());
    let msg = error.to_string();
    assert_eq!(msg, "Pearl not found: prl-a1b2c3");
    assert!(
        msg.contains("prl-a1b2c3"),
        "Should include Pearl ID in message"
    );
}

#[test]
fn test_cycle_detected_error_formatting() {
    let cycle = vec![
        "prl-a1b2c3".to_string(),
        "prl-d4e5f6".to_string(),
        "prl-a1b2c3".to_string(),
    ];
    let error = Error::CycleDetected(cycle.clone());
    let msg = error.to_string();
    assert!(
        msg.contains("Cycle detected"),
        "Should contain 'Cycle detected' prefix"
    );
    assert!(msg.contains("prl-a1b2c3"), "Should include cycle nodes");
    assert!(msg.contains("prl-d4e5f6"), "Should include all cycle nodes");
}

#[test]
fn test_invalid_transition_error_formatting() {
    let error = Error::InvalidTransition(
        "Cannot transition from 'open' to 'closed'. Pearl is blocked by: prl-d4e5f6".to_string(),
    );
    let msg = error.to_string();
    assert_eq!(
        msg,
        "Invalid state transition: Cannot transition from 'open' to 'closed'. Pearl is blocked by: prl-d4e5f6"
    );
    assert!(
        msg.contains("Cannot transition"),
        "Should explain why transition is invalid"
    );
    assert!(
        msg.contains("blocked"),
        "Should mention blocking constraint"
    );
}

#[test]
fn test_ambiguous_id_error_formatting() {
    let matches = vec!["prl-a1b2c3".to_string(), "prl-a1d4e5".to_string()];
    let error = Error::AmbiguousId("a1".to_string(), matches.clone());
    let msg = error.to_string();
    assert!(
        msg.contains("Ambiguous ID"),
        "Should contain 'Ambiguous ID' prefix"
    );
    assert!(msg.contains("a1"), "Should include partial ID");
    assert!(msg.contains("prl-a1b2c3"), "Should list all matching IDs");
    assert!(msg.contains("prl-a1d4e5"), "Should list all matching IDs");
}

#[test]
fn test_git_error_formatting() {
    let error = Error::Git("Failed to push to remote".to_string());
    let msg = error.to_string();
    assert_eq!(msg, "Git error: Failed to push to remote");
    assert!(
        msg.contains("Failed to push"),
        "Should preserve Git error context"
    );
}

#[test]
fn test_error_debug_formatting() {
    let error = Error::NotFound("prl-test".to_string());
    let debug_msg = format!("{:?}", error);
    assert!(
        debug_msg.contains("NotFound"),
        "Debug format should show variant name"
    );
    assert!(
        debug_msg.contains("prl-test"),
        "Debug format should show context"
    );
}

#[test]
fn test_error_context_preservation_in_invalid_pearl() {
    let context = "Priority must be between 0 and 4, got 5";
    let error = Error::InvalidPearl(context.to_string());
    let msg = error.to_string();
    assert!(
        msg.contains(context),
        "Error should preserve full context message"
    );
}

#[test]
fn test_error_context_preservation_in_cycle() {
    let cycle = vec![
        "prl-a1b2c3".to_string(),
        "prl-d4e5f6".to_string(),
        "prl-g7h8i9".to_string(),
        "prl-a1b2c3".to_string(),
    ];
    let error = Error::CycleDetected(cycle.clone());
    let msg = error.to_string();
    for id in &cycle {
        assert!(
            msg.contains(id),
            "Cycle message should contain all node IDs"
        );
    }
}

#[test]
fn test_error_context_preservation_in_ambiguous_id() {
    let partial = "abc";
    let matches = vec![
        "prl-abc123".to_string(),
        "prl-abc456".to_string(),
        "prl-abc789".to_string(),
    ];
    let error = Error::AmbiguousId(partial.to_string(), matches.clone());
    let msg = error.to_string();
    assert!(msg.contains(partial), "Should preserve partial ID");
    for id in &matches {
        assert!(msg.contains(id), "Should list all matching IDs");
    }
}

#[test]
fn test_agent_friendly_error_messages() {
    // Test that error messages are structured and parseable
    let error = Error::InvalidTransition("Cannot close: blocked by prl-xyz".to_string());
    let msg = error.to_string();

    // Should have clear structure: "Error type: Details"
    assert!(
        msg.contains(":"),
        "Error message should have structured format"
    );
    assert!(!msg.contains("\n"), "Error message should be single line");
    assert!(msg.len() < 500, "Error message should be concise");
}

#[test]
fn test_error_message_consistency() {
    // Test that same error produces same message
    let error1 = Error::NotFound("prl-test".to_string());
    let error2 = Error::NotFound("prl-test".to_string());
    assert_eq!(
        error1.to_string(),
        error2.to_string(),
        "Same error should produce same message"
    );
}

#[test]
fn test_error_message_distinguishability() {
    // Test that different errors produce different messages
    let error1 = Error::NotFound("prl-a1b2c3".to_string());
    let error2 = Error::InvalidPearl("Title cannot be empty".to_string());
    assert_ne!(
        error1.to_string(),
        error2.to_string(),
        "Different errors should produce different messages"
    );
}
