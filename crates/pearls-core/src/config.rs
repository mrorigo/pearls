// Rust guideline compliant 2026-02-06

//! Configuration management for Pearls.

use crate::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Output format for command results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// JSON output format.
    Json,
    /// Human-readable table format.
    #[default]
    Table,
    /// Plain text format.
    Plain,
}

/// Configuration for Pearls behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default priority for new Pearls (0-4).
    #[serde(default = "default_priority")]
    pub default_priority: u8,

    /// Number of days before closed Pearls are archived.
    #[serde(default = "default_compact_threshold")]
    pub compact_threshold_days: u32,

    /// Whether to use index file for large repositories.
    #[serde(default)]
    pub use_index: bool,

    /// Default output format for commands.
    #[serde(default)]
    pub output_format: OutputFormat,

    /// Whether to auto-close Pearls on commit with "Fixes (prl-XXXXXX)" pattern.
    #[serde(default)]
    pub auto_close_on_commit: bool,
}

/// Default priority value (medium).
fn default_priority() -> u8 {
    2
}

/// Default compaction threshold in days.
fn default_compact_threshold() -> u32 {
    30
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_priority: default_priority(),
            compact_threshold_days: default_compact_threshold(),
            use_index: false,
            output_format: OutputFormat::default(),
            auto_close_on_commit: false,
        }
    }
}

impl Config {
    /// Loads configuration from file and environment variables.
    ///
    /// Configuration is loaded in the following order (later overrides earlier):
    /// 1. Default values
    /// 2. Configuration file at `.pearls/config.toml`
    /// 3. Environment variables with `PEARLS_` prefix
    ///
    /// # Arguments
    ///
    /// * `pearls_dir` - Path to the `.pearls` directory
    ///
    /// # Returns
    ///
    /// A Config struct with values from file and environment variables applied.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Configuration file exists but cannot be read
    /// - Configuration file contains invalid TOML
    /// - Configuration values fail validation
    pub fn load(pearls_dir: &Path) -> Result<Self> {
        let mut config = Self::default();

        // Try to load from config file
        let config_path = pearls_dir.join("config.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let file_config: Config = toml::from_str(&content)
                .map_err(|e| crate::Error::InvalidPearl(format!("Invalid config file: {}", e)))?;
            config = file_config;
        }

        // Apply environment variable overrides
        config.apply_env_overrides()?;

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Applies environment variable overrides to the configuration.
    ///
    /// Supported environment variables:
    /// - `PEARLS_DEFAULT_PRIORITY` - Default priority (0-4)
    /// - `PEARLS_COMPACT_THRESHOLD_DAYS` - Compaction threshold in days
    /// - `PEARLS_USE_INDEX` - Whether to use index file (true/false)
    /// - `PEARLS_OUTPUT_FORMAT` - Output format (json/table/plain)
    /// - `PEARLS_AUTO_CLOSE_ON_COMMIT` - Auto-close on commit (true/false)
    ///
    /// # Returns
    ///
    /// Ok if all environment variables are valid, Err otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if environment variable values are invalid.
    fn apply_env_overrides(&mut self) -> Result<()> {
        if let Ok(val) = std::env::var("PEARLS_DEFAULT_PRIORITY") {
            self.default_priority = val.parse().map_err(|_| {
                crate::Error::InvalidPearl(
                    "PEARLS_DEFAULT_PRIORITY must be a number 0-4".to_string(),
                )
            })?;
        }

        if let Ok(val) = std::env::var("PEARLS_COMPACT_THRESHOLD_DAYS") {
            self.compact_threshold_days = val.parse().map_err(|_| {
                crate::Error::InvalidPearl(
                    "PEARLS_COMPACT_THRESHOLD_DAYS must be a positive number".to_string(),
                )
            })?;
        }

        if let Ok(val) = std::env::var("PEARLS_USE_INDEX") {
            self.use_index = val.parse().map_err(|_| {
                crate::Error::InvalidPearl("PEARLS_USE_INDEX must be true or false".to_string())
            })?;
        }

        if let Ok(val) = std::env::var("PEARLS_OUTPUT_FORMAT") {
            self.output_format = match val.as_str() {
                "json" => OutputFormat::Json,
                "table" => OutputFormat::Table,
                "plain" => OutputFormat::Plain,
                _ => {
                    return Err(crate::Error::InvalidPearl(
                        "PEARLS_OUTPUT_FORMAT must be json, table, or plain".to_string(),
                    ))
                }
            };
        }

        if let Ok(val) = std::env::var("PEARLS_AUTO_CLOSE_ON_COMMIT") {
            self.auto_close_on_commit = val.parse().map_err(|_| {
                crate::Error::InvalidPearl(
                    "PEARLS_AUTO_CLOSE_ON_COMMIT must be true or false".to_string(),
                )
            })?;
        }

        Ok(())
    }

    /// Validates the configuration values.
    ///
    /// # Returns
    ///
    /// Ok if all values are valid, Err otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - default_priority is out of range (0-4)
    /// - compact_threshold_days is zero
    fn validate(&self) -> Result<()> {
        if self.default_priority > 4 {
            return Err(crate::Error::InvalidPearl(format!(
                "default_priority must be 0-4, got {}",
                self.default_priority
            )));
        }

        if self.compact_threshold_days == 0 {
            return Err(crate::Error::InvalidPearl(
                "compact_threshold_days must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Saves the configuration to a TOML file.
    ///
    /// # Arguments
    ///
    /// * `pearls_dir` - Path to the `.pearls` directory
    ///
    /// # Returns
    ///
    /// Ok if the file was written successfully, Err otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file cannot be created or written
    /// - Serialization fails
    pub fn save(&self, pearls_dir: &Path) -> Result<()> {
        let config_path = pearls_dir.join("config.toml");
        let content = toml::to_string_pretty(self).map_err(|e| {
            crate::Error::InvalidPearl(format!("Failed to serialize config: {}", e))
        })?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn clear_all_env_vars() {
        std::env::remove_var("PEARLS_DEFAULT_PRIORITY");
        std::env::remove_var("PEARLS_COMPACT_THRESHOLD_DAYS");
        std::env::remove_var("PEARLS_USE_INDEX");
        std::env::remove_var("PEARLS_OUTPUT_FORMAT");
        std::env::remove_var("PEARLS_AUTO_CLOSE_ON_COMMIT");
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_priority, 2);
        assert_eq!(config.compact_threshold_days, 30);
        assert!(!config.use_index);
        assert_eq!(config.output_format, OutputFormat::Table);
        assert!(!config.auto_close_on_commit);
    }

    #[test]
    fn test_config_load_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config::load(temp_dir.path()).unwrap();
        assert_eq!(config.default_priority, 2);
        assert_eq!(config.compact_threshold_days, 30);
    }

    #[test]
    fn test_config_load_from_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let content = r#"
default_priority = 1
compact_threshold_days = 60
use_index = true
output_format = "json"
auto_close_on_commit = true
"#;
        std::fs::write(&config_path, content).unwrap();

        // Clear any environment variables that might interfere
        std::env::remove_var("PEARLS_DEFAULT_PRIORITY");
        std::env::remove_var("PEARLS_COMPACT_THRESHOLD_DAYS");
        std::env::remove_var("PEARLS_USE_INDEX");
        std::env::remove_var("PEARLS_OUTPUT_FORMAT");
        std::env::remove_var("PEARLS_AUTO_CLOSE_ON_COMMIT");

        let config = Config::load(temp_dir.path()).unwrap();
        assert_eq!(config.default_priority, 1);
        assert_eq!(config.compact_threshold_days, 60);
        assert!(config.use_index);
        assert_eq!(config.output_format, OutputFormat::Json);
        assert!(config.auto_close_on_commit);
    }

    #[test]
    fn test_config_validation_invalid_priority() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let content = "default_priority = 5";
        std::fs::write(&config_path, content).unwrap();

        // Clear environment variables
        std::env::remove_var("PEARLS_DEFAULT_PRIORITY");
        std::env::remove_var("PEARLS_COMPACT_THRESHOLD_DAYS");
        std::env::remove_var("PEARLS_USE_INDEX");
        std::env::remove_var("PEARLS_OUTPUT_FORMAT");
        std::env::remove_var("PEARLS_AUTO_CLOSE_ON_COMMIT");

        let result = Config::load(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_validation_zero_threshold() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let content = "compact_threshold_days = 0";
        std::fs::write(&config_path, content).unwrap();

        // Clear environment variables
        std::env::remove_var("PEARLS_DEFAULT_PRIORITY");
        std::env::remove_var("PEARLS_COMPACT_THRESHOLD_DAYS");
        std::env::remove_var("PEARLS_USE_INDEX");
        std::env::remove_var("PEARLS_OUTPUT_FORMAT");
        std::env::remove_var("PEARLS_AUTO_CLOSE_ON_COMMIT");

        let result = Config::load(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_config_env_override_priority() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("PEARLS_DEFAULT_PRIORITY", "3");
        let config = Config::load(temp_dir.path()).unwrap();
        assert_eq!(config.default_priority, 3);

        clear_all_env_vars();
    }

    #[test]
    fn test_config_env_override_threshold() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("PEARLS_COMPACT_THRESHOLD_DAYS", "90");
        let config = Config::load(temp_dir.path()).unwrap();
        assert_eq!(config.compact_threshold_days, 90);

        clear_all_env_vars();
    }

    #[test]
    fn test_config_env_override_use_index() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("PEARLS_USE_INDEX", "true");
        let config = Config::load(temp_dir.path()).unwrap();
        assert!(config.use_index);

        clear_all_env_vars();
    }

    #[test]
    fn test_config_env_override_output_format() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("PEARLS_OUTPUT_FORMAT", "plain");
        let config = Config::load(temp_dir.path()).unwrap();
        assert_eq!(config.output_format, OutputFormat::Plain);

        clear_all_env_vars();
    }

    #[test]
    fn test_config_env_override_auto_close() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("PEARLS_AUTO_CLOSE_ON_COMMIT", "true");
        let config = Config::load(temp_dir.path()).unwrap();
        assert!(config.auto_close_on_commit);

        clear_all_env_vars();
    }

    #[test]
    fn test_config_env_invalid_priority() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("PEARLS_DEFAULT_PRIORITY", "invalid");
        let result = Config::load(temp_dir.path());
        assert!(result.is_err());

        clear_all_env_vars();
    }

    #[test]
    fn test_config_env_invalid_format() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        std::env::set_var("PEARLS_OUTPUT_FORMAT", "invalid");
        let result = Config::load(temp_dir.path());
        assert!(result.is_err());

        clear_all_env_vars();
    }

    #[test]
    fn test_config_save_and_load() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();

        let original = Config {
            default_priority: 1,
            compact_threshold_days: 45,
            use_index: true,
            output_format: OutputFormat::Json,
            auto_close_on_commit: true,
        };

        original.save(temp_dir.path()).unwrap();
        let loaded = Config::load(temp_dir.path()).unwrap();

        assert_eq!(original.default_priority, loaded.default_priority);
        assert_eq!(
            original.compact_threshold_days,
            loaded.compact_threshold_days
        );
        assert_eq!(original.use_index, loaded.use_index);
        assert_eq!(original.output_format, loaded.output_format);
        assert_eq!(original.auto_close_on_commit, loaded.auto_close_on_commit);

        clear_all_env_vars();
    }

    #[test]
    fn test_config_file_overridden_by_env() {
        clear_all_env_vars();
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let content = "default_priority = 1";
        std::fs::write(&config_path, content).unwrap();

        std::env::set_var("PEARLS_DEFAULT_PRIORITY", "3");
        let config = Config::load(temp_dir.path()).unwrap();
        assert_eq!(config.default_priority, 3);

        clear_all_env_vars();
    }
}
