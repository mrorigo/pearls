// Rust guideline compliant 2026-02-07

//! Regression tests for global output flags.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn prl_list_json_flag_outputs_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    let pearls_dir = temp_dir.path().join(".pearls");
    fs::create_dir(&pearls_dir).expect("create .pearls dir");
    fs::write(pearls_dir.join("issues.jsonl"), "").expect("create issues.jsonl");

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["list", "--json"])
        .output()
        .expect("run prl");

    assert!(
        output.status.success(),
        "expected success, got status: {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim_start().starts_with('{'),
        "expected JSON output, got:\n{}",
        stdout
    );
    assert!(
        stdout.contains("\"total\""),
        "expected JSON 'total' field, got:\n{}",
        stdout
    );
}
