// Rust guideline compliant 2026-02-06

//! Implementation of the `prl status` command.
//!
//! Displays project health checks and a "land the plane" checklist.

use anyhow::Result;
use chrono::{Duration, Utc};
use git2::{BranchType, Repository, StatusOptions};
use pearls_core::{IssueGraph, Status, Storage};
use std::path::Path;

/// Executes the status command.
///
/// # Arguments
///
/// * `detailed` - Whether to show a detailed checklist
///
/// # Returns
///
/// Ok if status is displayed successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The repository is not initialized
/// - Git repository discovery fails
/// - The issues file cannot be read
pub fn execute(detailed: bool) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;
    let graph = IssueGraph::from_pearls(pearls.clone())?;

    let git_status = collect_git_status()?;
    let p0_open = pearls
        .iter()
        .filter(|pearl| pearl.priority == 0 && pearl.status != Status::Closed)
        .count();
    let blocked_count = pearls
        .iter()
        .filter(|pearl| pearl.status != Status::Closed && graph.is_blocked(&pearl.id))
        .count();

    let recent_updates = collect_recent_updates(&pearls);

    println!("Pearls Status");
    println!("-------------");
    println!(
        "Git working tree: {}",
        if git_status.is_clean {
            "clean"
        } else {
            "dirty"
        }
    );
    println!("Open P0 Pearls: {}", p0_open);
    println!("Blocked Pearls: {}", blocked_count);
    println!(
        "Remote sync: {}",
        git_status.sync_status.as_deref().unwrap_or("unknown")
    );
    println!(
        "Tests: {}",
        git_status.tests_status.as_deref().unwrap_or("unknown")
    );

    if detailed {
        println!();
        println!("Checklist");
        println!("---------");
        print_check("Working tree clean", git_status.is_clean);
        print_check("No open P0 Pearls", p0_open == 0);
        print_check("No blocked Pearls", blocked_count == 0);
        print_check(
            "Branch is synced with remote",
            matches!(git_status.sync_status.as_deref(), Some("in sync")),
        );
    }

    println!();
    println!("Recent Work (last 24h)");
    println!("----------------------");
    if recent_updates.is_empty() {
        println!("No Pearls updated in the last 24 hours.");
    } else {
        for pearl in recent_updates {
            println!("- {} ({})", pearl.title, pearl.id);
        }
    }

    Ok(())
}

fn print_check(label: &str, ok: bool) {
    let marker = if ok { "✓" } else { "✗" };
    println!("{} {}", marker, label);
}

struct GitStatusSummary {
    is_clean: bool,
    sync_status: Option<String>,
    tests_status: Option<String>,
}

fn collect_git_status() -> Result<GitStatusSummary> {
    let repo = Repository::discover(".")?;
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let is_clean = statuses.is_empty();

    let sync_status = resolve_sync_status(&repo);

    Ok(GitStatusSummary {
        is_clean,
        sync_status,
        tests_status: None,
    })
}

fn resolve_sync_status(repo: &Repository) -> Option<String> {
    let head = repo.head().ok()?;
    let head_oid = head.target()?;
    let branch_name = head.shorthand()?;
    let branch = repo.find_branch(branch_name, BranchType::Local).ok()?;
    let upstream = branch.upstream().ok()?;
    let upstream_oid = upstream.get().target()?;
    let (ahead, behind) = repo.graph_ahead_behind(head_oid, upstream_oid).ok()?;

    if ahead == 0 && behind == 0 {
        Some("in sync".to_string())
    } else if ahead > 0 && behind == 0 {
        Some(format!("ahead by {}", ahead))
    } else if ahead == 0 && behind > 0 {
        Some(format!("behind by {}", behind))
    } else {
        Some(format!("diverged (ahead {}, behind {})", ahead, behind))
    }
}

fn collect_recent_updates(pearls: &[pearls_core::Pearl]) -> Vec<pearls_core::Pearl> {
    let cutoff = Utc::now() - Duration::hours(24);
    pearls
        .iter()
        .filter(|pearl| {
            let ts = chrono::DateTime::<Utc>::from_timestamp(pearl.updated_at, 0);
            ts.map(|t| t >= cutoff).unwrap_or(false)
        })
        .cloned()
        .collect()
}
