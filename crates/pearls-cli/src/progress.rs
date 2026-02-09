// Rust guideline compliant 2026-02-06

//! Progress reporting utilities for long-running CLI operations.

/// Simple progress reporter that emits periodic updates.
pub struct ProgressReporter {
    label: String,
    total: Option<usize>,
    interval: usize,
}

impl ProgressReporter {
    /// Creates a new progress reporter.
    ///
    /// # Arguments
    ///
    /// * `label` - Label to include in progress messages
    /// * `total` - Optional total count for the operation
    /// * `interval` - Report every N items (minimum 1)
    ///
    /// # Returns
    ///
    /// A new ProgressReporter instance.
    pub fn new(label: &str, total: Option<usize>, interval: usize) -> Self {
        Self {
            label: label.to_string(),
            total,
            interval: interval.max(1),
        }
    }

    /// Reports progress at the configured interval.
    ///
    /// # Arguments
    ///
    /// * `current` - Current item count processed (1-based)
    pub fn report(&self, current: usize) {
        if !current.is_multiple_of(self.interval) {
            return;
        }

        match self.total {
            Some(total) => eprintln!("{}: {} / {}", self.label, current, total),
            None => eprintln!("{}: {}", self.label, current),
        }
    }

    /// Reports completion for the operation.
    ///
    /// # Arguments
    ///
    /// * `current` - Final item count processed
    pub fn finish(&self, current: usize) {
        match self.total {
            Some(total) => eprintln!("{}: {} / {} complete", self.label, current, total),
            None => eprintln!("{}: {} complete", self.label, current),
        }
    }
}
