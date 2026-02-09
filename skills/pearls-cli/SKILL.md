---
name: pearls-cli
description: "Operate Pearls (`prl`) for Git-native issue tracking: initialize `.pearls`, create/update/list/show/close/compact issues, manage dependencies, labels, comments, and metadata, use the ready queue, run diagnostics/status/sync, and troubleshoot hooks/merge driver/config. Use when interacting with Pearls "
---

# Pearls CLI

## Use these references
- Read `references/user-guide.md` for full command syntax, flags, and examples.
- Read `references/architecture.md` for details on the graph model, FSM, and merge driver behavior.

## Core workflow
1. Ensure the repo is initialized: `git init` then `prl init`.
2. Create issues with titles and optional labels, priority, and description.
3. Link dependencies with a type; use `prl ready` to pick unblocked work.
4. Update status and fields; close when done.
5. Compact old closed issues to archive.

## Basic commands (copy/paste)
```bash
git init
prl init

prl create "Add cache invalidation"  # Creates an issue with default priority and no labels, status "open", and no description.
prl create "Improve ready queue" --priority 0 --label graph,perf
prl create "Add import support" --description "Supports Beads JSONL import."

prl list --status open --sort updated_at
prl list --label storage,perf
prl show prl-abc123
prl show abc

prl update prl-abc123 --status in_progress
prl update prl-abc123 --add-label urgent --remove-label backlog
prl close prl-abc123

prl link prl-abc123 prl-def456 blocks
prl ready --limit 5  # Use this to find work to do.

prl comments add prl-abc123 "Needs integration test coverage"
prl meta set prl-abc123 owner "alice"

prl doctor  # Run diagnostics to check for issues if `prl` fails.
prl status --detailed
```

## Command map
- Initialize and hooks: `prl init`, `prl hooks pre-commit`, `prl hooks post-merge`.
- Create and update: `prl create`, `prl update`, `prl close`.
- Inspect and search: `prl list`, `prl show`, `prl ready`.
- Dependencies: `prl link`, `prl unlink`.
- Comments and metadata: `prl comments add|list|delete`, `prl meta set|get`.
- Archive and maintenance: `prl compact`, `prl doctor`, `prl status`, `prl sync`.

## Operational rules
- Always use `prl` commands over manual edits to `.pearls/issues.jsonl`.
- Use at least 3 characters for IDs; expand on ambiguity.
- Treat `blocks` dependencies as strict gates for `in_progress` and `closed`.
- Use `prl doctor` to validate JSONL, schema, and graph integrity when errors arise.
- Use `prl status` for health checks before handoff.

## Git integration
- Configure the merge driver in local Git config: `git config merge.pearls.driver "prl merge %O %A %B"`.
- Hooks live in `.git/hooks`; reinstall via `prl init` if missing.
- Ensure `prl` is on PATH for non-interactive hooks.

## Files and config
- Data lives in `.pearls/issues.jsonl`; archived items in `.pearls/archive.jsonl`.
- Config lives at `.pearls/config.toml`; environment overrides include `PEARLS_DEFAULT_PRIORITY` and related keys.
- Use `prl list --format json` for machine-readable output.
