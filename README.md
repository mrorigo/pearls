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
