# Pearls User Guide

This guide covers everyday usage, repository setup, and Git integration details for Pearls.

## Quick Start

Install `prl` (fastest):

```bash
cargo install --git https://github.com/mrorigo/pearls pearls-cli
```

Initialize Pearls in your repository:

```bash
git init
prl init
```

Create your first Pearl:

```bash
prl create "Add cache invalidation" --priority 1 --label perf,backend
```

List open Pearls:

```bash
prl list --status open --sort updated_at
```

Show details for a Pearl (full or partial ID):

```bash
prl show prl-abc123
prl show abc
```

## Concepts

Pearls are issues with:
- A stable ID (`prl-xxxxxx`)
- A status (Open, InProgress, Blocked, Deferred, Closed)
- Priority (`P0` to `P4`)
- Labels, dependencies, metadata
- Author and timestamps

Everything is stored in `.pearls/issues.jsonl`, one JSON object per line.

## What `prl init` Produces

When you run `prl init`, Pearls sets up the following:

```
.pearls/
  issues.jsonl
  config.toml
.gitattributes
.git/
  hooks/
    pre-commit
    post-merge
```

### `.pearls/issues.jsonl`

This is the main data file. Each line is a JSON object representing a Pearl.

### `.pearls/config.toml`

The configuration file with defaults you can tune:

```toml
default_priority = 2
compact_threshold_days = 30
use_index = false
output_format = "table"
auto_close_on_commit = false
```

### `.gitattributes`

Pearls adds merge rules for JSONL files:

```
issues.jsonl merge=pearls
archive.jsonl merge=pearls
```

This tells Git to use the Pearls merge driver for JSONL files.

### Git Hooks

Pearls installs hooks into `.git/hooks`:

`pre-commit` validates JSONL integrity and optionally auto-closes Pearls based on commit messages.

`post-merge` validates dependency integrity after a merge.

The default hook scripts look like:

```
#!/bin/sh
prl hooks pre-commit
```

```
#!/bin/sh
prl hooks post-merge
```

## Git Integration (Merge Driver and Hooks)

### Merge Driver

Pearls includes a custom merge driver in the `pearls-merge` crate. The merge driver:
- Preserves all Pearls present in both branches
- Merges compatible changes
- Writes conflict markers for incompatible edits

The `prl init` command creates `.gitattributes`, but Git also needs a merge driver definition. Add this to your local repo config:

```bash
git config merge.pearls.name "Pearls JSONL merge driver"
git config merge.pearls.driver "prl merge %O %A %B"
```

If you want this available in all repositories, use `--global`.

### Hooks

Hooks are local to each clone. If your team wants consistent hooks:
- Keep a copy of the hook scripts in your repo (for example, `scripts/hooks/`)
- Ask developers to copy them into `.git/hooks`

You can re-run `prl init` to reinstall default hooks if they are missing.

If hooks fail to run, ensure `prl` is available on the PATH for non-interactive Git hooks.

## Creating Pearls

Basic:

```bash
prl create "Refactor storage layer"
```

With labels and priority:

```bash
prl create "Improve ready queue" --priority 0 --label graph,perf
```

With description:

```bash
prl create "Add import support" --description "Supports Beads JSONL import."
```

From file or stdin:

```bash
prl create "Spec update" --description-file notes.md
cat notes.md | prl create "Spec update" --description-file -
```

Override author:

```bash
prl create "Hotfix for parser" --author alice
```

## Updating Pearls

Update title, description, priority, status, and labels:

```bash
prl update prl-abc123 --title "New title"
prl update prl-abc123 --description "New description"
prl update prl-abc123 --description-file details.md
prl update prl-abc123 --priority 2
prl update prl-abc123 --status in_progress
prl update prl-abc123 --add-label urgent --remove-label backlog
```

## Listing Pearls

Filter by status, priority, labels, author, dependency type, and timestamps:

```bash
prl list --status open
prl list --priority 1
prl list --label storage,perf
prl list --author alice
prl list --dep-type blocks
prl list --created-after 1700000000 --created-before 1800000000
prl list --updated-after 1700000000 --updated-before 1800000000
```

Include archived items:

```bash
prl list --include-archived
```

Sort results:

```bash
prl list --sort updated_at
prl list --sort priority
```

## Showing Details

Show a Pearl:

```bash
prl show prl-abc123
```

Search archived items too:

```bash
prl show prl-abc123 --include-archived
```

## Dependencies

Add dependencies with a type:

```bash
prl link prl-abc123 prl-def456 blocks
prl link prl-abc123 prl-def456 related
```

Remove dependencies:

```bash
prl unlink prl-abc123 prl-def456
```

Dependency types:
- `blocks`: target must be closed before progress
- `parent_child`: hierarchical relationship
- `related`: informational
- `discovered_from`: provenance

## Ready Queue

Show unblocked work, ordered by priority and recency:

```bash
prl ready
prl ready --limit 5
```

## Closing Pearls

```bash
prl close prl-abc123
```

## Metadata

Store JSON metadata:

```bash
prl meta set prl-abc123 owner \"alice\"
prl meta set prl-abc123 estimate 3
prl meta get prl-abc123 owner
```

## Archiving

Archive closed Pearls older than a threshold:

```bash
prl compact --threshold-days 30
```

Dry run:

```bash
prl compact --threshold-days 30 --dry-run
```

Archived Pearls are moved to `.pearls/archive.jsonl`.

## Diagnostics

Doctor validates JSONL, schema, and graph integrity:

```bash
prl doctor
prl doctor --fix
```

Status provides a quick health check:

```bash
prl status
prl status --detailed
```

## Sync

Sync with a Git remote:

```bash
prl sync
prl sync --dry-run
```

## Output Formats and Timestamps

JSON output:

```bash
prl list --format json
```

Shorthand:

```bash
prl list --json
```

Plain output:

```bash
prl list --format plain
```

Absolute timestamps:

```bash
prl list --absolute-time
prl show prl-abc123 --absolute-time
```

## Configuration

The config file lives at `.pearls/config.toml`.

Key options:
- `default_priority` (0-4)
- `compact_threshold_days`
- `use_index`
- `output_format` (`json`, `table`, `plain`)
- `auto_close_on_commit`

Environment overrides:
- `PEARLS_DEFAULT_PRIORITY`
- `PEARLS_COMPACT_THRESHOLD_DAYS`
- `PEARLS_USE_INDEX`
- `PEARLS_OUTPUT_FORMAT`
- `PEARLS_AUTO_CLOSE_ON_COMMIT`

## Import and Migration

Import Pearls from a Beads JSONL file:

```bash
prl import path/to/beads.jsonl
```

Invalid lines are skipped with a warning.

## Troubleshooting

Common issues:
- Missing `.pearls` directory: run `prl init`
- Ambiguous IDs: use more characters (minimum 3)
- Status transition errors: check blockers and FSM rules
- Hooks not running: verify scripts exist in `.git/hooks`

If a command fails, re-run with a more specific ID or check `prl doctor` output.
