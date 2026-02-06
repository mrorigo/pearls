# Pearls Issue Tracker

A lightweight, Git-native distributed issue tracking system designed for agentic workflows.

## Project Structure

This is a Cargo workspace containing four crates:

### `pearls-core`
Core library containing:
- Data models (Pearl, Dependency, Status)
- Storage engine (JSONL read/write, streaming, indexing)
- Graph algorithms (DAG, cycle detection, topological sort)
- FSM logic (state transitions, validation)
- Hash ID generation and resolution
- Error types and result handling

### `pearls-cli`
Command-line interface providing the `prl` binary with commands:
- `init` - Initialize repository
- `create` - Create new Pearl
- `show` - Display Pearl details
- `list` - List Pearls
- `ready` - Show ready queue
- `update` - Update Pearl
- `close` - Close Pearl
- `link` - Add dependency
- `unlink` - Remove dependency
- `status` - Project health check
- `sync` - Git sync operation
- `compact` - Archive old Pearls
- `doctor` - Integrity validation
- `import` - Migrate from other formats

### `pearls-merge`
Git merge driver for semantic JSONL merging.

### `pearls-hooks`
Git hook implementations:
- Pre-commit validation
- Post-merge integrity checks

## Building

```bash
cargo build
```

## Usage

Initialize a repository and create your first Pearl:

```bash
prl init
prl create "Add search index" --priority 1 --label storage,performance
```

Common workflows:

```bash
# List open Pearls sorted by most recent update
prl list --status open --sort updated_at

# Show a Pearl by full or partial ID
prl show prl-abc123
prl show abc

# Add a blocking dependency
prl link prl-abc123 prl-def456 blocks

# Update a Pearl
prl update prl-abc123 --status in_progress --add-label urgent

# Archive closed Pearls older than 30 days
prl compact --threshold-days 30
```

See `examples/WORKFLOW.md` for a longer sample workflow.

## Testing

```bash
cargo test
```

## Development

This project follows the [Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/). See `AGENTS.md` for detailed coding standards.

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check compilation
cargo check
```

## License

MIT OR Apache-2.0
