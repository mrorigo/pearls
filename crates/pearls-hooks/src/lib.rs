// Rust guideline compliant 2026-02-06

//! Pearls Git Hooks
//!
//! This crate provides Git hook implementations for Pearls:
//! - Pre-commit validation
//! - Post-merge integrity checks

pub mod post_merge;
pub mod pre_commit;

pub use post_merge::post_merge_hook;
pub use pre_commit::pre_commit_hook;
