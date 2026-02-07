// Rust guideline compliant 2026-02-06

//! End-to-end integration tests for Pearls workflows.

use git2::Repository;
use pearls_cli::OutputFormatter;
use pearls_core::{Pearl, Status, Storage};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;

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

struct CaptureFormatter {
    captured: Mutex<Vec<Pearl>>,
}

impl CaptureFormatter {
    fn new() -> Self {
        Self {
            captured: Mutex::new(Vec::new()),
        }
    }
}

impl OutputFormatter for CaptureFormatter {
    fn format_pearl(&self, pearl: &Pearl) -> String {
        self.captured
            .lock()
            .expect("capture lock")
            .push(pearl.clone());
        "ok".to_string()
    }

    fn format_list(&self, pearls: &[Pearl]) -> String {
        self.captured
            .lock()
            .expect("capture lock")
            .extend_from_slice(pearls);
        "ok".to_string()
    }

    fn format_error(&self, error: &str) -> String {
        error.to_string()
    }
}

#[test]
fn test_full_workflow_end_to_end() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    Repository::init(temp_dir.path()).expect("Failed to init git repo");
    let _guard = enter_dir(temp_dir.path());

    pearls_cli::commands::init::execute().expect("Init failed");

    pearls_cli::commands::create::execute(
        "First Pearl".to_string(),
        None,
        None,
        None,
        vec![],
        Some("author".to_string()),
    )
    .expect("Create first failed");

    pearls_cli::commands::create::execute(
        "Second Pearl".to_string(),
        None,
        None,
        None,
        vec![],
        Some("author".to_string()),
    )
    .expect("Create second failed");

    let storage = Storage::new(temp_dir.path().join(".pearls/issues.jsonl"))
        .expect("Failed to create storage");
    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(pearls.len(), 2, "Expected two pearls");

    let first_id = pearls[0].id.clone();
    let second_id = pearls[1].id.clone();

    pearls_cli::commands::link::execute(second_id.clone(), first_id.clone(), "blocks".to_string())
        .expect("Link failed");

    pearls_cli::commands::ready::execute(None).expect("Ready failed");

    pearls_cli::commands::close::execute(first_id.clone()).expect("Close failed");

    let mut storage = Storage::new(temp_dir.path().join(".pearls/issues.jsonl"))
        .expect("Failed to create storage");
    let mut pearl = storage.load_by_id(&first_id).expect("Failed to load pearl");
    pearl.status = Status::Closed;
    pearl.updated_at = (chrono::Utc::now() - chrono::Duration::days(40)).timestamp();
    storage.save(&pearl).expect("Failed to save updated pearl");

    pearls_cli::commands::compact::execute(Some(30), false).expect("Compact failed");

    let archive_path = temp_dir.path().join(".pearls/archive.jsonl");
    assert!(archive_path.exists(), "Archive file should exist");
    let archive_storage = Storage::new(archive_path).expect("Failed to create archive storage");
    let archived = archive_storage.load_all().expect("Failed to load archive");
    assert!(
        archived.iter().any(|p| p.id == first_id),
        "Closed pearl should be archived"
    );
}

#[test]
fn test_git_integration_setup() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    Repository::init(temp_dir.path()).expect("Failed to init git repo");
    let _guard = enter_dir(temp_dir.path());

    pearls_cli::commands::init::execute().expect("Init failed");

    let gitattributes = temp_dir.path().join(".gitattributes");
    assert!(gitattributes.exists(), ".gitattributes should exist");

    let hooks_dir = temp_dir.path().join(".git/hooks");
    assert!(
        hooks_dir.join("pre-commit").exists(),
        "pre-commit hook should exist"
    );
    assert!(
        hooks_dir.join("post-merge").exists(),
        "post-merge hook should exist"
    );

    let contents = fs::read_to_string(gitattributes).expect("Failed to read .gitattributes");
    assert!(
        contents.contains("issues.jsonl"),
        ".gitattributes should include issues.jsonl"
    );
}

#[test]
fn test_concurrent_access_saves_all_pearls() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    Repository::init(temp_dir.path()).expect("Failed to init git repo");
    let _guard = enter_dir(temp_dir.path());

    pearls_cli::commands::init::execute().expect("Init failed");

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let path = temp_dir.path().join(".pearls/issues.jsonl");
            std::thread::spawn(move || {
                let mut storage = Storage::new(path).expect("Failed to create storage");
                let pearl = Pearl::new(format!("Pearl {}", i), "author".to_string());
                storage
                    .with_lock(|storage| storage.save(&pearl))
                    .expect("Failed to save pearl");
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread failed");
    }

    let storage = Storage::new(temp_dir.path().join(".pearls/issues.jsonl"))
        .expect("Failed to create storage");
    let pearls = storage.load_all().expect("Failed to load pearls");
    assert_eq!(pearls.len(), 4, "Expected all pearls to be saved");
}

#[test]
fn test_large_repository_listing() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    Repository::init(temp_dir.path()).expect("Failed to init git repo");
    let _guard = enter_dir(temp_dir.path());

    pearls_cli::commands::init::execute().expect("Init failed");

    let mut storage = Storage::new(temp_dir.path().join(".pearls/issues.jsonl"))
        .expect("Failed to create storage");
    let pearls: Vec<Pearl> = (0..2000)
        .map(|i| Pearl::new(format!("Pearl {}", i), "author".to_string()))
        .collect();
    storage.save_all(&pearls).expect("Failed to save pearls");

    let formatter = CaptureFormatter::new();
    pearls_cli::commands::list::execute(
        None,
        None,
        Vec::new(),
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        &formatter,
    )
    .expect("List failed");

    let captured = formatter.captured.lock().expect("capture lock");
    assert_eq!(captured.len(), 2000, "Expected all pearls in list");
}
