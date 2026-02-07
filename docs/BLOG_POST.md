# Pearls: A 12-Hour Sprint from Research Prompt to Release-Ready Tooling

This post is an account of the last 12-ish hours of building Pearls: a Git-native issue tracker designed for agentic workflows, built in Rust, with a single JSONL source of truth.

It starts with a research prompt, turns into a spec, becomes a working implementation, and finishes with the kind of release polish that only happens when a human keeps steering and an agent keeps shipping.

## Article Outline (Detailed)

1. Hook
   - Why "agentic development" needs different tooling than Jira tabs and markdown TODOs.
   - The elevator pitch: Pearls is Git-native issue tracking with graph semantics, no daemon, no service, one binary.

2. The Origin Story: A Research Prompt Becomes a North Star
   - The Gemini deep research prompt: "Building a Lightweight Git Workflow Tool, inspired by Beads, but lightweight and in Rust."
   - The artifact it produced: `docs/PEARLS.md` as a long-form architecture memo.
   - Why that mattered: writing down the "why" first stopped scope drift later.

3. Specs: Let Kiro Do What Kiro Does
   - Turn the architecture memo into structured specs in `.kiro/specs/pearls/`.
   - What the spec set contained:
     - `design.md` (architecture and invariants)
     - `requirements.md` (acceptance criteria)
     - `tasks.md` (incremental plan)
   - The real win: a task plan that is testable and checkable.

4. Implementation Phase 1: The Big Core Commit
   - What went in early: workspace layout, core models, storage, graph, FSM, CLI skeleton, tests.
   - Emphasis on "boring correctness": JSONL invariants, partial ID resolution, lock discipline, property tests.

5. Implementation Phase 2: Switching Tools (Codex App)
   - Why the switch mattered: faster iteration, bigger refactors, more aggressive polish.
   - What got finished and what got tightened.

6. The "Release-Ready" Gap (Where Humans Still Matter)
   - Hooks and merge drivers are the classic trap.
   - Agents will happily write something that works locally, but shipping requires:
     - the right binary invocation (`prl` not `cargo run`)
     - PATH realities for Git hooks
     - accurate docs and UX messages
     - removing accidental `unwrap()` in production paths
     - benchmarks that measure the right thing

7. The Final Checklist
   - Full `cargo test` run across crates.
   - Benchmarks and target adjustments (disk I/O is real).
   - Docs sweep: user guide, whitepaper, architecture memo, landing page, README.

8. Lessons Learned
   - Specs are a multiplier, not ceremony.
   - Property tests are an agent accelerator.
   - The human job is not "typing"; it is steering, taste, and correctness gates.

9. Appendix
   - A timestamped timeline grounded in commits and file mtimes.

## The Timeline (Backed by Commits and File Stamps)

All times below are local repository commit times (timezone `+0100`), with a couple file mtimes noted where helpful.

- 2026-02-06 21:19:12: "initial commit: plans" (`a0e46c4`)
  - Added the Kiro spec set in `.kiro/specs/pearls/` and the first version of `docs/PEARLS.md`.
- 2026-02-06 21:40:27: "AGENTS.md - Pragmatic Rust Guidelines" (`41e6ea8`)
  - Locked in Rust coding standards and panic policy early.
- 2026-02-06 22:50:57: "progress" (`0af41ed`)
  - The big foundational implementation landed: multi-crate workspace, core engine, CLI baseline, tests.
- 2026-02-06 23:51:43: "Implement tasks 18-23" (`9b3bcfc`)
  - Major feature expansion: compact/doctor/import/link/unlink/status/sync, hooks and merge behavior, more tests.
- 2026-02-07 00:10:55: "Finish remaining tasks 24-28" (`ed285e7`)
  - Author and timestamp features, archive support, performance work, progress reporting, benchmarks, docs/examples, E2E tests.
- 2026-02-07 08:20:03: "Review release readiness and docs" (`116acce`)
  - Wrapper subcommands (`prl hooks`, `prl merge`), doc improvements, lock retry behavior, merge-driver refactor.
  - `docs/WHITEPAPER.md` mtime: 07:48:14, `docs/USER-GUIDE.md` mtime: 08:04:52, `docs/PEARLS.md` mtime: 08:05:01.
- 2026-02-07 08:24:01: "Assess release readiness" (`71650db`)
  - A small correctness pass (example: remove a production `unwrap()` path).
- 2026-02-07 08:26:20: "added LICENSE file" (`4bfaa75`)
  - Also introduced the landing page scaffold: `docs/index.html` and `docs/logo.png`.
  - `docs/index.html` continued to evolve after that commit (mtime later in the morning).

That is the mechanical story. Here is the human story.

## The Origin Story: The Prompt That Started It

This build did not start with a blank `main.rs`. It started with a deep research prompt to Gemini, something like:

"Building a Lightweight Git Workflow Tool, based on Beads ideas, but light-weight and in Rust."

The output from that prompt became `docs/PEARLS.md`. That file did the thing architecture docs are supposed to do:

- it made the intent explicit (Git-native, JSONL, no daemon)
- it made the constraints explicit (graph semantics, typed dependencies, FSM)
- it clarified what we were not doing (SQLite caches, background processes, a server)

The practical effect is underrated: when the agent tries to "helpfully" invent features later, you can point at a document that says "no, the whole point is we do less, but do it well."

## Specs: Kiro Turned Narrative into a Plan

From `docs/PEARLS.md`, we used Kiro to generate a more operational spec set:

- `.kiro/specs/pearls/design.md`
- `.kiro/specs/pearls/requirements.md`
- `.kiro/specs/pearls/tasks.md`

That last file, `tasks.md`, is the difference between "a cool repo" and "a shippable product." It breaks work down into incremental steps that can be tested and checked off.

Agents are good at sprinting. They are worse at discovering what "done" means. Specs define "done."

## Implementation: The First Big Shape of the Tool

The early implementation push (commit `0af41ed` at 22:50) laid down the system spine:

- A Rust workspace with focused crates (`pearls-core`, `pearls-cli`, `pearls-merge`, `pearls-hooks`).
- A strict data model with timestamps, author, labels, dependencies, metadata.
- Storage built around JSONL, atomic updates, and file locking.
- A dependency graph with cycle detection and a ready queue.
- FSM validation so agents cannot "close" work that is still blocked.
- A CLI that makes the "happy path" cheap.
- Property tests and unit tests to keep the invariants honest.

This is where you want an agent writing code: large volumes of correct, consistent scaffolding, following a plan.

## Switching to the Codex App: Finishing the Job

Later in the evening, we moved into the (brand new) Codex App to finish the remaining tasks and do the unpleasant-but-necessary work:

- Git integration details (hooks, merge driver, repo config)
- Archive behavior and queries
- Output formatting, timestamps, and filters
- Progress reporting for long operations
- Benchmarks and performance sanity
- Docs that match reality

The best part of agentic coding is momentum. The dangerous part is momentum. The Codex App helped us keep speed while still doing careful review loops.

## The Release-Ready Gap: Humans Still Have to Steer

Here is the key takeaway of the last 12 hours:

An agent will build you a working program. It will not automatically build you a releasable product.

That gap is full of small, sharp details. A few that came up here:

- Git hooks and merge drivers cannot depend on `cargo run`.
  - You need a stable, global entrypoint: `prl`.
  - We added `prl hooks pre-commit`, `prl hooks post-merge`, and `prl merge %O %A %B` so a user only needs one binary in PATH.
- Git integration needs real Git config changes.
  - `prl init` now configures `merge.pearls.name` and `merge.pearls.driver` via libgit2.
  - It also requires a Git repo to exist (`git init`), and docs must say that.
- Correctness is not just algorithms; it is operational behavior.
  - A concurrent write test failed because the lock acquisition did not retry.
  - Fixing that meant implementing a small backoff loop with a timeout.
- Documentation has to match implementation.
  - If docs say blocked status is auto-updated, but code does not do that, either the docs or the code is wrong.
  - Someone (human) has to decide which reality is intended.
- Benchmarks need interpretation.
  - Our `create_pearl` benchmark includes disk I/O. It is not a pure CPU benchmark.
  - We adjusted expectations accordingly, because shipping software is allowed to acknowledge physics.

These are not glamorous problems. They are the problems that decide whether a tool feels trustworthy.

## Final Release Checklist

By the end of the sprint:

- Full `cargo test` across crates ran clean (unit tests, property tests, integration tests, doc-tests).
- Benchmarks ran for `pearls-core` with results in the sub-millisecond range for the core operations.
- Docs were expanded and aligned:
  - `docs/USER-GUIDE.md` (workflows, init outputs, Git setup)
  - `docs/WHITEPAPER.md` (background and principles)
  - `docs/PEARLS.md` (deep architecture narrative)
  - `docs/index.html` (landing page)
  - `README.md` (onboarding that welcomes humans)

In other words: it is not just "working." It is presentable.

## What This Says About Human-Agent Collaboration

If you strip away the hype, the healthy division of labor looks like this:

- The agent is a multiplier for throughput.
  - It is excellent at implementing a spec and plumbing consistent patterns across a codebase.
- The human is a multiplier for direction.
  - You choose the product constraints (one binary, no daemon).
  - You notice the footguns (hooks invoking `cargo run`).
  - You hold the line on quality (no stray `unwrap()` in production).
  - You decide what "release-ready" means (tests, docs, benchmarks, UX).

The idea did not invent itself. The spec did not appear by magic. The release polish did not happen by accident.

Agents are powerful. They are not sovereign.

## Closing

Pearls ended up as something that is both fun and boring in the best way: fun in its ambition (agent-ready, Git-native), boring in its mechanics (predictable, testable, documented).

That combination is what makes a tool feel like it belongs in a real engineering workflow.

If you want the full "why" story, start with `docs/PEARLS.md`.
If you want to actually use it, start with `docs/USER-GUIDE.md`.

