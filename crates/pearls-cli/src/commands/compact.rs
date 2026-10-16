// Rust guideline compliant 2026-02-06

//! Implementation of the `prl compact` command.
//!
//! Archives closed Pearls older than a threshold to `.pearls/archive.jsonl`.

use crate::output_mode::is_json_output;
use anyhow::Result;
use chrono::{Duration, Utc};
use pearls_app::service::GraphService;
use pearls_core::{Config, Status};
use std::path::Path;

/// Compacts closed Pearls older than the configured threshold.
///
/// # Arguments
///
/// * `threshold_days` - Optional override for compaction threshold
/// * `dry_run` - Whether to preview changes without applying
///
/// # Returns
///
/// Ok if compaction completed successfully, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The repository is not initialized
/// - The issues file cannot be read
/// - Archive file cannot be written
pub fn execute(threshold_days: Option<u32>, dry_run: bool) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let config = Config::load(pearls_dir)?;
    let threshold_days = threshold_days.unwrap_or(config.compact_threshold_days);
    let cutoff = Utc::now() - Duration::days(i64::from(threshold_days));
    let cutoff_ts = cutoff.timestamp();

    let service = GraphService::open(".")?;
    if dry_run {
        let snapshot = service.snapshot()?;
        let eligible: Vec<_> = snapshot
            .data
            .active
            .into_iter()
            .filter(|task| task.status == Status::Closed && task.updated_at <= cutoff_ts)
            .collect();
        if is_json_output() {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "status":"ok","action":"compact","dry_run":true,"threshold_days":threshold_days,
                    "cutoff_timestamp":cutoff_ts,"eligible":eligible
                }))?
            );
        } else {
            println!("Closed Pearls eligible for archive: {}", eligible.len());
            if eligible.is_empty() {
                println!("Dry run: no Pearls would be archived.");
            } else {
                for task in eligible {
                    println!("- {} ({})", task.title, task.id);
                }
            }
        }
        return Ok(());
    }
    let result = service.compact(cutoff_ts)?;
    if is_json_output() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "status":"ok","action":"compact","threshold_days":threshold_days,
                "cutoff_timestamp":cutoff_ts,"archived_total":result.data.archived.len(),
                "active_remaining":result.data.active.len()
            }))?
        );
    } else {
        println!("Archived Pearls: {}", result.data.archived.len());
        println!("Active Pearls remaining: {}", result.data.active.len());
    }
    Ok(())
}
