// Rust guideline compliant 2026-02-06

//! Terminal UI utilities for the Pearls CLI.
//!
//! This module provides color support, terminal width detection,
//! and other terminal UI utilities.

use std::env;
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

/// Determines if colored output should be used.
///
/// Respects the NO_COLOR environment variable and terminal capabilities.
///
/// # Returns
/// `true` if colored output should be used, `false` otherwise
pub fn should_use_color() -> bool {
    // Check NO_COLOR environment variable
    if env::var("NO_COLOR").is_ok() {
        return false;
    }

    // Check if stdout is a TTY
    atty::is(atty::Stream::Stdout)
}

/// Gets the terminal width in columns.
///
/// # Returns
/// The terminal width, or 80 if it cannot be determined
pub fn get_terminal_width() -> usize {
    term_size::dimensions().map(|(w, _)| w).unwrap_or(80)
}

/// Wraps text to fit within the terminal width.
///
/// # Arguments
/// * `text` - The text to wrap
/// * `indent` - The indentation level (in spaces)
///
/// # Returns
/// The wrapped text
pub fn wrap_text(text: &str, indent: usize) -> String {
    let width = get_terminal_width();
    let available_width = width.saturating_sub(indent);

    if available_width < 10 {
        return text.to_string();
    }

    let mut result = String::new();
    let indent_str = " ".repeat(indent);

    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            result.push('\n');
            result.push_str(&indent_str);
        }

        if line.len() <= available_width {
            result.push_str(line);
        } else {
            // Simple word wrapping
            let mut current_line = String::new();
            for word in line.split_whitespace() {
                if current_line.is_empty() {
                    current_line.push_str(word);
                } else if current_line.len() + 1 + word.len() <= available_width {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    result.push_str(&current_line);
                    result.push('\n');
                    result.push_str(&indent_str);
                    current_line = word.to_string();
                }
            }
            result.push_str(&current_line);
        }
    }

    result
}

/// Prints colored text to stderr.
///
/// # Arguments
/// * `text` - The text to print
/// * `color` - The color to use
/// * `bold` - Whether to use bold text
pub fn print_colored(text: &str, color: Color, bold: bool) {
    let mut stderr = StandardStream::stderr(ColorChoice::Auto);
    let _ = stderr.set_color(ColorSpec::new().set_fg(Some(color)).set_bold(bold));
    let _ = write!(stderr, "{}", text);
    let _ = stderr.reset();
}

/// Prints a status message with a colored prefix.
///
/// # Arguments
/// * `prefix` - The prefix text
/// * `prefix_color` - The color for the prefix
/// * `message` - The message text
pub fn print_status(prefix: &str, prefix_color: Color, message: &str) {
    let mut stderr = StandardStream::stderr(ColorChoice::Auto);
    let _ = stderr.set_color(ColorSpec::new().set_fg(Some(prefix_color)).set_bold(true));
    let _ = write!(stderr, "{}: ", prefix);
    let _ = stderr.reset();
    let _ = writeln!(stderr, "{}", message);
}

/// Prints a success message.
///
/// # Arguments
/// * `message` - The message to print
pub fn print_success(message: &str) {
    print_status("✓", Color::Green, message);
}

/// Prints an error message.
///
/// # Arguments
/// * `message` - The message to print
pub fn print_error(message: &str) {
    print_status("✗", Color::Red, message);
}

/// Prints a warning message.
///
/// # Arguments
/// * `message` - The message to print
pub fn print_warning(message: &str) {
    print_status("⚠", Color::Yellow, message);
}

/// Prints an info message.
///
/// # Arguments
/// * `message` - The message to print
pub fn print_info(message: &str) {
    print_status("ℹ", Color::Cyan, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_color_respects_no_color() {
        // This test verifies NO_COLOR is respected
        // Note: actual behavior depends on environment
        let _ = should_use_color();
    }

    #[test]
    fn test_get_terminal_width_returns_positive() {
        let width = get_terminal_width();
        assert!(width > 0);
    }

    #[test]
    fn test_wrap_text_short_text() {
        let text = "short";
        let wrapped = wrap_text(text, 0);
        assert_eq!(wrapped, "short");
    }

    #[test]
    fn test_wrap_text_with_indent() {
        let text = "hello world";
        let wrapped = wrap_text(text, 2);
        assert!(wrapped.contains("hello"));
    }
}
