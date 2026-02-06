// Rust guideline compliant 2026-02-06

//! Implementation of the `prl doctor` command.
//!
//! Validates JSONL syntax, schema compliance, graph integrity, and common issues.

use anyhow::Result;
use pearls_core::{IssueGraph, Pearl, Status, Storage};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Severity {
    Error,
    Warning,
    Info,
}

struct Finding {
    severity: Severity,
    message: String,
}

/// Executes the doctor command.
///
/// # Arguments
///
/// * `fix` - Whether to attempt automatic repairs
///
/// # Returns
///
/// Ok if validation completes successfully, Err if errors are found.
///
/// # Errors
///
/// Returns an error if:
/// - The repository is not initialized
/// - The issues file cannot be read
pub fn execute(fix: bool) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let issues_path = pearls_dir.join("issues.jsonl");
    let mut findings = Vec::new();
    let mut pearls = Vec::new();
    let mut invalid_lines = 0usize;
    let mut has_cycle_error = false;
    let mut has_closed_blocked_error = false;

    if issues_path.exists() {
        let content = std::fs::read_to_string(&issues_path)?;
        for (idx, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Pearl>(line) {
                Ok(pearl) => {
                    if let Err(err) = pearl.validate() {
                        findings.push(Finding {
                            severity: Severity::Error,
                            message: format!("Line {}: {}", idx + 1, err),
                        });
                    }
                    pearls.push(pearl);
                }
                Err(err) => {
                    invalid_lines += 1;
                    findings.push(Finding {
                        severity: Severity::Error,
                        message: format!("Line {}: Invalid JSON ({})", idx + 1, err),
                    });
                }
            }
        }
    }

    let (deduped, duplicate_ids) = dedupe_pearls(&pearls);
    if !duplicate_ids.is_empty() {
        findings.push(Finding {
            severity: Severity::Error,
            message: format!("Duplicate Pearl IDs detected: {}", duplicate_ids.join(", ")),
        });
    }

    let orphaned = find_orphaned_deps(&deduped);
    for (pearl_id, target_id) in &orphaned {
        findings.push(Finding {
            severity: Severity::Warning,
            message: format!(
                "Orphaned dependency: {} references missing {}",
                pearl_id, target_id
            ),
        });
    }

    match IssueGraph::from_pearls(deduped.clone()) {
        Ok(graph) => {
            for pearl in &deduped {
                if pearl.status == Status::Blocked && !graph.is_blocked(&pearl.id) {
                    findings.push(Finding {
                        severity: Severity::Warning,
                        message: format!(
                            "Pearl {} is marked blocked but has no open blockers",
                            pearl.id
                        ),
                    });
                }
                if pearl.status == Status::Closed && graph.is_blocked(&pearl.id) {
                    has_closed_blocked_error = true;
                    findings.push(Finding {
                        severity: Severity::Error,
                        message: format!(
                            "Pearl {} is closed but still blocked by open dependencies",
                            pearl.id
                        ),
                    });
                }
            }
        }
        Err(err) => {
            has_cycle_error = true;
            findings.push(Finding {
                severity: Severity::Error,
                message: format!("Cycle detected: {}", err),
            });
        }
    }

    if fix {
        let mut fixed = deduped.clone();
        let removed = remove_orphaned_deps(&mut fixed);
        let removed_dupes = duplicate_ids.len();
        let removed_invalid = invalid_lines;

        let mut storage = Storage::new(issues_path)?;
        storage.save_all(&fixed)?;

        findings.push(Finding {
            severity: Severity::Info,
            message: format!(
                "Fix applied: removed {} orphaned deps, {} duplicate IDs, {} invalid lines",
                removed, removed_dupes, removed_invalid
            ),
        });
    }

    report_findings(&findings);

    if findings.iter().any(|f| f.severity == Severity::Error) {
        if fix && !has_cycle_error && !has_closed_blocked_error {
            return Ok(());
        }
        anyhow::bail!("Doctor found errors. Run with --fix to attempt repairs.");
    }

    Ok(())
}

fn report_findings(findings: &[Finding]) {
    if findings.is_empty() {
        println!("Doctor: no issues found.");
        return;
    }

    println!("Doctor findings:");
    for finding in findings {
        let label = match finding.severity {
            Severity::Error => "ERROR",
            Severity::Warning => "WARN",
            Severity::Info => "INFO",
        };
        println!("[{}] {}", label, finding.message);
    }
}

fn dedupe_pearls(pearls: &[Pearl]) -> (Vec<Pearl>, Vec<String>) {
    let mut seen = HashSet::new();
    let mut dupes = Vec::new();
    let mut unique = Vec::new();

    for pearl in pearls {
        if seen.contains(&pearl.id) {
            dupes.push(pearl.id.clone());
        } else {
            seen.insert(pearl.id.clone());
            unique.push(pearl.clone());
        }
    }

    (unique, dupes)
}

fn find_orphaned_deps(pearls: &[Pearl]) -> Vec<(String, String)> {
    let ids: HashSet<String> = pearls.iter().map(|pearl| pearl.id.clone()).collect();
    let mut orphaned = Vec::new();

    for pearl in pearls {
        for dep in &pearl.deps {
            if !ids.contains(&dep.target_id) {
                orphaned.push((pearl.id.clone(), dep.target_id.clone()));
            }
        }
    }

    orphaned
}

fn remove_orphaned_deps(pearls: &mut [Pearl]) -> usize {
    let ids: HashSet<String> = pearls.iter().map(|pearl| pearl.id.clone()).collect();
    let mut removed = 0usize;

    for pearl in pearls {
        let before = pearl.deps.len();
        pearl.deps.retain(|dep| ids.contains(&dep.target_id));
        removed += before - pearl.deps.len();
    }

    removed
}
