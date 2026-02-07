// Rust guideline compliant 2026-02-06

//! Core data models for Pearls.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// A comment attached to a Pearl.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    /// Unique hash-based comment identifier (format: cmt-XXXXXX).
    pub id: String,
    /// Comment author identifier.
    pub author: String,
    /// Comment body text.
    pub body: String,
    /// Unix timestamp of creation.
    pub created_at: i64,
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
    /// Comments attached to this Pearl.
    #[serde(default)]
    pub comments: Vec<Comment>,
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
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
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
            comments: Vec::new(),
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

        for comment in &self.comments {
            if comment.id.trim().is_empty() {
                return Err(crate::Error::InvalidPearl(
                    "Comment ID cannot be empty".to_string(),
                ));
            }

            if comment.author.trim().is_empty() {
                return Err(crate::Error::InvalidPearl(
                    "Comment author cannot be empty".to_string(),
                ));
            }

            if comment.body.trim().is_empty() {
                return Err(crate::Error::InvalidPearl(
                    "Comment body cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Adds a comment and returns the new comment ID.
    ///
    /// # Arguments
    ///
    /// * `author` - Comment author
    /// * `body` - Comment body
    ///
    /// # Returns
    ///
    /// The new comment ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the author or body is empty.
    pub fn add_comment(&mut self, author: String, body: String) -> crate::Result<String> {
        let author = author.trim().to_string();
        let body = body.trim().to_string();

        if author.is_empty() {
            return Err(crate::Error::InvalidPearl(
                "Comment author cannot be empty".to_string(),
            ));
        }

        if body.is_empty() {
            return Err(crate::Error::InvalidPearl(
                "Comment body cannot be empty".to_string(),
            ));
        }

        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs() as i64;

        let mut nonce = 0u32;
        let comment_id = loop {
            let id = generate_comment_id(&self.id, &author, &body, now, nonce);
            if !self.comments.iter().any(|comment| comment.id == id) {
                break id;
            }
            nonce = nonce.saturating_add(1);
        };

        self.comments.push(Comment {
            id: comment_id.clone(),
            author,
            body,
            created_at: now,
        });
        self.updated_at = now;

        Ok(comment_id)
    }

    /// Deletes a comment by ID.
    ///
    /// # Arguments
    ///
    /// * `comment_id` - The comment ID to delete
    ///
    /// # Returns
    ///
    /// True if a comment was deleted, false otherwise.
    pub fn delete_comment(&mut self, comment_id: &str) -> bool {
        let initial_len = self.comments.len();
        self.comments.retain(|comment| comment.id != comment_id);
        if self.comments.len() != initial_len {
            use std::time::{SystemTime, UNIX_EPOCH};
            if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
                self.updated_at = now.as_secs() as i64;
            }
            true
        } else {
            false
        }
    }
}

fn generate_comment_id(
    pearl_id: &str,
    author: &str,
    body: &str,
    timestamp: i64,
    nonce: u32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pearl_id.as_bytes());
    hasher.update(author.as_bytes());
    hasher.update(body.as_bytes());
    hasher.update(timestamp.to_le_bytes());
    hasher.update(nonce.to_le_bytes());

    let hash = hasher.finalize();
    let hex = format!("{:x}", hash);
    format!("cmt-{}", &hex[..6])
}
