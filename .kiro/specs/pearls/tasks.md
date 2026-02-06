# Implementation Plan: Pearls Issue Tracker

## Overview

This implementation plan breaks down the Pearls issue tracker into incremental, testable steps. The approach follows a bottom-up strategy: build core functionality first (data models, storage, graph), then add CLI commands, and finally integrate Git features. Each major component includes property-based tests to validate correctness properties from the design document.

The project uses a multi-crate workspace structure for separation of concerns:
- `pearls-core`: Core library (storage, graph, FSM, identity)
- `pearls-cli`: Command-line interface
- `pearls-merge`: Git merge driver
- `pearls-hooks`: Git hooks

## Tasks

- [X] 1. Set up workspace and project structure
  - Create Cargo workspace with four crates: pearls-core, pearls-cli, pearls-merge, pearls-hooks
  - Configure dependencies: serde, serde_json, clap, git2, petgraph, sha2, thiserror, anyhow, proptest, tabled, rayon
  - Set up .gitignore for Rust projects
  - Create basic module structure in each crate
  - _Requirements: All (foundational)_

- [x] 2. Implement core data models in pearls-core
  - [x] 2.1 Define Pearl struct with all fields and serde attributes
    - Implement mandatory fields: id, title, status, created_at, updated_at, author
    - Implement optional fields: description, priority, labels, deps, metadata
    - Add serde derives and default value functions
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9_
  
  - [x] 2.2 Define Status enum with FSM states
    - Implement enum variants: Open, InProgress, Blocked, Deferred, Closed
    - Add serde rename_all attribute for snake_case
    - _Requirements: 2.5, 5.1_
  
  - [x] 2.3 Define Dependency and DepType structures
    - Implement Dependency struct with target_id and dep_type fields
    - Implement DepType enum: Blocks, ParentChild, Related, DiscoveredFrom
    - _Requirements: 2.8, 4.2, 27.1-27.4_
  
  - [x] 2.4 Implement Pearl validation methods
    - Add validate() method checking mandatory fields, priority range, ID format
    - Add new() constructor with sensible defaults
    - _Requirements: 2.1, 2.6, 2.3_
  
  - [x] 2.5 Write property tests for data models
    - **Property 1: JSONL Round-Trip Preservation**
    - **Property 2: Single-Line Serialization**
    - **Property 4: Mandatory Field Presence**
    - **Property 5: Schema Conformance**
    - **Property 6: Optional Field Flexibility**
    - **Validates: Requirements 1.2, 1.3, 1.7, 2.1, 2.2, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9**

- [x] 3. Implement identity module in pearls-core
  - [x] 3.1 Implement hash-based ID generation
    - Create generate_id() function using SHA-256 of (title, author, timestamp, nonce)
    - Truncate hash to 6-8 hex characters and prefix with "prl-"
    - Implement collision detection with nonce increment
    - _Requirements: 3.1, 3.2, 3.3, 3.4_
  
  - [x] 3.2 Implement partial ID resolution
    - Create resolve_partial_id() function with prefix matching
    - Handle unique matches, ambiguous matches, and no matches
    - Require minimum 3 characters for partial IDs
    - _Requirements: 3.6, 3.7, 28.1-28.5_
  
  - [x] 3.3 Implement ID validation
    - Create validate_id_format() function with regex matching
    - _Requirements: 2.3_
  
  - [x] 3.4 Write property tests for identity module
    - **Property 7: ID Format Consistency**
    - **Property 8: ID Generation Determinism**
    - **Property 9: Partial ID Resolution Uniqueness**
    - **Property 10: Partial ID Ambiguity Detection**
    - **Validates: Requirements 2.3, 3.1, 3.2, 3.3, 3.6, 3.7**


- [x] 4. Implement storage module in pearls-core
  - [x] 4.1 Create Storage struct with file path and optional index
    - Implement new() constructor
    - Add path validation
    - _Requirements: 1.1, 10.6_
  
  - [x] 4.2 Implement JSONL file reading with streaming
    - Create load_all() method using serde_json streaming deserializer
    - Create load_by_id() method with early termination
    - Handle empty files and malformed JSON gracefully
    - _Requirements: 1.1, 1.3, 10.2, 10.3, 29.1, 29.2, 29.3_
  
  - [x] 4.3 Implement JSONL file writing with atomic operations
    - Create save() method for single Pearl (append or update)
    - Create save_all() method for bulk writes
    - Use temp file + rename for atomicity
    - Ensure single-line serialization with newline separation
    - _Requirements: 1.2, 1.4, 17.6_
  
  - [x] 4.4 Implement file locking for concurrent access
    - Add with_lock() method using fs2 crate
    - Implement platform-appropriate locking (flock/LockFileEx)
    - Add timeout handling for lock acquisition
    - Ensure lock release on success and failure
    - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5_
  
  - [x] 4.5 Implement delete() method
    - Remove Pearl from JSONL file by rewriting without it
    - Update index if enabled
    - _Requirements: 1.1_
  
  - [x] 4.6 Write property tests for storage module
    - **Property 1: JSONL Round-Trip Preservation** (integration with serialization)
    - **Property 3: Multi-Pearl Separation**
    - **Property 25: Write Atomicity**
    - **Property 26: Lock Release Guarantee**
    - **Property 27: Concurrent Write Serialization**
    - **Validates: Requirements 1.2, 1.4, 17.1, 17.4, 17.6**
  
  - [x] 4.7 Write unit tests for storage edge cases
    - Test empty file handling
    - Test malformed JSON recovery
    - Test concurrent read operations
    - Test lock timeout scenarios
    - _Requirements: 17.3, 17.5_

- [x] 5. Implement optional index file for large repositories
  - [x] 5.1 Create Index struct with HashMap and file path
    - Implement new() constructor
    - Add methods: insert(), get(), remove(), rebuild()
    - _Requirements: 10.6, 30.1, 30.2_
  
  - [x] 5.2 Implement index file serialization
    - Use binary format (bincode or custom)
    - Implement atomic updates alongside JSONL modifications
    - _Requirements: 10.7, 30.3_
  
  - [x] 5.3 Integrate index with Storage operations
    - Modify load_by_id() to use index when available
    - Update index on save() and delete()
    - Add rebuild_index() method for corruption recovery
    - _Requirements: 30.4, 30.5_
  
  - [x] 5.4 Write property tests for index module
    - **Property 31: Index Consistency**
    - **Property 32: Index Lookup Correctness**
    - **Validates: Requirements 10.7, 30.2, 30.4**

- [x] 6. Checkpoint - Validate storage layer
  - Ensure all storage tests pass
  - Verify JSONL files are human-readable
  - Test with sample data (create, read, update, delete)
  - Ask the user if questions arise


- [x] 7. Implement FSM module in pearls-core
  - [x] 7.1 Implement Status transition validation
    - Create can_transition_to() method on Status enum
    - Encode valid transitions: open→in_progress, in_progress→closed, etc.
    - Add is_blocked parameter to handle blocking dependencies
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_
  
  - [x] 7.2 Implement validate_transition() function
    - Take Pearl, target status, and graph as parameters
    - Check FSM rules and blocking dependencies
    - Return descriptive errors for invalid transitions
    - _Requirements: 5.6, 11.3_
  
  - [x] 7.3 Implement valid_transitions() helper
    - Return list of valid target states for current state
    - Consider blocking status
    - _Requirements: 5.1_
  
  - [x] 7.4 Write property tests for FSM module
    - **Property 14: Valid Transition Enforcement**
    - **Property 16: Deferred Transition Universality**
    - **Property 17: Reopen Capability**
    - **Validates: Requirements 5.1, 5.4, 5.5, 5.6**
  
  - [x] 7.5 Write unit tests for FSM edge cases
    - Test all valid transitions
    - Test all invalid transitions
    - Test transition error messages
    - _Requirements: 5.1, 5.6, 11.3_

- [x] 8. Implement graph module in pearls-core
  - [x] 8.1 Create IssueGraph struct with petgraph DiGraph
    - Implement from_pearls() constructor
    - Build graph from Pearl dependencies
    - Store Pearl data in HashMap for quick lookup
    - _Requirements: 4.1, 4.2_
  
  - [x] 8.2 Implement dependency management methods
    - Create add_dependency() with cycle detection
    - Create remove_dependency()
    - Validate dependency types
    - _Requirements: 4.2, 4.3, 4.6_
  
  - [x] 8.3 Implement cycle detection
    - Use petgraph::algo::is_cyclic_directed()
    - Create find_cycle() to return cycle path
    - Reject operations that would create cycles
    - _Requirements: 4.4, 4.5_
  
  - [x] 8.4 Implement topological sort
    - Use petgraph::algo::toposort()
    - Handle cyclic graphs gracefully
    - _Requirements: 6.1_
  
  - [x] 8.5 Implement blocking dependency queries
    - Create is_blocked() method checking for open blocking deps
    - Create blocking_deps() returning list of blockers
    - Filter by dependency type (only "blocks" type matters)
    - _Requirements: 4.3, 4.7, 5.2, 5.3_
  
  - [x] 8.6 Implement ready_queue() method
    - Filter by status (exclude closed, deferred)
    - Filter by blocking dependencies (zero open blockers)
    - Sort by priority ascending, then updated_at descending
    - _Requirements: 6.2, 6.3, 6.4, 6.5_
  
  - [x] 8.7 Write property tests for graph module
    - **Property 11: Cycle Detection**
    - **Property 12: Acyclic Graph Invariant**
    - **Property 13: Multiple Dependencies Support**
    - **Property 15: Blocking Dependency Constraint** (integration with FSM)
    - **Property 18: Blocked State Derivation**
    - **Property 19: Topological Sort Validity**
    - **Property 20: Ready Queue Unblocked Invariant**
    - **Property 21: Ready Queue Ordering**
    - **Validates: Requirements 4.3, 4.4, 4.5, 4.6, 4.7, 6.1, 6.2, 6.3, 6.4**
  
  - [x] 8.8 Write unit tests for graph edge cases
    - Test empty graph
    - Test single node graph
    - Test disconnected components
    - Test complex dependency chains
    - _Requirements: 4.1, 6.6_

- [x] 9. Checkpoint - Validate core library
  - Ensure all core module tests pass
  - Verify FSM and graph integration works correctly
  - Test end-to-end: create Pearls, add dependencies, check ready queue
  - Ask the user if questions arise


- [x] 10. Implement configuration management in pearls-core
  - [x] 10.1 Define Config struct with all settings
    - Add fields: default_priority, compact_threshold_days, use_index, output_format, auto_close_on_commit
    - Implement serde derives and default functions
    - _Requirements: 18.2_
  
  - [x] 10.2 Implement config file loading
    - Load from .pearls/config.toml
    - Use sensible defaults if file missing
    - Validate configuration values
    - _Requirements: 18.1, 18.3, 18.4_
  
  - [x] 10.3 Implement environment variable overrides
    - Support PEARLS_* environment variables
    - Override file config with env vars
    - _Requirements: 18.6_
  
  - [x] 10.4 Write unit tests for configuration
    - Test default values
    - Test file loading
    - Test validation errors
    - Test environment variable overrides
    - _Requirements: 18.1, 18.3, 18.4, 18.5, 18.6_

- [x] 11. Implement error types in pearls-core
  - [x] 11.1 Define Error enum with thiserror
    - Add variants: Io, Json, InvalidPearl, NotFound, CycleDetected, InvalidTransition, AmbiguousId, Git
    - Implement Display with descriptive messages
    - Add context fields for detailed errors
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6_
  
  - [x] 11.2 Implement Result type alias
    - Create type Result<T> = std::result::Result<T, Error>
    - _Requirements: 11.1_
  
  - [x] 11.3 Write unit tests for error messages
    - Test error formatting
    - Test error context preservation
    - Verify agent-friendly error messages
    - _Requirements: 11.2, 11.3, 11.4, 11.5_

- [x] 12. Set up CLI framework in pearls-cli
  - [x] 12.1 Define CLI structure with clap
    - Create main command with global flags: --json, --format, --no-color, --config
    - Define subcommands: init, create, show, list, ready, update, close, link, unlink, status, sync, compact, doctor, import
    - Add command-specific arguments and flags
    - _Requirements: 7.1, 7.2, 12.1, 12.3_
  
  - [x] 12.2 Implement output formatting module
    - Create OutputFormatter trait with methods: format_pearl(), format_list(), format_error()
    - Implement JsonFormatter for --json output
    - Implement TableFormatter for human-readable tables
    - Implement PlainFormatter for simple text
    - _Requirements: 12.2, 12.4, 12.5, 13.1, 13.2, 13.3_
  
  - [x] 12.3 Implement color and terminal UI utilities
    - Add color support with termcolor or similar
    - Respect NO_COLOR environment variable
    - Add table formatting with tabled crate
    - Handle terminal width for wrapping
    - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_
  
  - [x] 12.4 Write unit tests for output formatting
    - Test JSON output structure
    - Test table formatting
    - Test color handling
    - Test NO_COLOR respect
    - _Requirements: 12.2, 12.4, 13.1, 13.4_


- [x] 13. Implement core CLI commands (Part 1: Basic operations)
  - [x] 13.1 Implement `prl init` command
    - Create .pearls directory
    - Initialize empty issues.jsonl file
    - Create default config.toml
    - Configure Git merge driver and hooks (call setup functions)
    - _Requirements: 7.3, 8.1, 8.2_
  
  - [x] 13.2 Implement `prl create` command
    - Parse title and optional fields from arguments
    - Generate Pearl with hash ID
    - Append to issues.jsonl
    - Output created Pearl ID
    - _Requirements: 7.4, 2.3, 3.1, 3.2, 3.3_
  
  - [x] 13.3 Implement `prl show` command
    - Parse Pearl ID (support partial IDs)
    - Load Pearl from storage
    - Format and display Pearl details
    - _Requirements: 7.5, 3.6, 3.7_
  
  - [x] 13.4 Implement `prl list` command
    - Load all Pearls from storage
    - Apply filters: status, priority, labels, author
    - Sort by specified field
    - Format as table or JSON
    - _Requirements: 7.6, 22.3, 26.4_
  
  - [x] 13.5 Write integration tests for basic commands
    - Test init creates correct structure
    - Test create adds Pearl to file
    - Test show retrieves correct Pearl
    - Test list with various filters
    - _Requirements: 7.3, 7.4, 7.5, 7.6_

- [x] 14. Implement core CLI commands (Part 2: State management)
  - [x] 14.1 Implement `prl update` command
    - Parse Pearl ID and fields to update
    - Load Pearl, apply updates, validate
    - Update updated_at timestamp
    - Save back to storage
    - _Requirements: 7.7, 26.2_
  
  - [x] 14.2 Implement `prl close` command
    - Parse Pearl ID
    - Validate transition to closed (check blocking deps)
    - Update status and timestamp
    - Save to storage
    - _Requirements: 7.8, 5.1, 5.3_
  
  - [x] 14.3 Implement `prl ready` command
    - Load all Pearls and build graph
    - Call ready_queue() method
    - Format and display ready Pearls
    - Handle empty queue case
    - _Requirements: 7.6, 6.1, 6.2, 6.3, 6.4, 6.6_
  
  - [x] 14.4 Write integration tests for state management commands
    - Test update modifies fields correctly
    - Test close validates blocking dependencies
    - Test ready returns unblocked Pearls
    - Test ready queue ordering
    - _Requirements: 7.7, 7.8, 6.1, 6.2, 6.3, 6.4_

- [x] 15. Implement core CLI commands (Part 3: Dependencies)
  - [x] 15.1 Implement `prl link` command
    - Parse from ID, to ID, and dependency type
    - Load both Pearls
    - Add dependency with cycle detection
    - Save updated Pearl
    - _Requirements: 7.9, 4.2, 4.4, 4.5_
  
  - [x] 15.2 Implement `prl unlink` command
    - Parse from ID and to ID
    - Load Pearl and remove dependency
    - Save updated Pearl
    - _Requirements: 7.10, 4.2_
  
  - [x] 15.3 Write integration tests for dependency commands
    - Test link creates dependency
    - Test link detects cycles
    - Test unlink removes dependency
    - Test blocking dependency effects on FSM
    - _Requirements: 7.9, 7.10, 4.4, 4.5_

- [x] 16. Checkpoint - Validate core CLI functionality
  - Ensure all CLI command tests pass
  - Test full workflow: init → create → link → ready → close
  - Verify output formatting works correctly
  - Ask the user if questions arise


- [x] 17. Implement advanced CLI commands
  - [x] 17.1 Implement `prl status` command (Land the Plane protocol)
    - Check Git working directory status
    - Count open P0 (critical) Pearls
    - Check for unresolved blocking dependencies
    - Check sync status with remote
    - Display checklist with completion status
    - _Requirements: 7.11, 15.1, 15.2, 15.3, 15.4, 15.5, 15.6, 15.7_
  
  - [x] 17.2 Implement `prl compact` command
    - Parse threshold from config or argument
    - Identify closed Pearls older than threshold
    - Move to archive.jsonl
    - Remove from issues.jsonl
    - Preserve dependency references
    - Support --dry-run flag
    - _Requirements: 7.13, 14.1, 14.2, 14.3, 14.4, 14.6_
  
  - [x] 17.3 Implement `prl doctor` command
    - Validate JSONL syntax
    - Validate Pearl schema compliance
    - Check for orphaned dependencies
    - Check for cycles in graph
    - Check for duplicate IDs
    - Report issues with severity levels
    - Support --fix flag for auto-repair
    - _Requirements: 7.14, 20.1, 20.2, 20.3, 20.4, 20.5, 20.6, 20.7_
  
  - [x] 17.4 Implement `prl import beads` command
    - Parse Beads JSONL file
    - Validate and convert to Pearls format
    - Handle field mapping and incompatibilities
    - Write to .pearls/issues.jsonl
    - Provide migration summary report
    - _Requirements: 7.15, 16.1, 16.2, 16.3, 16.4, 16.5, 16.6, 16.7_
  
  - [x] 17.5 Write integration tests for advanced commands
    - Test status checklist generation
    - Test compact with various thresholds
    - Test doctor detects all issue types
    - Test import from Beads format
    - _Requirements: 15.1-15.7, 14.1-14.6, 20.1-20.7, 16.1-16.7_

- [x] 18. Implement `prl sync` command with Git integration
  - [x] 18.1 Implement Git operations wrapper
    - Use git2 crate for Git operations
    - Implement pull with rebase
    - Implement push with retry logic
    - Handle authentication and remote configuration
    - _Requirements: 7.12, 21.1, 21.4, 21.5_
  
  - [x] 18.2 Implement sync command logic
    - Execute git pull --rebase
    - Run integrity checks after merge
    - Execute git push if checks pass
    - Retry on push failure due to remote changes
    - Support --dry-run flag
    - _Requirements: 21.1, 21.2, 21.3, 21.4, 21.5, 21.6, 21.7_
  
  - [x] 18.3 Write integration tests for sync command
    - Test sync with clean working directory
    - Test sync with merge conflicts
    - Test sync retry logic
    - Test dry-run mode
    - _Requirements: 21.1-21.7_


- [x] 19. Implement Git merge driver (pearls-merge)
  - [x] 19.1 Create merge driver binary structure
    - Set up clap for argument parsing (ancestor, ours, theirs paths)
    - Create main entry point
    - _Requirements: 8.3_
  
  - [x] 19.2 Implement three-way merge algorithm
    - Parse all three JSONL files into HashMaps
    - Identify Pearls in each set: only_ours, only_theirs, both, unchanged
    - Implement field-level merge for Pearls in both
    - Use updated_at as tiebreaker for scalar fields
    - Union array fields (labels, deps) with deduplication
    - _Requirements: 8.4, 8.5, 8.6_
  
  - [x] 19.3 Implement conflict detection and marking
    - Detect unresolvable conflicts (same field, same timestamp, different values)
    - Mark conflicts in output or return error
    - _Requirements: 8.8_
  
  - [x] 19.4 Implement merge output generation
    - Combine all Pearls and sort by ID
    - Serialize to valid JSONL
    - Write to output file
    - _Requirements: 8.7_
  
  - [x] 19.5 Write property tests for merge driver
    - **Property 22: Three-Way Merge Validity**
    - **Property 23: Field-Level Merge Preservation**
    - **Property 24: List Field Union**
    - **Validates: Requirements 8.4, 8.5, 8.6, 8.7**
  
  - [x] 19.6 Write unit tests for merge scenarios
    - Test merge with no conflicts
    - Test merge with field-level conflicts
    - Test merge with list field conflicts
    - Test merge with unresolvable conflicts
    - _Requirements: 8.4, 8.5, 8.6, 8.8_

- [x] 20. Implement Git hooks (pearls-hooks)
  - [x] 20.1 Implement pre-commit hook
    - Validate JSONL syntax
    - Validate Pearl schema compliance
    - Check for duplicate IDs
    - Parse commit message for "Fixes (prl-XXXXXX)" pattern
    - Auto-close referenced Pearls if pattern found
    - _Requirements: 9.1, 9.2, 9.3, 9.4_
  
  - [x] 20.2 Implement post-merge hook
    - Run integrity checks on dependency graph
    - Detect and report cycles
    - Detect and report orphaned dependencies
    - _Requirements: 9.5, 9.6, 9.7_
  
  - [x] 20.3 Implement hook installation in init command
    - Copy hook scripts to .git/hooks/
    - Make hooks executable
    - Configure Git to use hooks
    - _Requirements: 9.1, 9.5_
  
  - [x] 20.4 Write integration tests for Git hooks
    - Test pre-commit validation
    - Test auto-close on commit message
    - Test post-merge integrity checks
    - _Requirements: 9.1-9.7_

- [x] 21. Checkpoint - Validate Git integration
  - Ensure merge driver handles all conflict scenarios
  - Test hooks trigger correctly on Git operations
  - Test full Git workflow: branch → modify → merge
  - Ask the user if questions arise


- [x] 22. Implement label and priority features
  - [x] 22.1 Add label filtering to list command
    - Parse --label flag
    - Filter Pearls by label (case-insensitive matching)
    - Support multiple label filters
    - _Requirements: 22.3, 22.6_
  
  - [x] 22.2 Add label autocomplete suggestions
    - Collect all unique labels from existing Pearls
    - Provide suggestions when creating/updating Pearls
    - _Requirements: 22.4_
  
  - [x] 22.3 Add priority display and filtering
    - Format priority as P0, P1, P2, P3, P4
    - Add --priority flag to list command
    - Ensure ready queue sorts by priority correctly
    - _Requirements: 23.4, 23.5_
  
  - [x] 22.4 Write property tests for label and priority
    - **Property 35: Label Case Preservation**
    - **Property 36: Priority Range Validation**
    - **Property 37: Priority Default**
    - **Validates: Requirements 22.6, 23.1, 23.2**
  
  - [x] 22.5 Write unit tests for label and priority features
    - Test label filtering
    - Test label case-insensitivity
    - Test priority formatting
    - Test priority sorting
    - _Requirements: 22.1-22.6, 23.1-23.5_

- [x] 23. Implement description and metadata features
  - [x] 23.1 Add description support to create and update commands
    - Accept --description flag or read from stdin
    - Support multi-line Markdown descriptions
    - Validate description length (max 64KB)
    - _Requirements: 24.1, 24.3, 24.5_
  
  - [x] 23.2 Add description display to show command
    - Format Markdown for terminal display
    - Preserve formatting and newlines
    - _Requirements: 24.2_
  
  - [x] 23.3 Add metadata read/write commands
    - Implement `prl meta get <id> <key>` command
    - Implement `prl meta set <id> <key> <value>` command
    - Support JSON values for metadata
    - _Requirements: 19.1, 19.3, 19.5_
  
  - [x] 23.4 Write property tests for description and metadata
    - **Property 33: Metadata Preservation**
    - **Property 34: Unknown Field Tolerance**
    - **Property 38: Markdown Preservation**
    - **Validates: Requirements 2.10, 19.2, 19.3, 24.2, 24.3**
  
  - [x] 23.5 Write unit tests for description and metadata
    - Test multi-line description handling
    - Test Markdown preservation
    - Test metadata get/set operations
    - Test unknown field preservation
    - _Requirements: 24.1-24.5, 19.1-19.5_

- [x] 24. Implement author and timestamp features
  - [x] 24.1 Add author tracking to create command
    - Derive author from Git config (user.name)
    - Fall back to system username if Git config unavailable
    - Support --author flag for override
    - _Requirements: 25.1, 25.2, 25.3, 25.4_
  
  - [x] 24.2 Add timestamp display formatting
    - Format timestamps as human-readable (e.g., "2 days ago")
    - Support absolute timestamp display with flag
    - _Requirements: 26.3_
  
  - [x] 24.3 Add date range filtering to list command
    - Support --created-after and --created-before flags
    - Support --updated-after and --updated-before flags
    - _Requirements: 26.4_
  
  - [x] 24.4 Write property tests for timestamps
    - **Property 39: Timestamp Update on Modification**
    - **Property 40: Timestamp Immutability on Read**
    - **Validates: Requirements 26.1, 26.2, 26.5**
  
  - [x] 24.5 Write unit tests for author and timestamp features
    - Test author derivation from Git config
    - Test author fallback to system username
    - Test timestamp formatting
    - Test date range filtering
    - _Requirements: 25.1-25.5, 26.1-26.5_


- [x] 25. Implement dependency type semantics
  - [x] 25.1 Add dependency type filtering to graph queries
    - Filter by dependency type in blocking_deps() method
    - Only "blocks" type affects FSM constraints
    - Other types are informational only
    - _Requirements: 27.1, 27.2, 27.3, 27.4_
  
  - [x] 25.2 Add dependency type display to show command
    - Display all dependencies with their types
    - Highlight blocking dependencies
    - _Requirements: 27.5_
  
  - [x] 25.3 Add dependency type filtering to list command
    - Support --dep-type flag to filter by dependency type
    - _Requirements: 27.6_
  
  - [x] 25.4 Write unit tests for dependency type semantics
    - Test blocks type affects FSM
    - Test other types don't affect FSM
    - Test dependency type filtering
    - _Requirements: 27.1-27.6_

- [x] 26. Implement archive query support
  - [x] 26.1 Modify show command to search archive
    - Check issues.jsonl first
    - Fall back to archive.jsonl if not found
    - _Requirements: 14.5_
  
  - [x] 26.2 Modify list command to support --include-archived flag
    - Load both active and archived Pearls when flag present
    - Mark archived Pearls in output
    - _Requirements: 14.5_
  
  - [x] 26.3 Write unit tests for archive queries
    - Test show finds archived Pearls
    - Test list includes archived Pearls with flag
    - _Requirements: 14.5_

- [x] 27. Add performance optimizations
  - [x] 27.1 Implement parallel processing with rayon
    - Use rayon for parallel Pearl processing in list command
    - Use rayon for parallel graph operations where applicable
    - _Requirements: 10.4_
  
  - [x] 27.2 Optimize streaming deserialization
    - Ensure early termination in load_by_id()
    - Use streaming for large file operations
    - _Requirements: 10.2, 10.3, 29.1, 29.2, 29.3_
  
  - [x] 27.3 Add progress indicators for long operations
    - Show progress for compact operation
    - Show progress for doctor operation
    - Show progress for import operation
    - _Requirements: 13.6_
  
  - [x] 27.4 Write performance benchmarks
    - Benchmark load_all with 1000 Pearls
    - Benchmark topological_sort with 1000 nodes
    - Benchmark create operation
    - Benchmark ready_queue with 1000 Pearls
    - Verify targets: load <10ms, toposort <5ms, create <1ms, ready <15ms
    - _Requirements: 10.1, 10.5_

- [x] 28. Final integration and polish
  - [x] 28.1 Add comprehensive error handling
    - Ensure all errors have descriptive messages
    - Add context to errors where helpful
    - Test error messages for agent-friendliness
    - _Requirements: 11.1-11.7_
  
  - [x] 28.2 Add comprehensive documentation
    - Write README.md with installation and usage
    - Add doc comments to all public APIs
    - Create examples directory with sample workflows
    - _Requirements: All (documentation)_
  
  - [x] 28.3 Add CLI help text and examples
    - Ensure all commands have clear help text
    - Add examples to help output
    - _Requirements: 7.1-7.15_
  
  - [x] 28.4 Write end-to-end integration tests
    - Test full workflow: init → create → link → ready → close → compact
    - Test Git workflow: init → create → commit → branch → merge
    - Test concurrent access scenarios
    - Test large repository scenarios (10k+ Pearls)
    - _Requirements: All (integration)_

- [ ] 29. Final checkpoint - Complete system validation
  - Run all tests (unit, property, integration)
  - Run performance benchmarks
  - Test on Linux, macOS, Windows
  - Verify all requirements are met
  - Ask the user if questions arise
  - Note: Local `cargo test`/`cargo bench` blocked by offline crates.io access.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The implementation follows a bottom-up approach: core library first, then CLI, then Git integration
- Multi-crate structure enables better separation of concerns and testing
