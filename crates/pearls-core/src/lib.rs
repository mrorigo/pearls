// Rust guideline compliant 2026-02-06

//! Pearls Core Library
//!
//! This crate provides the foundational components for the Pearls issue tracking system:
//! - Data models (Pearl, Dependency, Status)
//! - Storage engine (JSONL read/write, streaming, indexing)
//! - Graph algorithms (DAG, cycle detection, topological sort)
//! - FSM logic (state transitions, validation)
//! - Hash ID generation and resolution
//! - Error types and result handling

pub mod config;
pub mod error;
pub mod fsm;
pub mod graph;
pub mod identity;
pub mod models;
pub mod storage;

pub use config::{Config, OutputFormat};
pub use error::{Error, Result};
pub use fsm::validate_transition;
pub use graph::IssueGraph;
pub use models::{Comment, DepType, Dependency, Pearl, Status};
pub use storage::Storage;
