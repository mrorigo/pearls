// Rust guideline compliant 2026-02-06

//! Implementation of the `prl sync` command.
//!
//! Synchronizes the repository with the remote using pull --rebase semantics.

use anyhow::Result;
use git2::{Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository, Signature};
use pearls_core::{IssueGraph, Storage};
use std::path::Path;

/// Syncs the repository with the remote using pull --rebase semantics.
///
/// # Arguments
///
/// * `dry_run` - Whether to preview actions without applying
///
/// # Returns
///
/// Ok if sync completes successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The repository is not initialized
/// - Git operations fail
/// - Integrity checks fail after merge
pub fn execute(dry_run: bool) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    if dry_run {
        println!("Sync dry-run: would fetch, rebase, validate, and push.");
        return Ok(());
    }

    let repo = Repository::discover(".")?;
    let branch_name = current_branch_name(&repo)?;

    let mut attempts = 0;
    loop {
        attempts += 1;
        fetch_origin(&repo)?;
        rebase_onto_upstream(&repo, &branch_name)?;
        run_integrity_checks(pearls_dir)?;
        if push_origin(&repo, &branch_name).is_ok() {
            break;
        }
        if attempts >= 2 {
            anyhow::bail!("Sync failed: push rejected after retry.");
        }
    }

    println!("✓ Sync completed.");
    Ok(())
}

fn current_branch_name(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    if !head.is_branch() {
        anyhow::bail!("HEAD is not a branch.");
    }
    let name = head
        .shorthand()
        .ok_or_else(|| anyhow::anyhow!("Missing branch name"))?;
    Ok(name.to_string())
}

fn fetch_origin(repo: &Repository) -> Result<()> {
    let mut remote = repo.find_remote("origin")?;
    let mut callbacks = RemoteCallbacks::new();
    let config = repo.config()?;
    callbacks.credentials(move |url, username_from_url, _allowed| {
        Cred::credential_helper(&config, url, username_from_url)
            .or_else(|_| Cred::default())
            .or_else(|_| Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")))
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    remote.fetch(&[] as &[&str], Some(&mut fetch_options), None)?;
    Ok(())
}

fn rebase_onto_upstream(repo: &Repository, branch_name: &str) -> Result<()> {
    let branch_ref = format!("refs/heads/{}", branch_name);
    let upstream_ref = format!("refs/remotes/origin/{}", branch_name);

    let branch = repo.find_reference(&branch_ref)?;
    let upstream = repo.find_reference(&upstream_ref)?;

    let branch_commit = repo.reference_to_annotated_commit(&branch)?;
    let upstream_commit = repo.reference_to_annotated_commit(&upstream)?;

    let mut rebase = repo.rebase(Some(&branch_commit), Some(&upstream_commit), None, None)?;
    let signature = repo
        .signature()
        .or_else(|_| Signature::now("pearls", "pearls@local"))?;

    while let Some(operation) = rebase.next() {
        operation?;
        let index = rebase.inmemory_index()?;
        if index.has_conflicts() {
            anyhow::bail!("Rebase conflict detected. Resolve manually and retry.");
        }
        rebase.commit(None, &signature, None)?;
    }

    rebase.finish(Some(&signature))?;
    Ok(())
}

fn push_origin(repo: &Repository, branch_name: &str) -> Result<()> {
    let mut remote = repo.find_remote("origin")?;
    let mut callbacks = RemoteCallbacks::new();
    let config = repo.config()?;
    callbacks.credentials(move |url, username_from_url, _allowed| {
        Cred::credential_helper(&config, url, username_from_url)
            .or_else(|_| Cred::default())
            .or_else(|_| Cred::ssh_key_from_agent(username_from_url.unwrap_or("git")))
    });
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);
    remote.push(&[refspec], Some(&mut push_options))?;
    Ok(())
}

fn run_integrity_checks(pearls_dir: &Path) -> Result<()> {
    let storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;
    let _graph = IssueGraph::from_pearls(pearls.clone())?;
    let ids: std::collections::HashSet<String> = pearls.iter().map(|p| p.id.clone()).collect();
    for pearl in &pearls {
        for dep in &pearl.deps {
            if !ids.contains(&dep.target_id) {
                anyhow::bail!(
                    "Integrity check failed: orphaned dependency {} -> {}",
                    pearl.id,
                    dep.target_id
                );
            }
        }
    }
    Ok(())
}
