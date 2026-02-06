// Rust guideline compliant 2026-02-06

//! Output formatting module for the Pearls CLI.
//!
//! This module provides functionality for formatting Pearls data
//! in various output formats (JSON, table, plain text).

use pearls_core::Pearl;
use serde_json::json;
use std::io::Write;
use tabled::{builder::Builder, settings::Style};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

/// Output formatter trait.
///
/// Defines the interface for formatting Pearls data in different output formats.
pub trait OutputFormatter {
    /// Formats a single Pearl for display.
    ///
    /// # Arguments
    /// * `pearl` - The Pearl to format
    ///
    /// # Returns
    /// A formatted string representation of the Pearl
    fn format_pearl(&self, pearl: &Pearl) -> String;

    /// Formats a list of Pearls for display.
    ///
    /// # Arguments
    /// * `pearls` - The Pearls to format
    ///
    /// # Returns
    /// A formatted string representation of the Pearl list
    fn format_list(&self, pearls: &[Pearl]) -> String;

    /// Formats an error message for display.
    ///
    /// # Arguments
    /// * `error` - The error message to format
    ///
    /// # Returns
    /// A formatted error string
    fn format_error(&self, error: &str) -> String;
}

/// JSON output formatter.
///
/// Formats Pearls as valid JSON for machine consumption.
pub struct JsonFormatter;

impl OutputFormatter for JsonFormatter {
    fn format_pearl(&self, pearl: &Pearl) -> String {
        serde_json::to_string_pretty(pearl)
            .unwrap_or_else(|_| json!({ "error": "Failed to serialize Pearl" }).to_string())
    }

    fn format_list(&self, pearls: &[Pearl]) -> String {
        let output = json!({
            "pearls": pearls,
            "total": pearls.len(),
        });
        serde_json::to_string_pretty(&output)
            .unwrap_or_else(|_| json!({ "error": "Failed to serialize Pearl list" }).to_string())
    }

    fn format_error(&self, error: &str) -> String {
        json!({ "error": error }).to_string()
    }
}

/// Table output formatter.
///
/// Formats Pearls as human-readable tables with colors and alignment.
pub struct TableFormatter {
    use_color: bool,
}

impl TableFormatter {
    /// Creates a new table formatter.
    ///
    /// # Arguments
    /// * `use_color` - Whether to use colored output
    ///
    /// # Returns
    /// A new TableFormatter instance
    pub fn new(use_color: bool) -> Self {
        Self { use_color }
    }
}

impl OutputFormatter for TableFormatter {
    fn format_pearl(&self, pearl: &Pearl) -> String {
        let mut output = String::new();

        output.push_str(&format!("ID:          {}\n", pearl.id));
        output.push_str(&format!("Title:       {}\n", pearl.title));
        output.push_str(&format!("Status:      {:?}\n", pearl.status));
        output.push_str(&format!("Priority:    {}\n", pearl.priority));
        output.push_str(&format!("Author:      {}\n", pearl.author));
        output.push_str(&format!("Created:     {}\n", pearl.created_at));
        output.push_str(&format!("Updated:     {}\n", pearl.updated_at));

        if !pearl.description.is_empty() {
            output.push_str(&format!("Description: {}\n", pearl.description));
        }

        if !pearl.labels.is_empty() {
            output.push_str(&format!("Labels:      {}\n", pearl.labels.join(", ")));
        }

        if !pearl.deps.is_empty() {
            output.push_str(&format!("Dependencies: {}\n", pearl.deps.len()));
        }

        output
    }

    fn format_list(&self, pearls: &[Pearl]) -> String {
        if pearls.is_empty() {
            return "No Pearls found.".to_string();
        }

        let mut builder = Builder::default();
        builder.push_record(vec!["ID", "Status", "Priority", "Title", "Author"]);

        for pearl in pearls {
            builder.push_record(vec![
                &pearl.id,
                &format!("{:?}", pearl.status),
                &pearl.priority.to_string(),
                &pearl.title,
                &pearl.author,
            ]);
        }

        let mut table = builder.build();
        table.with(Style::modern());

        table.to_string()
    }

    fn format_error(&self, error: &str) -> String {
        if self.use_color {
            let mut output = Vec::new();
            let mut stderr = StandardStream::stderr(ColorChoice::Auto);
            let _ = stderr.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true));
            let _ = write!(output, "Error: ");
            let _ = stderr.reset();
            let _ = write!(output, "{}", error);
            String::from_utf8_lossy(&output).to_string()
        } else {
            format!("Error: {}", error)
        }
    }
}

/// Plain text output formatter.
///
/// Formats Pearls as simple plain text without colors or tables.
pub struct PlainFormatter;

impl OutputFormatter for PlainFormatter {
    fn format_pearl(&self, pearl: &Pearl) -> String {
        let mut output = String::new();

        output.push_str(&format!("{}\n", pearl.id));
        output.push_str(&format!("{}\n", pearl.title));
        output.push_str(&format!("{:?}\n", pearl.status));
        output.push_str(&format!("{}\n", pearl.priority));
        output.push_str(&format!("{}\n", pearl.author));

        if !pearl.description.is_empty() {
            output.push_str(&format!("{}\n", pearl.description));
        }

        output
    }

    fn format_list(&self, pearls: &[Pearl]) -> String {
        if pearls.is_empty() {
            return "No Pearls found.".to_string();
        }

        let mut output = String::new();
        for pearl in pearls {
            output.push_str(&format!(
                "{} {} {} {}\n",
                pearl.id, pearl.status as u8, pearl.priority, pearl.title
            ));
        }
        output
    }

    fn format_error(&self, error: &str) -> String {
        format!("Error: {}", error)
    }
}

/// Factory function to create an appropriate formatter.
///
/// # Arguments
/// * `format` - The desired output format ("json", "table", or "plain")
/// * `use_color` - Whether to use colored output (ignored for JSON)
///
/// # Returns
/// A boxed OutputFormatter instance
pub fn create_formatter(format: &str, use_color: bool) -> Box<dyn OutputFormatter> {
    match format {
        "json" => Box::new(JsonFormatter),
        "table" => Box::new(TableFormatter::new(use_color)),
        "plain" => Box::new(PlainFormatter),
        _ => Box::new(TableFormatter::new(use_color)),
    }
}
