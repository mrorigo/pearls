// Rust guideline compliant 2026-02-06

//! Integration tests for CLI commands.

use git2::Repository;
use pearls_core::{DepType, IssueGraph, Storage};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;

/// Helper to verify .pearls directory structure.
fn verify_pearls_dir(pearls_dir: &Path) {
    assert!(pearls_dir.exists(), ".pearls directory should exist");
    assert!(
        pearls_dir.join("issues.jsonl").exists(),
        "issues.jsonl should exist"
    );
    assert!(
        pearls_dir.join("config.toml").exists(),
        "config.toml should exist"
    );
}

static DIR_LOCK: Mutex<()> = Mutex::new(());

struct DirGuard {
    previous: PathBuf,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

fn enter_dir(path: &Path) -> DirGuard {
    let lock = DIR_LOCK.lock().expect("Failed to lock dir mutex");
    let previous = std::env::current_dir().expect("Failed to read current dir");
    std::env::set_current_dir(path).expect("Failed to change current dir");
    DirGuard {
        previous,
        _lock: lock,
    }
}

fn init_repo(base: &Path) -> PathBuf {
    let pearls_dir = base.join(".pearls");
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");
    let config = pearls_core::Config::default();
    config.save(&pearls_dir).expect("Failed to save config");
    pearls_dir
}

fn init_git_repo(path: &Path) {
    Repository::init(path).expect("Failed to init git repo");
}

fn create_commit(repo: &Repository, message: &str) {
    let sig = repo.signature().unwrap_or_else(|_| {
        git2::Signature::now("tester", "tester@example.com").expect("Failed to create signature")
    });
    let tree_id = {
        let mut index = repo.index().expect("Failed to get index");
        index.write_tree().expect("Failed to write tree")
    };
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");
    let head = repo.head().ok();
    let parent_commits = head
        .and_then(|h| h.target())
        .and_then(|oid| repo.find_commit(oid).ok())
        .map(|commit| vec![commit])
        .unwrap_or_default();
    let parents: Vec<&git2::Commit> = parent_commits.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
        .expect("Failed to commit");
}

fn add_file_and_commit(repo: &Repository, path: &Path, content: &str, message: &str) {
    std::fs::write(path, content).expect("Failed to write file");
    let mut index = repo.index().expect("Failed to get index");
    index
        .add_path(Path::new(path.file_name().unwrap()))
        .expect("Failed to add path");
    index.write().expect("Failed to write index");
    create_commit(repo, message);
}

#[test]
fn test_init_creates_correct_structure() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Manually create the structure (simulating init)
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    let config = pearls_core::Config::default();
    config.save(&pearls_dir).expect("Failed to save config");

    // Verify structure
    verify_pearls_dir(&pearls_dir);

    // Verify issues.jsonl is empty
    let issues_content =
        fs::read_to_string(pearls_dir.join("issues.jsonl")).expect("Failed to read issues.jsonl");
    assert_eq!(
        issues_content, "",
        "issues.jsonl should be empty after init"
    );

    // Verify config.toml contains default values
    let config_content =
        fs::read_to_string(pearls_dir.join("config.toml")).expect("Failed to read config.toml");
    assert!(
        config_content.contains("default_priority"),
        "config.toml should contain default_priority"
    );
}

#[test]
fn test_create_adds_pearl_to_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");
    let config = pearls_core::Config::default();
    config.save(&pearls_dir).expect("Failed to save config");

    // Create a Pearl directly using the storage API
    let mut pearl = pearls_core::Pearl::new("Test Pearl".to_string(), "test-author".to_string());
    pearl.description = "A test description".to_string();
    pearl.priority = 1;
    pearl.labels = vec!["test".to_string()];

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl).expect("Failed to save pearl");

    // Verify Pearl was added to file
    let issues_path = pearls_dir.join("issues.jsonl");
    let content = fs::read_to_string(&issues_path).expect("Failed to read issues.jsonl");
    assert!(!content.is_empty(), "issues.jsonl should not be empty");
    assert!(
        content.contains("Test Pearl"),
        "issues.jsonl should contain the Pearl title"
    );
    assert!(
        content.contains("test-author"),
        "issues.jsonl should contain the author"
    );

    // Verify it's valid JSON Lines format (one line per Pearl)
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "Should have exactly one Pearl");

    // Verify the line is valid JSON
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(lines[0]);
    assert!(parsed.is_ok(), "Pearl should be valid JSON");
}

#[test]
fn test_show_retrieves_correct_pearl() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");
    let config = pearls_core::Config::default();
    config.save(&pearls_dir).expect("Failed to save config");

    // Create a Pearl
    let pearl = pearls_core::Pearl::new("Show Test Pearl".to_string(), "test-author".to_string());
    let pearl_id = pearl.id.clone();

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl).expect("Failed to save pearl");

    // Load the Pearl back
    let loaded = storage.load_by_id(&pearl_id).expect("Failed to load pearl");
    assert_eq!(loaded.id, pearl_id);
    assert_eq!(loaded.title, "Show Test Pearl");
}

#[test]
fn test_list_with_filters() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");
    let config = pearls_core::Config::default();
    config.save(&pearls_dir).expect("Failed to save config");

    // Create multiple Pearls with different properties
    let mut pearl1 =
        pearls_core::Pearl::new("High Priority Pearl".to_string(), "alice".to_string());
    pearl1.priority = 0;
    pearl1.labels = vec!["urgent".to_string()];

    let mut pearl2 = pearls_core::Pearl::new("Low Priority Pearl".to_string(), "bob".to_string());
    pearl2.priority = 3;
    pearl2.labels = vec!["backlog".to_string()];

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl1).expect("Failed to save pearl1");
    storage.save(&pearl2).expect("Failed to save pearl2");

    // Load all and verify
    let all_pearls = storage.load_all().expect("Failed to load all pearls");
    assert_eq!(all_pearls.len(), 2, "Should have 2 pearls");

    // Filter by priority
    let high_priority: Vec<_> = all_pearls.iter().filter(|p| p.priority == 0).collect();
    assert_eq!(high_priority.len(), 1, "Should have 1 high priority pearl");

    // Filter by author
    let alice_pearls: Vec<_> = all_pearls.iter().filter(|p| p.author == "alice").collect();
    assert_eq!(alice_pearls.len(), 1, "Should have 1 pearl from alice");

    // Filter by label
    let urgent_pearls: Vec<_> = all_pearls
        .iter()
        .filter(|p| p.labels.iter().any(|l| l.eq_ignore_ascii_case("urgent")))
        .collect();
    assert_eq!(urgent_pearls.len(), 1, "Should have 1 urgent pearl");
}

#[test]
fn test_init_idempotent() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Initialize twice
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");
    let config = pearls_core::Config::default();
    config.save(&pearls_dir).expect("Failed to save config");

    // Initialize again (should not fail)
    fs::create_dir_all(&pearls_dir).ok(); // Ignore if already exists
    let config2 = pearls_core::Config::default();
    config2
        .save(&pearls_dir)
        .expect("Failed to save config again");

    // Verify structure is still correct
    verify_pearls_dir(&pearls_dir);
}

#[test]
fn test_list_empty_repository() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up empty repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    // Load from empty repository
    let storage = Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(pearls.len(), 0, "Empty repository should have no pearls");
}

#[test]
fn test_partial_id_resolution() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    // Create a Pearl
    let pearl = pearls_core::Pearl::new("Test Pearl".to_string(), "test-author".to_string());
    let full_id = pearl.id.clone();

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl).expect("Failed to save pearl");

    // Load all pearls for partial ID resolution
    let pearls = storage.load_all().expect("Failed to load pearls");

    // Test partial ID resolution
    let partial_id = &full_id[..7]; // "prl-abc"
    let resolved = pearls_core::identity::resolve_partial_id(partial_id, &pearls);
    assert!(resolved.is_ok(), "Should resolve partial ID");
    assert_eq!(
        resolved.unwrap(),
        full_id,
        "Should resolve to correct full ID"
    );
}

#[test]
fn test_update_pearl_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    // Create a Pearl
    let pearl = pearls_core::Pearl::new("Original Title".to_string(), "test-author".to_string());
    let pearl_id = pearl.id.clone();

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl).expect("Failed to save pearl");

    // Update the Pearl
    let mut updated_pearl = storage.load_by_id(&pearl_id).expect("Failed to load pearl");
    updated_pearl.title = "Updated Title".to_string();
    updated_pearl.priority = 1;
    updated_pearl.labels.push("urgent".to_string());

    storage
        .save(&updated_pearl)
        .expect("Failed to save updated pearl");

    // Verify the update
    let reloaded = storage
        .load_by_id(&pearl_id)
        .expect("Failed to reload pearl");
    assert_eq!(reloaded.title, "Updated Title");
    assert_eq!(reloaded.priority, 1);
    assert!(reloaded.labels.contains(&"urgent".to_string()));
}

#[test]
fn test_close_pearl_without_blocking_deps() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    // Create a Pearl
    let mut pearl = pearls_core::Pearl::new("Test Pearl".to_string(), "test-author".to_string());
    pearl.status = pearls_core::Status::Open;
    let pearl_id = pearl.id.clone();

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl).expect("Failed to save pearl");

    // Load and close the Pearl
    let mut pearl_to_close = storage.load_by_id(&pearl_id).expect("Failed to load pearl");
    let all_pearls = storage.load_all().expect("Failed to load all pearls");
    let graph =
        pearls_core::graph::IssueGraph::from_pearls(all_pearls).expect("Failed to create graph");

    // Validate transition
    assert!(
        pearls_core::fsm::validate_transition(&pearl_to_close, pearls_core::Status::Closed, &graph)
            .is_ok(),
        "Should allow closing unblocked Pearl"
    );

    pearl_to_close.status = pearls_core::Status::Closed;
    storage
        .save(&pearl_to_close)
        .expect("Failed to save closed pearl");

    // Verify the Pearl is closed
    let reloaded = storage
        .load_by_id(&pearl_id)
        .expect("Failed to reload pearl");
    assert_eq!(reloaded.status, pearls_core::Status::Closed);
}

#[test]
fn test_ready_queue_ordering() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    // Create multiple Pearls with different priorities
    let mut pearl1 = pearls_core::Pearl::new("Low Priority".to_string(), "alice".to_string());
    pearl1.priority = 3;
    pearl1.status = pearls_core::Status::Open;

    let mut pearl2 = pearls_core::Pearl::new("High Priority".to_string(), "bob".to_string());
    pearl2.priority = 0;
    pearl2.status = pearls_core::Status::Open;

    let mut pearl3 = pearls_core::Pearl::new("Medium Priority".to_string(), "charlie".to_string());
    pearl3.priority = 2;
    pearl3.status = pearls_core::Status::Open;

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl1).expect("Failed to save pearl1");
    storage.save(&pearl2).expect("Failed to save pearl2");
    storage.save(&pearl3).expect("Failed to save pearl3");

    // Build graph and get ready queue
    let all_pearls = storage.load_all().expect("Failed to load all pearls");
    let graph =
        pearls_core::graph::IssueGraph::from_pearls(all_pearls).expect("Failed to create graph");
    let ready = graph.ready_queue();

    // Verify ordering: should be sorted by priority ascending
    assert_eq!(ready.len(), 3, "Should have 3 ready Pearls");
    assert_eq!(ready[0].priority, 0, "First should be P0");
    assert_eq!(ready[1].priority, 2, "Second should be P2");
    assert_eq!(ready[2].priority, 3, "Third should be P3");
}

#[test]
fn test_ready_queue_excludes_blocked() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    // Create two Pearls with a blocking dependency
    let mut blocker = pearls_core::Pearl::new("Blocker".to_string(), "alice".to_string());
    blocker.status = pearls_core::Status::Open;
    let blocker_id = blocker.id.clone();

    let mut blocked = pearls_core::Pearl::new("Blocked".to_string(), "bob".to_string());
    blocked.status = pearls_core::Status::Open;
    blocked.deps.push(pearls_core::Dependency {
        target_id: blocker_id.clone(),
        dep_type: pearls_core::DepType::Blocks,
    });
    let blocked_id = blocked.id.clone();

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&blocker).expect("Failed to save blocker");
    storage.save(&blocked).expect("Failed to save blocked");

    // Build graph and get ready queue
    let all_pearls = storage.load_all().expect("Failed to load all pearls");
    let graph =
        pearls_core::graph::IssueGraph::from_pearls(all_pearls).expect("Failed to create graph");
    let ready = graph.ready_queue();

    // Verify only the blocker is ready
    assert_eq!(ready.len(), 1, "Should have 1 ready Pearl");
    assert_eq!(ready[0].id, blocker_id, "Only blocker should be ready");

    // Verify the blocked Pearl is not in the ready queue
    let ready_ids: Vec<&str> = ready.iter().map(|p| p.id.as_str()).collect();
    assert!(
        !ready_ids.contains(&blocked_id.as_str()),
        "Blocked Pearl should not be ready"
    );
}

#[test]
fn test_ready_queue_excludes_closed_and_deferred() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let pearls_dir = temp_dir.path().join(".pearls");

    // Set up repository
    fs::create_dir(&pearls_dir).expect("Failed to create .pearls directory");
    fs::File::create(pearls_dir.join("issues.jsonl")).expect("Failed to create issues.jsonl");

    // Create Pearls with different statuses
    let mut open_pearl = pearls_core::Pearl::new("Open".to_string(), "alice".to_string());
    open_pearl.status = pearls_core::Status::Open;

    let mut closed_pearl = pearls_core::Pearl::new("Closed".to_string(), "bob".to_string());
    closed_pearl.status = pearls_core::Status::Closed;

    let mut deferred_pearl = pearls_core::Pearl::new("Deferred".to_string(), "charlie".to_string());
    deferred_pearl.status = pearls_core::Status::Deferred;

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage
        .save(&open_pearl)
        .expect("Failed to save open pearl");
    storage
        .save(&closed_pearl)
        .expect("Failed to save closed pearl");
    storage
        .save(&deferred_pearl)
        .expect("Failed to save deferred pearl");

    // Build graph and get ready queue
    let all_pearls = storage.load_all().expect("Failed to load all pearls");
    let graph =
        pearls_core::graph::IssueGraph::from_pearls(all_pearls).expect("Failed to create graph");
    let ready = graph.ready_queue();

    // Verify only the open Pearl is ready
    assert_eq!(ready.len(), 1, "Should have 1 ready Pearl");
    assert_eq!(
        ready[0].status,
        pearls_core::Status::Open,
        "Only open Pearl should be ready"
    );
}

#[test]
fn test_link_creates_dependency() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let pearl_a = pearls_core::Pearl::new("Pearl A".to_string(), "alice".to_string());
    let pearl_b = pearls_core::Pearl::new("Pearl B".to_string(), "bob".to_string());

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl_a).expect("Failed to save Pearl A");
    storage.save(&pearl_b).expect("Failed to save Pearl B");

    pearls_cli::commands::link::execute(
        pearl_a.id.clone(),
        pearl_b.id.clone(),
        "blocks".to_string(),
    )
    .expect("Failed to link Pearls");

    let updated = storage
        .load_by_id(&pearl_a.id)
        .expect("Failed to load updated Pearl A");
    assert!(updated
        .deps
        .iter()
        .any(|dep| { dep.target_id == pearl_b.id && dep.dep_type == DepType::Blocks }));
}

#[test]
fn test_link_detects_cycles() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let pearl_a = pearls_core::Pearl::new("Pearl A".to_string(), "alice".to_string());
    let pearl_b = pearls_core::Pearl::new("Pearl B".to_string(), "bob".to_string());

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl_a).expect("Failed to save Pearl A");
    storage.save(&pearl_b).expect("Failed to save Pearl B");

    pearls_cli::commands::link::execute(
        pearl_b.id.clone(),
        pearl_a.id.clone(),
        "blocks".to_string(),
    )
    .expect("Failed to link Pearls");

    let result = pearls_cli::commands::link::execute(
        pearl_a.id.clone(),
        pearl_b.id.clone(),
        "blocks".to_string(),
    );

    assert!(result.is_err(), "Cycle should be rejected");
}

#[test]
fn test_unlink_removes_dependency() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let pearl_a = pearls_core::Pearl::new("Pearl A".to_string(), "alice".to_string());
    let pearl_b = pearls_core::Pearl::new("Pearl B".to_string(), "bob".to_string());

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl_a).expect("Failed to save Pearl A");
    storage.save(&pearl_b).expect("Failed to save Pearl B");

    pearls_cli::commands::link::execute(
        pearl_a.id.clone(),
        pearl_b.id.clone(),
        "related".to_string(),
    )
    .expect("Failed to link Pearls");

    pearls_cli::commands::unlink::execute(pearl_a.id.clone(), pearl_b.id.clone())
        .expect("Failed to unlink Pearls");

    let updated = storage
        .load_by_id(&pearl_a.id)
        .expect("Failed to load updated Pearl A");
    assert!(!updated.deps.iter().any(|dep| dep.target_id == pearl_b.id));
}

#[test]
fn test_blocking_dependency_affects_fsm() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let blocker = pearls_core::Pearl::new("Blocker".to_string(), "alice".to_string());
    let blocked = pearls_core::Pearl::new("Blocked".to_string(), "bob".to_string());

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&blocker).expect("Failed to save blocker");
    storage.save(&blocked).expect("Failed to save blocked");

    pearls_cli::commands::link::execute(
        blocked.id.clone(),
        blocker.id.clone(),
        "blocks".to_string(),
    )
    .expect("Failed to link Pearls");

    let all = storage.load_all().expect("Failed to load pearls");
    let graph = IssueGraph::from_pearls(all).expect("Failed to build graph");
    assert!(graph.is_blocked(&blocked.id));
}

#[test]
fn test_status_command_runs_in_git_repo() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    init_git_repo(temp_dir.path());
    let _guard = enter_dir(temp_dir.path());
    init_repo(temp_dir.path());

    pearls_cli::commands::status::execute(false).expect("Status command failed");
}

#[test]
fn test_compact_archives_closed_pearls() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let mut old_closed = pearls_core::Pearl::new("Old Closed".to_string(), "alice".to_string());
    old_closed.status = pearls_core::Status::Closed;
    old_closed.updated_at = (chrono::Utc::now() - chrono::Duration::days(30)).timestamp();

    let open = pearls_core::Pearl::new("Open".to_string(), "bob".to_string());

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage
        .save_all(&[old_closed.clone(), open.clone()])
        .expect("Failed to save pearls");

    pearls_cli::commands::compact::execute(Some(7), false).expect("Compact failed");

    let active = storage.load_all().expect("Failed to load active pearls");
    assert_eq!(active.len(), 1, "Only one active Pearl should remain");
    assert_eq!(active[0].id, open.id);

    let archive_storage =
        Storage::new(pearls_dir.join("archive.jsonl")).expect("Failed to create archive storage");
    let archived = archive_storage
        .load_all()
        .expect("Failed to load archived pearls");
    assert_eq!(archived.len(), 1, "One Pearl should be archived");
    assert_eq!(archived[0].id, old_closed.id);
}

#[test]
fn test_compact_dry_run_keeps_files() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let mut old_closed = pearls_core::Pearl::new("Old Closed".to_string(), "alice".to_string());
    old_closed.status = pearls_core::Status::Closed;
    old_closed.updated_at = (chrono::Utc::now() - chrono::Duration::days(30)).timestamp();

    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&old_closed).expect("Failed to save pearl");

    pearls_cli::commands::compact::execute(Some(7), true).expect("Compact dry run failed");

    let active = storage.load_all().expect("Failed to load active pearls");
    assert_eq!(active.len(), 1, "Dry run should not archive Pearls");
    assert!(!pearls_dir.join("archive.jsonl").exists());
}

#[test]
fn test_doctor_fix_repairs_common_issues() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let pearl = pearls_core::Pearl::new("Dup".to_string(), "alice".to_string());
    let mut orphaned = pearls_core::Pearl::new("Orphan".to_string(), "bob".to_string());
    orphaned.deps.push(pearls_core::Dependency {
        target_id: "prl-missing".to_string(),
        dep_type: DepType::Related,
    });

    let mut content = String::new();
    content.push_str(&serde_json::to_string(&pearl).unwrap());
    content.push('\n');
    content.push_str(&serde_json::to_string(&pearl).unwrap());
    content.push('\n');
    content.push_str(&serde_json::to_string(&orphaned).unwrap());
    content.push('\n');
    content.push_str("{invalid json}\n");

    fs::write(pearls_dir.join("issues.jsonl"), content).expect("Failed to write issues.jsonl");

    pearls_cli::commands::doctor::execute(true).expect("Doctor fix failed");

    let storage = Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(
        pearls.len(),
        2,
        "Doctor should remove duplicate and invalid lines"
    );
    for pearl in &pearls {
        assert!(pearl.deps.is_empty(), "Orphaned deps should be removed");
    }
}

#[test]
fn test_import_beads_writes_issues() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let pearl = pearls_core::Pearl::new("Imported".to_string(), "alice".to_string());
    let beads_path = temp_dir.path().join("beads.jsonl");
    let line = serde_json::to_string(&pearl).unwrap();
    fs::write(&beads_path, format!("{}\n", line)).expect("Failed to write beads file");

    pearls_cli::commands::import::import_beads(beads_path.to_string_lossy().to_string())
        .expect("Import failed");

    let storage = Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(pearls.len(), 1);
    assert_eq!(pearls[0].id, pearl.id);
}

#[test]
fn test_meta_set_updates_metadata() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    let pearls_dir = init_repo(temp_dir.path());

    let pearl = pearls_core::Pearl::new("Meta".to_string(), "alice".to_string());
    let mut storage =
        Storage::new(pearls_dir.join("issues.jsonl")).expect("Failed to create storage");
    storage.save(&pearl).expect("Failed to save pearl");

    pearls_cli::commands::meta::set(pearl.id.clone(), "key".to_string(), "\"value\"".to_string())
        .expect("Meta set failed");

    let updated = storage.load_by_id(&pearl.id).expect("Failed to load pearl");
    assert_eq!(
        updated.metadata.get("key"),
        Some(&serde_json::Value::String("value".to_string()))
    );
}

#[test]
fn test_create_with_description_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _guard = enter_dir(temp_dir.path());
    init_repo(temp_dir.path());

    let desc_path = temp_dir.path().join("desc.md");
    std::fs::write(&desc_path, "Hello **world**").expect("Failed to write desc");

    pearls_cli::commands::create::execute(
        "Desc Pearl".to_string(),
        None,
        Some(desc_path.to_string_lossy().to_string()),
        None,
        vec![],
        Some("author".to_string()),
    )
    .expect("Create with description file failed");
}

#[test]
fn test_sync_dry_run() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    init_git_repo(temp_dir.path());
    let _guard = enter_dir(temp_dir.path());
    init_repo(temp_dir.path());

    let repo = Repository::open(temp_dir.path()).expect("Failed to open repo");
    add_file_and_commit(&repo, Path::new("README.md"), "test", "init");

    // Set up a bare remote and push initial commit
    let remote_dir = TempDir::new().expect("Failed to create remote");
    Repository::init_bare(remote_dir.path()).expect("Failed to init bare repo");
    repo.remote("origin", remote_dir.path().to_str().unwrap())
        .expect("Failed to add remote");
    {
        let mut remote = repo.find_remote("origin").expect("Failed to find remote");
        remote
            .push(&["refs/heads/master:refs/heads/master"], None)
            .expect("Failed to push");
    }

    // Dry run should succeed without contacting remote
    pearls_cli::commands::sync::execute(true).expect("Sync dry run failed");
}

#[test]
fn test_sync_pushes_to_remote() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    init_git_repo(temp_dir.path());
    let _guard = enter_dir(temp_dir.path());
    init_repo(temp_dir.path());

    let repo = Repository::open(temp_dir.path()).expect("Failed to open repo");
    add_file_and_commit(&repo, Path::new("README.md"), "test", "init");

    let remote_dir = TempDir::new().expect("Failed to create remote");
    Repository::init_bare(remote_dir.path()).expect("Failed to init bare repo");
    repo.remote("origin", remote_dir.path().to_str().unwrap())
        .expect("Failed to add remote");

    {
        let mut remote = repo.find_remote("origin").expect("Failed to find remote");
        remote
            .push(&["refs/heads/master:refs/heads/master"], None)
            .expect("Failed to push");
    }

    pearls_cli::commands::sync::execute(false).expect("Sync failed");
}
