<img src="docs/logo.png">

# Pearls

Git-native issue tracking for the age of agentic development.

Pearls is a fast, local-first issue tracker that lives in your repo, speaks in structured data, and behaves like a serious engineering tool. No cloud lock-in. No daemon circus. No mystery state.

## Why Pearls

- `Local-first`: issue state lives with your code in `.pearls/issues.jsonl`
- `Agent-ready`: strict schema, dependency graph, finite-state transitions
- `Git-native`: merge driver + hooks for real workflows
- `Single binary`: `prl` is all users need in their PATH
- `Small`: example macOS release binary is ~4.4MB (Beads `bd` is ~31MB on the same platform; sizes vary by target and build flags)
- `Rust-fast`: optimized for tight human and agent feedback loops

If markdown TODOs feel too fuzzy and SaaS trackers feel too heavy, Pearls is the middle path that actually scales.

## Install

Fastest install:

```bash
cargo install --git https://github.com/mrorigo/pearls pearls-cli
```

Install from local checkout:

```bash
cargo install --path /path/to/pearls/crates/pearls-cli
```

Ensure `prl` is on PATH so Git hooks and merge drivers can invoke it.

## 60-Second Start

```bash
git init
prl init

prl create "Ship launch page" --priority 1 --label marketing,web
prl create "Wire merge driver docs" --priority 2 --label docs

prl list --status open --sort updated_at
prl ready
```

## Core Commands

- `prl init`: initialize `.pearls`, hooks, and Git merge integration
- `prl create`, `prl update`, `prl close`: lifecycle operations
- `prl list`, `prl show`, `prl ready`: discovery and execution flow
- `prl link`, `prl unlink`: dependency management
- `prl meta`: structured per-issue metadata
- `prl doctor`: integrity checks and optional repairs
- `prl compact`: archive old closed issues
- `prl sync`: Git sync workflow helper
- `prl hooks`: run hook actions directly
- `prl merge`: merge-driver entrypoint for JSONL conflicts

## Documentation

- User guide: `docs/USER-GUIDE.md`
- Whitepaper: `docs/WHITEPAPER.md`
- Deep architecture: `docs/PEARLS.md`
- End-to-end sample: `examples/WORKFLOW.md`

## Project Layout

This workspace ships four crates:

- `crates/pearls-core`: models, storage, graph, FSM, identity
- `crates/pearls-cli`: `prl` command-line application
- `crates/pearls-merge`: semantic JSONL merge engine
- `crates/pearls-hooks`: pre-commit and post-merge validations

## Build and Test

```bash
cargo check
cargo fmt
cargo clippy -- -D warnings
cargo test
```

## Engineering Standards

Pearls follows the [Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/). See `AGENTS.md` for repository-specific coding standards.

## License

MIT
