// Rust guideline compliant 2026-02-06

//! Implementation of the `prl compact` command.
//!
//! Archives closed Pearls older than a threshold to `.pearls/archive.jsonl`.

use anyhow::Result;
use chrono::{Duration, Utc};
use pearls_core::{Config, Pearl, Status, Storage};
use std::collections::HashMap;
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

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    let pearls = storage.load_all()?;

    let (archive_candidates, remaining): (Vec<Pearl>, Vec<Pearl>) = pearls
        .into_iter()
        .partition(|pearl| pearl.status == Status::Closed && pearl.updated_at <= cutoff_ts);

    println!(
        "Compaction threshold: {} days (cutoff timestamp {})",
        threshold_days, cutoff_ts
    );
    println!(
        "Closed Pearls eligible for archive: {}",
        archive_candidates.len()
    );

    if dry_run {
        if archive_candidates.is_empty() {
            println!("Dry run: no Pearls would be archived.");
        } else {
            println!("Dry run: Pearls to archive:");
            for pearl in &archive_candidates {
                println!("- {} ({})", pearl.title, pearl.id);
            }
        }
        return Ok(());
    }

    if archive_candidates.is_empty() {
        println!("No Pearls to archive.");
        return Ok(());
    }

    let archive_path = pearls_dir.join("archive.jsonl");
    let mut archive_storage = Storage::new(archive_path.clone())?;
    let archive_pearls = if archive_path.exists() {
        archive_storage.load_all()?
    } else {
        Vec::new()
    };

    let mut archive_map: HashMap<String, Pearl> = archive_pearls
        .into_iter()
        .map(|pearl| (pearl.id.clone(), pearl))
        .collect();

    for pearl in archive_candidates {
        archive_map.entry(pearl.id.clone()).or_insert(pearl);
    }

    let mut merged_archive: Vec<Pearl> = archive_map.into_values().collect();
    merged_archive.sort_by(|a, b| a.id.cmp(&b.id));

    archive_storage.save_all(&merged_archive)?;
    storage.save_all(&remaining)?;

    println!("Archived Pearls: {}", merged_archive.len());
    println!("Active Pearls remaining: {}", remaining.len());

    Ok(())
}
