// Rust guideline compliant 2026-02-06

//! Implementation of the `prl import` command.
//!
//! Imports Pearls data from Beads JSONL format.

use anyhow::Result;
use pearls_core::{Pearl, Storage};
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::progress::ProgressReporter;

/// Imports Pearls from a Beads JSONL file.
///
/// # Arguments
///
/// * `path` - Path to the Beads JSONL file
///
/// # Returns
///
/// Ok if import succeeds, Err otherwise.
///
/// # Errors
///
/// Returns an error if:
/// - The repository is not initialized
/// - The source file cannot be read
/// - The destination file cannot be written
pub fn import_beads(path: String) -> Result<()> {
    let pearls_dir = Path::new(".pearls");
    if !pearls_dir.exists() {
        anyhow::bail!("Pearls repository not initialized. Run 'prl init' first.");
    }

    let beads_path = PathBuf::from(path);
    if !beads_path.exists() {
        anyhow::bail!("Beads file not found: {}", beads_path.display());
    }

    let file = std::fs::File::open(&beads_path)?;
    let reader = std::io::BufReader::with_capacity(64 * 1024, file);
    let mut pearls = Vec::new();
    let mut skipped = 0usize;
    let mut processed = 0usize;
    let progress = ProgressReporter::new("Importing", None, 1000);

    for (idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        processed += 1;
        progress.report(processed);
        match serde_json::from_str::<Pearl>(&line) {
            Ok(pearl) => {
                if let Err(err) = pearl.validate() {
                    skipped += 1;
                    eprintln!(
                        "Warning: Skipping invalid Pearl on line {}: {}",
                        idx + 1,
                        err
                    );
                } else {
                    pearls.push(pearl);
                }
            }
            Err(err) => {
                skipped += 1;
                eprintln!(
                    "Warning: Skipping invalid JSON on line {}: {}",
                    idx + 1,
                    err
                );
            }
        }
    }
    progress.finish(processed);

    if pearls.is_empty() {
        anyhow::bail!("No valid Pearls found in Beads file.");
    }

    let mut storage = Storage::new(pearls_dir.join("issues.jsonl"))?;
    storage.save_all(&pearls)?;

    println!("Imported Pearls: {}", pearls.len());
    if skipped > 0 {
        println!("Skipped entries: {}", skipped);
    }

    Ok(())
}
