// Rust guideline compliant 2026-02-06

//! Pearls CLI library.
//!
//! This library exposes the CLI modules for use in tests and external code.

pub mod commands;
pub mod output;
pub mod terminal;

pub use output::{create_formatter, OutputFormatter};
pub use terminal::{get_terminal_width, should_use_color, wrap_text};
