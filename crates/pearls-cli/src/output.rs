// Rust guideline compliant 2026-02-06

//! Output formatting module for the Pearls CLI.
//!
//! This module provides functionality for formatting Pearls data
//! in various output formats (JSON, table, plain text).

use chrono::{DateTime, Duration, Utc};
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
    absolute_time: bool,
}

impl TableFormatter {
    /// Creates a new table formatter.
    ///
    /// # Arguments
    /// * `use_color` - Whether to use colored output
    ///
    /// # Returns
    /// A new TableFormatter instance
    pub fn new(use_color: bool, absolute_time: bool) -> Self {
        Self {
            use_color,
            absolute_time,
        }
    }
}

impl OutputFormatter for TableFormatter {
    fn format_pearl(&self, pearl: &Pearl) -> String {
        let mut output = String::new();

        output.push_str(&format!("ID:          {}\n", pearl.id));
        output.push_str(&format!("Title:       {}\n", pearl.title));
        output.push_str(&format!("Status:      {:?}\n", pearl.status));
        output.push_str(&format!("Priority:    P{}\n", pearl.priority));
        output.push_str(&format!("Author:      {}\n", pearl.author));
        output.push_str(&format!(
            "Created:     {}\n",
            format_timestamp(pearl.created_at, self.absolute_time)
        ));
        output.push_str(&format!(
            "Updated:     {}\n",
            format_timestamp(pearl.updated_at, self.absolute_time)
        ));

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
        builder.push_record(vec!["ID", "Status", "Priority", "Title", "Author", "Deps"]);

        for pearl in pearls {
            let is_archived = pearl
                .metadata
                .get("archived")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let id_display = if is_archived {
                format!("{}*", pearl.id)
            } else {
                pearl.id.clone()
            };
            builder.push_record(vec![
                &id_display,
                &format!("{:?}", pearl.status),
                &format!("P{}", pearl.priority),
                &pearl.title,
                &pearl.author,
                &format_dep_summary(pearl),
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
pub struct PlainFormatter {
    absolute_time: bool,
}

impl PlainFormatter {
    /// Creates a new plain formatter.
    ///
    /// # Arguments
    /// * `absolute_time` - Whether to display absolute timestamps
    ///
    /// # Returns
    /// A new PlainFormatter instance
    pub fn new(absolute_time: bool) -> Self {
        Self { absolute_time }
    }
}

impl OutputFormatter for PlainFormatter {
    fn format_pearl(&self, pearl: &Pearl) -> String {
        let mut output = String::new();

        output.push_str(&format!("{}\n", pearl.id));
        output.push_str(&format!("{}\n", pearl.title));
        output.push_str(&format!("{:?}\n", pearl.status));
        output.push_str(&format!("P{}\n", pearl.priority));
        output.push_str(&format!("{}\n", pearl.author));
        output.push_str(&format!(
            "Updated: {}\n",
            format_timestamp(pearl.updated_at, self.absolute_time)
        ));

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
            let is_archived = pearl
                .metadata
                .get("archived")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let id_display = if is_archived {
                format!("{}*", pearl.id)
            } else {
                pearl.id.clone()
            };
            output.push_str(&format!(
                "{} {:?} P{} {} by {} [{}] ({})\n",
                id_display,
                pearl.status,
                pearl.priority,
                pearl.title,
                pearl.author,
                format_dep_summary(pearl),
                format_timestamp(pearl.updated_at, self.absolute_time)
            ));
        }
        output
    }

    fn format_error(&self, error: &str) -> String {
        format!("Error: {}", error)
    }
}

fn format_timestamp(timestamp: i64, absolute_time: bool) -> String {
    let dt = match DateTime::<Utc>::from_timestamp(timestamp, 0) {
        Some(value) => value,
        None => return timestamp.to_string(),
    };
    if absolute_time {
        return dt.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    }
    let now = Utc::now();
    let delta = now.signed_duration_since(dt);
    if delta < Duration::seconds(0) {
        return dt.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    }
    if delta < Duration::minutes(1) {
        return "just now".to_string();
    }
    if delta < Duration::hours(1) {
        let minutes = delta.num_minutes();
        return format!("{} minutes ago", minutes);
    }
    if delta < Duration::days(1) {
        let hours = delta.num_hours();
        return format!("{} hours ago", hours);
    }
    if delta < Duration::days(30) {
        let days = delta.num_days();
        return format!("{} days ago", days);
    }
    dt.format("%Y-%m-%d").to_string()
}

fn format_dep_summary(pearl: &Pearl) -> String {
    if pearl.deps.is_empty() {
        return "-".to_string();
    }

    let mut blocks = 0usize;
    let mut parent_child = 0usize;
    let mut related = 0usize;
    let mut discovered_from = 0usize;

    for dep in &pearl.deps {
        match dep.dep_type {
            pearls_core::DepType::Blocks => blocks += 1,
            pearls_core::DepType::ParentChild => parent_child += 1,
            pearls_core::DepType::Related => related += 1,
            pearls_core::DepType::DiscoveredFrom => discovered_from += 1,
        }
    }

    let mut parts = Vec::new();
    if blocks > 0 {
        parts.push(format!("blocks:{}", blocks));
    }
    if parent_child > 0 {
        parts.push(format!("parent_child:{}", parent_child));
    }
    if related > 0 {
        parts.push(format!("related:{}", related));
    }
    if discovered_from > 0 {
        parts.push(format!("discovered_from:{}", discovered_from));
    }

    parts.join(", ")
}

/// Factory function to create an appropriate formatter.
///
/// # Arguments
/// * `format` - The desired output format ("json", "table", or "plain")
/// * `use_color` - Whether to use colored output (ignored for JSON)
///
/// # Returns
/// A boxed OutputFormatter instance
pub fn create_formatter(
    format: &str,
    use_color: bool,
    absolute_time: bool,
) -> Box<dyn OutputFormatter> {
    match format {
        "json" => Box::new(JsonFormatter),
        "table" => Box::new(TableFormatter::new(use_color, absolute_time)),
        "plain" => Box::new(PlainFormatter::new(absolute_time)),
        _ => Box::new(TableFormatter::new(use_color, absolute_time)),
    }
}
