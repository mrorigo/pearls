# Pearls Technical Whitepaper

## Abstract

Pearls is a Git-native, distributed issue tracking system that stores all issue data as JSON Lines (JSONL) within a repository. It provides strong offline support, deterministic merges, and low operational overhead by avoiding centralized services and complex databases. This whitepaper describes the system architecture, data model, merge behavior, integrity guarantees, and performance characteristics.

## Background and Motivation

Modern agentic development workflows demand low-latency, local-first memory. Traditional issue trackers (hosted services or heavy local databases) impose network dependencies, operational overhead, and context drift between code and planning artifacts. Teams often fall back to ad-hoc markdown checklists, which are human-friendly but lack machine-readable structure, leading to ambiguous dependencies and unreliable automation.

Pearls draws inspiration from the "Beads" system, which proved that Git-backed issue tracking with graph semantics dramatically improves long-horizon execution. However, Beads relies on a background daemon and a dual-storage model (JSONL plus SQLite), introducing synchronization latency, binary artifacts, and process management complexity. Pearls removes those layers in favor of a single, canonical JSONL file that is easy to audit, diff, and merge.

Rust performance makes this simplification practical. Streaming JSON deserialization and efficient graph algorithms allow real-time parsing for typical repository sizes, eliminating the need for always-on services or cache databases. The result is a system aligned with Git workflows, resilient to offline operation, and optimized for both human and agent usage.

## Design Principles

- **Local-First by Default**: All state lives in the repository and remains usable offline.
- **Single Canonical Source**: JSONL is the source of truth; indexes are optional and rebuildable.
- **Graph-First Semantics**: Dependencies define execution order and FSM constraints.
- **Deterministic Merges**: A semantic merge driver protects JSONL integrity.
- **Operational Minimalism**: Avoid daemons, services, and background sync.
- **Strict Validation**: Invalid transitions and malformed data are rejected early.

## Rust Conventions and Standards

Pearls adheres to the Microsoft Pragmatic Rust Guidelines:
- Panics are reserved for unrecoverable situations.
- `unsafe` is avoided unless required, with safety notes when used.
- Public APIs are documented with clear sections and usage guidance.
- Recoverable errors use `Result<T, E>` with `thiserror` for libraries and `anyhow` for binaries.

## Goals

- Local-first issue tracking with no external service dependency
- Human-readable storage format with a single source of truth
- Deterministic merge behavior aligned with Git workflows
- Strong integrity checks for dependency graphs and state transitions
- Scalable performance for large repositories

## Architecture Overview

Pearls is composed of four Rust crates:
- `pearls-core`: Core models, storage, graph, FSM, identity
- `pearls-cli`: User interface, commands, output formatting
- `pearls-merge`: Custom Git merge driver for JSONL (invoked via `prl merge`)
- `pearls-hooks`: Git hooks for validation and auto-close behavior

All state lives in `.pearls/issues.jsonl`, one Pearl per line, with archived Pearls in `.pearls/archive.jsonl`.

## Data Model

Each Pearl is a JSON object with mandatory fields:
- `id`: Hash-based identifier
- `title`
- `status`
- `created_at`
- `updated_at`
- `author`

Optional fields:
- `description`
- `priority`
- `labels`
- `deps` (dependency array with `target_id` and `dep_type`)
- `metadata` (arbitrary JSON values)

The schema is stable, forward-compatible, and tolerant of unknown fields.

## Identity and ID Resolution

Pearl IDs are derived from a SHA-256 hash of:
- Title
- Author
- Timestamp
- Nonce

The resulting hash is truncated and prefixed with `prl-`. Partial IDs are resolved by prefix matching (minimum 3 characters) with ambiguity detection.

## Storage Format and Guarantees

Pearls are stored in JSONL:
- One JSON object per line
- Streaming read for large files
- Atomic writes for single Pearl updates and bulk rewrites
- Optional binary index (`.pearls/index.bin`) for fast lookups

Storage operations provide:
- Single-line serialization guarantee
- Locking for concurrent writes
- Atomicity via temporary file + rename

## Dependency Graph and FSM

Pearls can declare dependencies with types:
- `blocks`: enforces FSM constraints
- `parent_child`: hierarchical relationship
- `related`: informational link
- `discovered_from`: provenance

The graph is maintained as a directed acyclic graph (DAG) with cycle detection. The FSM enforces legal state transitions and rejects progress if blocking dependencies are open.

## Git Integration

Pearls integrates with Git through:
- A merge driver for JSONL that preserves all Pearls and detects conflicts
- Pre-commit hook validating JSONL integrity and auto-closing based on commit messages
- Post-merge hook verifying dependency integrity

Merge logic favors preserving user data and producing conflict markers when necessary.

## CLI Design

The CLI is intentionally minimal and consistent. It supports:
- Creation, update, and closure
- Filtering and sorting
- Archive management
- Import/migration
- Diagnostics and integrity checks

Output is available in table, plain, or JSON formats, and timestamps can be displayed in relative or absolute format.

## Performance Characteristics

Pearls is designed for large repositories:
- Streaming deserialization for low memory usage
- Parallel filtering for list operations
- Optional index for fast lookup
- Benchmarks for load and graph operations

Binary size is intentionally modest. As an example, the macOS release `prl` binary is ~4.4MB, compared to ~31MB for Beads `bd` on the same platform (sizes vary by target, build flags, and symbol stripping).

Target performance for 1000 Pearls:
- `load_all`: <10ms
- `topological_sort`: <5ms
- `create`: <5ms (includes disk I/O)
- `ready_queue`: <15ms

## Reliability and Safety

Pearls follows the Microsoft Pragmatic Rust Guidelines:
- No panics for recoverable errors
- Minimal unsafe code (none in core paths)
- Robust error handling with clear messages
- Structured logging in production components

## Security Model

Pearls does not introduce a network surface by default. The system relies on:
- Local filesystem integrity
- Git history and signatures where applicable
- Explicit hooks for validation

Any external integration should be treated as a separate trust boundary.

## Extensibility

The `metadata` field allows tool-specific extensions without schema changes. New CLI functionality can be added without breaking file format compatibility.

## Limitations

- JSONL merges can still produce conflicts when the same Pearl is edited in multiple branches.
- Large repositories can benefit from indexing and parallel operations.
- Git hooks are local to the developer environment unless distributed and installed.

## Conclusion

Pearls provides a pragmatic, local-first issue tracker that fits naturally into Git workflows. By leveraging JSONL, Rust, and careful merge logic, it offers a lightweight yet reliable solution for issue tracking in distributed, agentic development environments.
