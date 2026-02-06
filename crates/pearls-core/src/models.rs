// Rust guideline compliant 2026-02-06

//! Core data models for Pearls.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Status of a Pearl in the finite state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Pearl is open and ready to be worked on.
    Open,
    /// Pearl is currently being worked on.
    InProgress,
    /// Pearl is blocked by dependencies.
    Blocked,
    /// Pearl is deferred for later.
    Deferred,
    /// Pearl is closed and complete.
    Closed,
}

/// Type of dependency relationship between Pearls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepType {
    /// Blocking dependency - target must be closed before dependent can progress.
    Blocks,
    /// Parent-child hierarchical relationship.
    ParentChild,
    /// Related but non-blocking relationship.
    Related,
    /// Provenance tracking - this Pearl was discovered from another.
    DiscoveredFrom,
}

/// Dependency relationship to another Pearl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dependency {
    /// ID of the target Pearl.
    pub target_id: String,
    /// Type of dependency relationship.
    pub dep_type: DepType,
}

/// A Pearl represents a single issue or task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pearl {
    /// Unique hash-based identifier (format: prl-XXXXXX).
    pub id: String,
    /// One-line summary of the Pearl.
    pub title: String,
    /// Detailed Markdown description.
    #[serde(default)]
    pub description: String,
    /// Current status in the FSM.
    pub status: Status,
    /// Priority level (0=critical, 4=trivial).
    #[serde(default = "default_priority")]
    pub priority: u8,
    /// Unix timestamp of creation.
    pub created_at: i64,
    /// Unix timestamp of last update.
    pub updated_at: i64,
    /// Author identifier.
    pub author: String,
    /// Labels for categorization.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Dependencies on other Pearls.
    #[serde(default)]
    pub deps: Vec<Dependency>,
    /// Extensible metadata for agent-specific data.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Default priority value (medium).
fn default_priority() -> u8 {
    2
}

impl Pearl {
    /// Creates a new Pearl with sensible defaults.
    ///
    /// # Arguments
    ///
    /// * `title` - The Pearl title
    /// * `author` - The author identifier
    ///
    /// # Returns
    ///
    /// A new Pearl with default values for optional fields.
    pub fn new(title: String, author: String) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time before UNIX epoch")
            .as_secs() as i64;

        let id = crate::identity::generate_id(&title, &author, now, 0);

        Self {
            id,
            title,
            description: String::new(),
            status: Status::Open,
            priority: default_priority(),
            created_at: now,
            updated_at: now,
            author,
            labels: Vec::new(),
            deps: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Validates the Pearl data.
    ///
    /// # Returns
    ///
    /// Ok if the Pearl is valid, Err otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Title is empty
    /// - Priority is out of range (0-4)
    /// - ID format is invalid
    pub fn validate(&self) -> crate::Result<()> {
        if self.title.is_empty() {
            return Err(crate::Error::InvalidPearl(
                "Title cannot be empty".to_string(),
            ));
        }

        if self.priority > 4 {
            return Err(crate::Error::InvalidPearl(format!(
                "Priority must be 0-4, got {}",
                self.priority
            )));
        }

        crate::identity::validate_id_format(&self.id)?;

        Ok(())
    }
}
