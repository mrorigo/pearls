// Rust guideline compliant 2026-02-09

//! Timestamp helpers for Pearls.

use crate::error::{AppError, Result};
use std::time::{SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds.
///
/// # Returns
///
/// The current Unix timestamp.
///
/// # Errors
///
/// Returns an error if the system clock is before the Unix epoch.
pub fn unix_timestamp() -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| AppError::InvalidInput(format!("System time before epoch: {err}")))?
        .as_secs() as i64;
    Ok(now)
}
