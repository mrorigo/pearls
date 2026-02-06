# Requirements Document: Pearls Issue Tracker

## Introduction

Pearls is a lightweight, Git-native distributed issue tracking system designed specifically for agentic workflows. It provides a production-grade successor to the Beads system, implementing a radical architectural simplification: eliminating the SQLite cache and background daemon in favor of a single JSONL file as the source of truth, with Rust's performance enabling real-time parsing.

The system addresses the fundamental crisis in project management tooling for LLM agents by providing locality (Git-backed data), structure (strict JSON schema), graph awareness (dependency enforcement), and zero-conflict identity (hash-based IDs). Pearls enables agents to maintain structured memory across sessions, execute long-horizon tasks, and coordinate work without the latency and context penalties of traditional issue trackers.

## Glossary

- **Pearl**: A single issue or task tracked by the system, represented as a JSON object
- **JSONL**: JSON Lines format, where each line is a complete JSON object
- **CLI**: Command-Line Interface, the primary interaction method for Pearls
- **FSM**: Finite State Machine, the strict state transition system for issue status
- **DAG**: Directed Acyclic Graph, the dependency structure where issues are nodes and dependencies are edges
- **Hash_ID**: Content-addressable identifier generated from SHA-256 hash (format: prl-a1b2c3)
- **Blocking_Dependency**: A dependency relationship where issue A must be closed before issue B can progress
- **Ready_Queue**: The set of issues with zero open blocking dependencies, available for work
- **Merge_Driver**: Custom Git merge handler for conflict-free JSONL merging
- **Agent**: An LLM-based autonomous system that interacts with Pearls via CLI
- **Topological_Sort**: Graph algorithm that produces a linear ordering respecting dependency constraints
- **Snapshot_Persistence**: Storage strategy where the file represents current state, relying on Git for history
- **Index_File**: Optional binary file mapping Hash_IDs to file offsets for O(log n) lookup
- **Compact_Operation**: Process of archiving closed issues to reduce active dataset size

## Requirements

### Requirement 1: JSONL Storage and Persistence

**User Story:** As a developer, I want all issue data stored in a single human-readable JSONL file, so that I can version control it with Git and inspect it without special tools.

#### Acceptance Criteria

1. THE System SHALL store all Pearl data in a single file at `.pearls/issues.jsonl`
2. WHEN a Pearl is created or updated, THE System SHALL write it as a complete JSON object on a single line
3. THE System SHALL ensure each line in the JSONL file is valid JSON conforming to the Pearl schema
4. WHEN multiple Pearls are stored, THE System SHALL separate them with newline characters
5. THE System SHALL use snapshot-based persistence where the file represents current state
6. WHEN the JSONL file is modified, THE System SHALL maintain syntactic validity to prevent parse errors
7. THE System SHALL support UTF-8 encoding for all text fields

### Requirement 2: Pearl Schema and Data Model

**User Story:** As an agent, I want each Pearl to have a strict, typed schema, so that I can reliably parse and manipulate issue data without hallucinations.

#### Acceptance Criteria

1. THE System SHALL enforce a Pearl schema with mandatory fields: id, title, status, created_at, updated_at, author
2. THE System SHALL support optional fields: description, priority, labels, deps, metadata
3. WHEN a Pearl is created, THE System SHALL generate a unique Hash_ID in format `prl-XXXXXX` where X is hexadecimal
4. THE System SHALL store timestamps as Unix epoch integers in UTC
5. THE System SHALL validate that status is one of: open, in_progress, blocked, deferred, closed
6. THE System SHALL store priority as an integer from 0 (critical) to 4 (trivial) with default value 2
7. THE System SHALL store labels as an array of strings
8. THE System SHALL store dependencies as an array of Dependency objects with fields: target_id, dep_type
9. THE System SHALL store metadata as a flexible key-value map for agent-specific data
10. WHEN deserializing a Pearl with unknown fields, THE System SHALL preserve or ignore them based on configuration

### Requirement 3: Hash-Based Identity System

**User Story:** As a distributed team member, I want collision-resistant issue IDs, so that multiple agents can create issues concurrently without merge conflicts.

#### Acceptance Criteria

1. WHEN generating a Pearl ID, THE System SHALL compute SHA-256 hash of (title, author, timestamp, nonce)
2. THE System SHALL truncate the hash to 6-8 hexadecimal characters
3. THE System SHALL prefix the truncated hash with "prl-" to create the final ID
4. IF a hash collision is detected, THEN THE System SHALL increment the nonce and regenerate the ID
5. THE System SHALL ensure IDs are immutable after creation
6. WHEN a user provides a partial ID, THE System SHALL resolve it to the full ID if the prefix is unique
7. IF a partial ID matches multiple Pearls, THEN THE System SHALL return an error listing all matches

### Requirement 4: Directed Acyclic Graph and Dependencies

**User Story:** As an agent, I want to define dependency relationships between issues, so that I can understand the correct order of work and avoid working on blocked tasks.

#### Acceptance Criteria

1. THE System SHALL model issues as nodes in a directed graph
2. THE System SHALL support dependency types: blocks, parent_child, related, discovered_from
3. WHEN a dependency of type "blocks" is created from A to B, THE System SHALL prevent B from transitioning to in_progress until A is closed
4. WHEN a dependency is created, THE System SHALL perform cycle detection
5. IF adding a dependency would create a cycle, THEN THE System SHALL reject the operation with a descriptive error
6. THE System SHALL allow multiple dependencies per Pearl
7. WHEN a blocking dependency is closed, THE System SHALL automatically update the dependent Pearl's derived blocked state

### Requirement 5: Finite State Machine Enforcement

**User Story:** As an agent, I want strict state transition rules, so that I cannot create invalid issue states that would confuse my workflow.

#### Acceptance Criteria

1. THE System SHALL enforce valid state transitions: open→in_progress, in_progress→closed, in_progress→open, any→deferred, closed→open
2. WHEN a Pearl has open blocking dependencies, THE System SHALL prevent transition from open to in_progress
3. WHEN a Pearl has open blocking dependencies, THE System SHALL prevent transition to closed
4. THE System SHALL allow transition to deferred from any state
5. THE System SHALL allow reopening a closed Pearl by transitioning closed→open
6. WHEN an invalid state transition is attempted, THE System SHALL return an error describing the constraint violation
7. THE System SHALL derive the "blocked" meta-state from graph traversal rather than storing it as a static field

### Requirement 6: Topological Sorting and Ready Queue

**User Story:** As an agent, I want to query for unblocked tasks, so that I always work on the highest-priority task that is ready for execution.

#### Acceptance Criteria

1. WHEN the ready command is invoked, THE System SHALL perform topological sort on the dependency graph
2. THE System SHALL filter out Pearls with status closed or deferred
3. THE System SHALL return only Pearls with zero open blocking dependencies
4. THE System SHALL sort the ready queue by priority ascending, then by updated_at descending
5. THE System SHALL exclude Pearls that are blocked by open dependencies from the ready queue
6. WHEN the ready queue is empty, THE System SHALL return an appropriate message indicating no work is available

### Requirement 7: CLI Interface and Commands

**User Story:** As a user, I want a comprehensive CLI with intuitive commands, so that I can manage issues efficiently without a background daemon.

#### Acceptance Criteria

1. THE System SHALL provide a CLI executable named `prl`
2. THE System SHALL support commands: init, create, show, list, ready, update, close, link, unlink, status, sync, compact, doctor, import
3. WHEN the init command is invoked, THE System SHALL create the `.pearls` directory and initialize `issues.jsonl`
4. WHEN the create command is invoked with title and optional fields, THE System SHALL generate a new Pearl and append it to the JSONL file
5. WHEN the show command is invoked with an ID, THE System SHALL display the complete Pearl data
6. WHEN the list command is invoked, THE System SHALL display all Pearls with configurable filtering
7. WHEN the update command is invoked, THE System SHALL modify the specified Pearl fields and rewrite the JSONL file
8. WHEN the close command is invoked, THE System SHALL transition the Pearl to closed status if not blocked
9. WHEN the link command is invoked with two IDs and a dependency type, THE System SHALL create the dependency after cycle detection
10. WHEN the unlink command is invoked, THE System SHALL remove the specified dependency
11. WHEN the status command is invoked, THE System SHALL display project health metrics and checklist
12. WHEN the sync command is invoked, THE System SHALL perform git pull, merge, and push operations
13. WHEN the compact command is invoked, THE System SHALL archive closed Pearls older than a threshold
14. WHEN the doctor command is invoked, THE System SHALL validate graph integrity and report issues
15. WHEN the import command is invoked with a Beads JSONL file, THE System SHALL migrate the data to Pearls format

### Requirement 8: Git Integration and Merge Driver

**User Story:** As a developer, I want seamless Git integration with conflict-free merging, so that multiple agents can work on different branches without corrupting the issue database.

#### Acceptance Criteria

1. WHEN the init command is invoked, THE System SHALL configure a custom merge driver in `.git/config`
2. WHEN the init command is invoked, THE System SHALL create a `.gitattributes` file specifying the merge driver for `issues.jsonl`
3. WHEN Git detects a merge conflict in `issues.jsonl`, THE System SHALL invoke the `pearls-merge` binary
4. THE Merge_Driver SHALL parse the ancestor, ours, and theirs versions into HashMaps
5. THE Merge_Driver SHALL perform field-level merging for Pearls modified in both branches
6. THE Merge_Driver SHALL handle list field merging (labels, dependencies) by union or conflict resolution
7. THE Merge_Driver SHALL ensure the output is syntactically valid JSONL
8. IF the Merge_Driver cannot resolve a conflict automatically, THEN THE System SHALL mark the conflict and require manual resolution

### Requirement 9: Git Hooks for Validation

**User Story:** As a developer, I want automatic validation on commit and merge, so that the issue database remains consistent with the codebase.

#### Acceptance Criteria

1. WHEN the init command is invoked, THE System SHALL install a pre-commit hook
2. THE Pre_Commit_Hook SHALL validate that `issues.jsonl` is syntactically correct JSON
3. THE Pre_Commit_Hook SHALL check for schema violations in all Pearls
4. IF the commit message contains "Fixes (prl-XXXXXX)", THEN THE Pre_Commit_Hook SHALL auto-close the referenced Pearl
5. WHEN the init command is invoked, THE System SHALL install a post-merge hook
6. THE Post_Merge_Hook SHALL run integrity checks on the dependency graph
7. THE Post_Merge_Hook SHALL detect and report orphaned dependencies

### Requirement 10: Performance and Scalability

**User Story:** As a user, I want millisecond-level command execution, so that the CLI feels instant even with thousands of issues.

#### Acceptance Criteria

1. THE System SHALL start, execute, and exit in under 100 milliseconds for typical operations
2. WHEN parsing the JSONL file, THE System SHALL use streaming deserialization
3. WHEN executing the show command, THE System SHALL stop parsing after finding the target Pearl
4. THE System SHALL support parallel processing with rayon for operations on multiple Pearls
5. THE System SHALL maintain performance with up to 100,000 Pearls without degradation
6. WHERE the repository contains more than 50,000 Pearls, THE System SHALL support an optional index file for O(log n) lookup
7. WHEN the index file is enabled, THE System SHALL update it atomically with JSONL modifications

### Requirement 11: Error Handling and Agent-Friendly Messages

**User Story:** As an agent, I want precise, structured error messages, so that I can self-correct and retry operations without human intervention.

#### Acceptance Criteria

1. WHEN an error occurs, THE System SHALL return a non-zero exit code
2. THE System SHALL provide error messages in a consistent format with error type and description
3. WHEN a state transition is invalid, THE System SHALL explain which constraint was violated
4. WHEN a dependency would create a cycle, THE System SHALL list the cycle path
5. WHEN a Pearl ID is not found, THE System SHALL suggest similar IDs if available
6. THE System SHALL distinguish between user errors (invalid input) and system errors (IO failure)
7. THE System SHALL log detailed error context for debugging without exposing it to the user by default

### Requirement 12: Structured Output for Agents

**User Story:** As an agent, I want machine-readable output formats, so that I can parse command results without heuristic text processing.

#### Acceptance Criteria

1. THE System SHALL support a `--json` flag for all commands
2. WHEN the `--json` flag is provided, THE System SHALL output results as valid JSON
3. THE System SHALL support a `--format` flag with options: json, table, plain
4. WHEN outputting lists, THE System SHALL provide consistent field ordering
5. THE System SHALL include metadata in JSON output (e.g., total count, query time)
6. WHEN errors occur with `--json` flag, THE System SHALL output error details as JSON

### Requirement 13: Human-Friendly Terminal UI

**User Story:** As a human developer, I want rich terminal output with colors and tables, so that I can quickly scan issue lists and understand project status.

#### Acceptance Criteria

1. WHEN outputting to a TTY, THE System SHALL use colors to highlight status, priority, and IDs
2. THE System SHALL format list output as tables with aligned columns
3. THE System SHALL use Unicode box-drawing characters for visual hierarchy
4. THE System SHALL respect the NO_COLOR environment variable
5. WHEN the terminal width is narrow, THE System SHALL wrap or truncate output gracefully
6. THE System SHALL provide progress indicators for long-running operations

### Requirement 14: Compaction and Archival

**User Story:** As an agent, I want to archive old closed issues, so that my context window remains focused on active work.

#### Acceptance Criteria

1. WHEN the compact command is invoked, THE System SHALL identify closed Pearls older than a configurable threshold
2. THE System SHALL move archived Pearls to `.pearls/archive.jsonl`
3. THE System SHALL remove archived Pearls from the active `issues.jsonl` file
4. THE System SHALL preserve dependency references to archived Pearls
5. WHEN querying archived Pearls, THE System SHALL search both active and archive files
6. THE System SHALL provide a `--dry-run` flag to preview compaction without executing it

### Requirement 15: Land the Plane Protocol

**User Story:** As an agent, I want a pre-sign-off checklist, so that I can verify all work is complete before ending a session.

#### Acceptance Criteria

1. WHEN the status command is invoked, THE System SHALL display a checklist of completion criteria
2. THE System SHALL check for a clean Git working directory
3. THE System SHALL check for open P0 (critical priority) Pearls
4. THE System SHALL check for unresolved blocking dependencies
5. THE System SHALL indicate whether all tests have passed (via metadata or external integration)
6. THE System SHALL indicate whether the local branch is synced with remote
7. THE System SHALL provide a summary of work completed in the current session

### Requirement 16: Beads Migration Support

**User Story:** As a Beads user, I want to migrate my existing issues to Pearls, so that I can transition to the new system without data loss.

#### Acceptance Criteria

1. WHEN the import beads command is invoked with a path to `.beads/issues.jsonl`, THE System SHALL parse the Beads format
2. THE System SHALL validate that each Beads issue conforms to the Pearls schema
3. THE System SHALL convert Beads-specific fields to Pearls equivalents
4. THE System SHALL preserve all dependencies and metadata during migration
5. IF a Beads issue has an incompatible field, THEN THE System SHALL log a warning and skip or adapt the field
6. THE System SHALL write the migrated data to `.pearls/issues.jsonl`
7. THE System SHALL provide a summary report of migration success and any issues encountered

### Requirement 17: Concurrent Access and File Locking

**User Story:** As a user, I want safe concurrent access to the issue database, so that multiple CLI invocations do not corrupt data.

#### Acceptance Criteria

1. WHEN a write operation begins, THE System SHALL acquire an exclusive lock on `issues.jsonl`
2. THE System SHALL use platform-appropriate file locking (flock on Unix, LockFileEx on Windows)
3. IF the lock cannot be acquired within a timeout, THEN THE System SHALL return an error
4. WHEN a write operation completes, THE System SHALL release the lock immediately
5. THE System SHALL allow multiple concurrent read operations without locking
6. THE System SHALL ensure atomic writes by writing to a temporary file and renaming

### Requirement 18: Configuration Management

**User Story:** As a user, I want to configure Pearls behavior, so that I can adapt it to my workflow preferences.

#### Acceptance Criteria

1. THE System SHALL support a configuration file at `.pearls/config.toml`
2. THE System SHALL allow configuration of: default priority, compaction threshold, index file usage, output format
3. WHEN a configuration file is not present, THE System SHALL use sensible defaults
4. THE System SHALL validate configuration values on load
5. IF a configuration value is invalid, THEN THE System SHALL return an error with the expected format
6. THE System SHALL support environment variable overrides for configuration values

### Requirement 19: Metadata and Extensibility

**User Story:** As an agent developer, I want to store custom metadata in Pearls, so that I can extend the system for specialized workflows without modifying the core schema.

#### Acceptance Criteria

1. THE System SHALL provide a metadata field as a flexible JSON object
2. THE System SHALL preserve unknown metadata keys during read-write cycles
3. THE System SHALL allow agents to store workflow-specific data (e.g., LLM conversation IDs, test results)
4. THE System SHALL not enforce schema validation on metadata contents
5. THE System SHALL provide CLI commands to read and write metadata fields

### Requirement 20: Doctor Command and Integrity Checks

**User Story:** As a developer, I want to validate the integrity of my issue database, so that I can detect and repair corruption or inconsistencies.

#### Acceptance Criteria

1. WHEN the doctor command is invoked, THE System SHALL parse all Pearls and validate schema compliance
2. THE System SHALL check for orphaned dependencies (references to non-existent Pearls)
3. THE System SHALL check for cycles in the dependency graph
4. THE System SHALL check for duplicate IDs
5. THE System SHALL check for invalid state transitions in the history
6. THE System SHALL report all detected issues with severity levels (error, warning, info)
7. THE System SHALL provide a `--fix` flag to automatically repair common issues

### Requirement 21: Sync Command and Remote Coordination

**User Story:** As an agent, I want a single command to synchronize with the remote repository, so that I can ensure my work is backed up and integrated with team changes.

#### Acceptance Criteria

1. WHEN the sync command is invoked, THE System SHALL execute `git pull --rebase`
2. IF merge conflicts occur, THEN THE System SHALL invoke the merge driver
3. THE System SHALL run integrity checks after the merge
4. IF integrity checks pass, THEN THE System SHALL execute `git push`
5. IF the push fails due to remote changes, THEN THE System SHALL retry the pull-merge-push cycle
6. THE System SHALL provide detailed output of each sync step
7. THE System SHALL support a `--dry-run` flag to preview sync operations

### Requirement 22: Label Management

**User Story:** As a user, I want to tag issues with labels, so that I can categorize and filter work by type, component, or priority.

#### Acceptance Criteria

1. THE System SHALL store labels as an array of strings in each Pearl
2. WHEN creating or updating a Pearl, THE System SHALL accept a list of labels
3. THE System SHALL support filtering by labels in the list command
4. THE System SHALL provide label autocomplete suggestions based on existing labels
5. THE System SHALL allow multiple labels per Pearl
6. THE System SHALL treat labels as case-insensitive for filtering but preserve original case

### Requirement 23: Priority Management

**User Story:** As a user, I want to assign priority levels to issues, so that agents can focus on the most critical work first.

#### Acceptance Criteria

1. THE System SHALL support priority levels: 0 (critical), 1 (high), 2 (medium), 3 (low), 4 (trivial)
2. WHEN creating a Pearl without specifying priority, THE System SHALL default to 2 (medium)
3. THE System SHALL allow updating priority via the update command
4. WHEN sorting the ready queue, THE System SHALL prioritize lower numeric values first
5. THE System SHALL display priority in human-readable format (e.g., "P0", "P1")

### Requirement 24: Description and Markdown Support

**User Story:** As a user, I want to write detailed issue descriptions with Markdown formatting, so that I can provide rich context for complex tasks.

#### Acceptance Criteria

1. THE System SHALL store descriptions as plain text strings supporting Markdown syntax
2. WHEN displaying descriptions, THE System SHALL preserve Markdown formatting
3. THE System SHALL support multi-line descriptions
4. THE System SHALL allow editing descriptions via the update command
5. THE System SHALL support descriptions up to 64KB in length

### Requirement 25: Author Tracking

**User Story:** As a team member, I want to track who created each issue, so that I can attribute work and coordinate with the right people.

#### Acceptance Criteria

1. WHEN creating a Pearl, THE System SHALL automatically populate the author field
2. THE System SHALL derive the author from Git configuration (user.name)
3. IF Git configuration is not available, THEN THE System SHALL use the system username
4. THE System SHALL allow overriding the author via a `--author` flag
5. THE System SHALL display the author in list and show commands

### Requirement 26: Timestamp Tracking

**User Story:** As a user, I want to track when issues were created and last updated, so that I can identify stale work and measure velocity.

#### Acceptance Criteria

1. WHEN creating a Pearl, THE System SHALL set created_at to the current Unix timestamp
2. WHEN updating a Pearl, THE System SHALL update the updated_at timestamp
3. THE System SHALL display timestamps in human-readable format (e.g., "2 days ago")
4. THE System SHALL support filtering by date ranges in the list command
5. THE System SHALL preserve timestamp precision to the second

### Requirement 27: Dependency Type Semantics

**User Story:** As a user, I want different types of dependencies with distinct semantics, so that I can model complex relationships between issues.

#### Acceptance Criteria

1. WHEN a "blocks" dependency is created, THE System SHALL enforce FSM constraints
2. WHEN a "parent_child" dependency is created, THE System SHALL establish hierarchical relationship
3. WHEN a "related" dependency is created, THE System SHALL create an informational link without FSM constraints
4. WHEN a "discovered_from" dependency is created, THE System SHALL track provenance
5. THE System SHALL display dependency types clearly in show and list commands
6. THE System SHALL allow filtering by dependency type

### Requirement 28: Partial ID Resolution

**User Story:** As a human user, I want to use short ID prefixes, so that I can type commands quickly without remembering full hash IDs.

#### Acceptance Criteria

1. WHEN a user provides a partial ID (e.g., "a1b"), THE System SHALL search for Pearls with matching ID prefixes
2. IF exactly one Pearl matches, THE System SHALL resolve to that Pearl
3. IF multiple Pearls match, THEN THE System SHALL return an error listing all matches
4. IF no Pearls match, THEN THE System SHALL return an error suggesting similar IDs
5. THE System SHALL require at least 3 characters for partial ID matching

### Requirement 29: Streaming and Large File Handling

**User Story:** As a user with a large repository, I want efficient handling of large JSONL files, so that commands remain fast even with tens of thousands of issues.

#### Acceptance Criteria

1. WHEN reading the JSONL file, THE System SHALL use streaming deserialization
2. THE System SHALL not load the entire file into memory unless necessary
3. WHEN searching for a specific Pearl, THE System SHALL stop parsing after finding it
4. THE System SHALL support parallel processing for operations on multiple Pearls
5. THE System SHALL handle files up to 1GB in size without performance degradation

### Requirement 30: Index File for Large Repositories

**User Story:** As a user with a very large repository, I want optional indexing, so that lookups remain fast even with 50,000+ issues.

#### Acceptance Criteria

1. WHERE indexing is enabled, THE System SHALL maintain a binary index file at `.pearls/index.bin`
2. THE Index_File SHALL map Hash_IDs to byte offsets in the JSONL file
3. WHEN a Pearl is created or updated, THE System SHALL update the index atomically
4. WHEN looking up a Pearl by ID, THE System SHALL use binary search on the index
5. THE System SHALL rebuild the index if it becomes corrupted or out of sync
6. THE System SHALL add the index file to `.gitignore` to prevent versioning

