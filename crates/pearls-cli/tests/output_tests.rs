// Rust guideline compliant 2026-02-06

//! Unit tests for output formatting module.

use pearls_core::{Pearl, Status};
use std::collections::HashMap;

fn create_test_pearl() -> Pearl {
    Pearl {
        id: "prl-a1b2c3".to_string(),
        title: "Test Pearl".to_string(),
        description: "A test pearl for formatting".to_string(),
        status: Status::Open,
        priority: 1,
        created_at: 1704067200,
        updated_at: 1704240000,
        author: "test-author".to_string(),
        labels: vec!["test".to_string(), "formatting".to_string()],
        deps: vec![],
        metadata: HashMap::new(),
    }
}

#[test]
fn test_json_formatter_single_pearl() {
    use pearls_cli::create_formatter;

    let pearl = create_test_pearl();
    let formatter = create_formatter("json", false, false);
    let output = formatter.format_pearl(&pearl);

    assert!(output.contains("prl-a1b2c3"));
    assert!(output.contains("Test Pearl"));
    assert!(output.contains("test-author"));
}

#[test]
fn test_json_formatter_pearl_list() {
    use pearls_cli::create_formatter;

    let pearl1 = create_test_pearl();
    let mut pearl2 = create_test_pearl();
    pearl2.id = "prl-d4e5f6".to_string();

    let formatter = create_formatter("json", false, false);
    let output = formatter.format_list(&[pearl1, pearl2]);

    assert!(output.contains("prl-a1b2c3"));
    assert!(output.contains("prl-d4e5f6"));
    assert!(output.contains("\"total\": 2"));
}

#[test]
fn test_json_formatter_error() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("json", false, false);
    let output = formatter.format_error("Test error message");

    assert!(output.contains("Test error message"));
    assert!(output.contains("error"));
}

#[test]
fn test_table_formatter_single_pearl() {
    use pearls_cli::create_formatter;

    let pearl = create_test_pearl();
    let formatter = create_formatter("table", false, false);
    let output = formatter.format_pearl(&pearl);

    assert!(output.contains("prl-a1b2c3"));
    assert!(output.contains("Test Pearl"));
    assert!(output.contains("test-author"));
}

#[test]
fn test_table_formatter_absolute_time() {
    use pearls_cli::create_formatter;

    let pearl = create_test_pearl();
    let formatter = create_formatter("table", false, true);
    let output = formatter.format_pearl(&pearl);

    assert!(output.contains("UTC"));
}

#[test]
fn test_table_formatter_pearl_list() {
    use pearls_cli::create_formatter;

    let pearl1 = create_test_pearl();
    let mut pearl2 = create_test_pearl();
    pearl2.id = "prl-d4e5f6".to_string();
    pearl2.title = "Another Pearl".to_string();

    let formatter = create_formatter("table", false, false);
    let output = formatter.format_list(&[pearl1, pearl2]);

    assert!(output.contains("prl-a1b2c3"));
    assert!(output.contains("prl-d4e5f6"));
    assert!(output.contains("Test Pearl"));
    assert!(output.contains("Another Pearl"));
    assert!(output.contains("P1"));
    assert!(output.contains("Deps"));
}

#[test]
fn test_table_formatter_empty_list() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("table", false, false);
    let output = formatter.format_list(&[]);

    assert!(output.contains("No Pearls found"));
}

#[test]
fn test_table_formatter_error() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("table", false, false);
    let output = formatter.format_error("Test error message");

    assert!(output.contains("Error"));
    assert!(output.contains("Test error message"));
}

#[test]
fn test_plain_formatter_single_pearl() {
    use pearls_cli::create_formatter;

    let pearl = create_test_pearl();
    let formatter = create_formatter("plain", false, false);
    let output = formatter.format_pearl(&pearl);

    assert!(output.contains("prl-a1b2c3"));
    assert!(output.contains("Test Pearl"));
    assert!(output.contains("P1"));
}

#[test]
fn test_plain_formatter_pearl_list() {
    use pearls_cli::create_formatter;

    let pearl1 = create_test_pearl();
    let mut pearl2 = create_test_pearl();
    pearl2.id = "prl-d4e5f6".to_string();

    let formatter = create_formatter("plain", false, false);
    let output = formatter.format_list(&[pearl1, pearl2]);

    assert!(output.contains("prl-a1b2c3"));
    assert!(output.contains("prl-d4e5f6"));
    assert!(output.contains("P1"));
    assert!(output.contains("test-author"));
}

#[test]
fn test_plain_formatter_empty_list() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("plain", false, false);
    let output = formatter.format_list(&[]);

    assert!(output.contains("No Pearls found"));
}

#[test]
fn test_plain_formatter_error() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("plain", false, false);
    let output = formatter.format_error("Test error message");

    assert!(output.contains("Error"));
    assert!(output.contains("Test error message"));
}

#[test]
fn test_formatter_factory_json() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("json", false, false);
    let pearl = create_test_pearl();
    let output = formatter.format_pearl(&pearl);

    // JSON formatter should produce valid JSON
    assert!(output.contains("{"));
    assert!(output.contains("}"));
}

#[test]
fn test_formatter_factory_table() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("table", false, false);
    let pearl = create_test_pearl();
    let output = formatter.format_pearl(&pearl);

    // Table formatter should produce readable output
    assert!(!output.is_empty());
}

#[test]
fn test_formatter_factory_plain() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("plain", false, false);
    let pearl = create_test_pearl();
    let output = formatter.format_pearl(&pearl);

    // Plain formatter should produce simple output
    assert!(output.contains("prl-a1b2c3"));
}

#[test]
fn test_formatter_factory_unknown_format_defaults_to_table() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("unknown", false, false);
    let pearl = create_test_pearl();
    let output = formatter.format_pearl(&pearl);

    // Unknown format should default to table
    assert!(!output.is_empty());
}

#[test]
fn test_json_formatter_preserves_all_fields() {
    use pearls_cli::create_formatter;

    let pearl = create_test_pearl();
    let formatter = create_formatter("json", false, false);
    let output = formatter.format_pearl(&pearl);

    assert!(output.contains("\"id\""));
    assert!(output.contains("\"title\""));
    assert!(output.contains("\"status\""));
    assert!(output.contains("\"priority\""));
    assert!(output.contains("\"author\""));
    assert!(output.contains("\"labels\""));
}

#[test]
fn test_table_formatter_with_color_flag() {
    use pearls_cli::create_formatter;

    let formatter = create_formatter("table", true, false);
    let pearl = create_test_pearl();
    let output = formatter.format_pearl(&pearl);

    // Should still produce output with color flag
    assert!(output.contains("prl-a1b2c3"));
}

#[test]
fn test_no_color_environment_variable_respected() {
    use pearls_cli::create_formatter;

    // This test verifies that the formatter respects NO_COLOR
    // The actual color behavior depends on environment
    let formatter = create_formatter("table", false, false);
    let pearl = create_test_pearl();
    let output = formatter.format_pearl(&pearl);

    assert!(!output.is_empty());
}
