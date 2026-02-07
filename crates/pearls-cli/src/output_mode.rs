// Rust guideline compliant 2026-02-07

//! Process-wide output mode settings for CLI commands.

use std::sync::atomic::{AtomicBool, Ordering};

static JSON_OUTPUT_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enables or disables JSON output mode for the current process.
pub fn set_json_output(enabled: bool) {
    JSON_OUTPUT_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Returns whether JSON output mode is enabled.
pub fn is_json_output() -> bool {
    JSON_OUTPUT_ENABLED.load(Ordering::Relaxed)
}
