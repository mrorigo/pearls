// Rust guideline compliant 2026-02-07

//! Regression tests for global output flags.

use serde_json::Value;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn init_pearls_repo(temp_dir: &TempDir) {
    let pearls_dir = temp_dir.path().join(".pearls");
    fs::create_dir(&pearls_dir).expect("create .pearls dir");
    fs::write(pearls_dir.join("issues.jsonl"), "").expect("create issues.jsonl");
    fs::write(
        pearls_dir.join("config.toml"),
        "default_priority = 2\ncompact_threshold_days = 30\nuse_index = false\noutput_format = \"table\"\nauto_close_on_commit = false\n",
    )
    .expect("create config");
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected success, got status: {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_json_stdout(output: &std::process::Output) -> Value {
    assert_success(output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|err| {
        panic!(
            "expected valid JSON output, parse error: {}\n{}",
            err, stdout
        )
    })
}

#[test]
fn prl_list_json_flag_outputs_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["list", "--json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert!(payload.get("total").is_some(), "missing total field");
}

#[test]
fn prl_comments_list_json_flag_outputs_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);
    let pearls_dir = temp_dir.path().join(".pearls");

    let mut pearl = pearls_core::Pearl::new("With comments".to_string(), "author".to_string());
    let _ = pearl
        .add_comment("reviewer".to_string(), "first".to_string())
        .expect("seed comment");
    let pearl_id = pearl.id.clone();
    let mut storage =
        pearls_core::Storage::new(pearls_dir.join("issues.jsonl")).expect("create storage");
    storage.save(&pearl).expect("save pearl");

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["comments", "list", &pearl_id, "--json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert!(payload.get("comments").is_some(), "missing comments field");
}

#[test]
fn prl_create_json_flag_outputs_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["create", "JSON create", "--json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert_eq!(
        payload.get("action").and_then(Value::as_str),
        Some("create"),
        "expected create action"
    );
}

#[test]
fn prl_show_json_is_pure_json_without_human_suffix() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);
    let pearls_dir = temp_dir.path().join(".pearls");

    let parent = pearls_core::Pearl::new("Parent".to_string(), "author".to_string());
    let mut child = pearls_core::Pearl::new("Child".to_string(), "author".to_string());
    child.deps.push(pearls_core::Dependency {
        target_id: parent.id.clone(),
        dep_type: pearls_core::DepType::ParentChild,
    });
    child
        .add_comment("reviewer".to_string(), "ship it".to_string())
        .expect("seed comment");
    let child_id = child.id.clone();

    let mut storage =
        pearls_core::Storage::new(pearls_dir.join("issues.jsonl")).expect("create storage");
    storage.save(&parent).expect("save parent");
    storage.save(&child).expect("save child");

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["show", &child_id, "--json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert_eq!(
        payload.get("id").and_then(Value::as_str),
        Some(child_id.as_str())
    );
    assert!(payload.get("deps").is_some(), "missing deps in JSON");
    assert!(
        payload.get("comments").is_some(),
        "missing comments in JSON"
    );
}

#[test]
fn prl_show_format_json_is_pure_json_without_human_suffix() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);
    let pearls_dir = temp_dir.path().join(".pearls");

    let parent = pearls_core::Pearl::new("Parent".to_string(), "author".to_string());
    let mut child = pearls_core::Pearl::new("Child".to_string(), "author".to_string());
    child.deps.push(pearls_core::Dependency {
        target_id: parent.id.clone(),
        dep_type: pearls_core::DepType::ParentChild,
    });
    child
        .add_comment("reviewer".to_string(), "ship it".to_string())
        .expect("seed comment");
    let child_id = child.id.clone();

    let mut storage =
        pearls_core::Storage::new(pearls_dir.join("issues.jsonl")).expect("create storage");
    storage.save(&parent).expect("save parent");
    storage.save(&child).expect("save child");

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["show", &child_id, "--format", "json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert_eq!(payload.get("id").and_then(Value::as_str), Some(child_id.as_str()));
    assert!(payload.get("deps").is_some(), "missing deps in JSON");
    assert!(payload.get("comments").is_some(), "missing comments in JSON");
}

#[test]
fn prl_ready_json_outputs_valid_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);
    let pearls_dir = temp_dir.path().join(".pearls");

    let pearl = pearls_core::Pearl::new("Ready task".to_string(), "author".to_string());
    let mut storage =
        pearls_core::Storage::new(pearls_dir.join("issues.jsonl")).expect("create storage");
    storage.save(&pearl).expect("save pearl");

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["ready", "--json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert!(payload.get("ready").is_some(), "missing ready field");
    assert!(payload.get("total").is_some(), "missing total field");
}

#[test]
fn prl_compact_dry_run_json_outputs_valid_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["compact", "--dry-run", "--json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert_eq!(
        payload.get("action").and_then(Value::as_str),
        Some("compact"),
        "expected compact action"
    );
}

#[test]
fn prl_doctor_json_outputs_valid_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    init_pearls_repo(&temp_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_prl"))
        .current_dir(temp_dir.path())
        .args(["doctor", "--json"])
        .output()
        .expect("run prl");

    let payload = assert_json_stdout(&output);
    assert_eq!(
        payload.get("action").and_then(Value::as_str),
        Some("doctor"),
        "expected doctor action"
    );
}
