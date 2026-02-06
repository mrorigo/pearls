# Pearls: A Lightweight, Git-Native Distributed Issue Tracking Architecture for Agentic Workflows

## 1. Executive Summary and Architectural Philosophy

The integration of Large Language Model (LLM) agents into the software development lifecycle has created a fundamental crisis in project management tooling. Traditional issue trackers—such as Jira, Linear, or GitHub Issues—impose latency and context penalties that make them unsuitable for the high-frequency, autonomous loops of modern coding agents. The "Beads" system emerged as a pioneering solution, offering a distributed, graph-based issue tracker backed by Git and demonstrating that providing agents with a structured, local-first memory store significantly enhances their ability to execute long-horizon tasks.

However, Beads introduces operational complexity through its reliance on a background daemon, a dual-storage model (SQLite caching alongside JSONL), and associated synchronization overhead. For development environments prioritizing minimalism, strict portability, and "zero-infrastructure" overhead, maintaining a background process and binary database artifacts constitutes significant friction.

This report presents the architectural blueprint for "Pearls," a streamlined successor to Beads designed specifically for the Rust ecosystem. Pearls retains critical semantic capabilities—the dependency-aware graph, strict finite state machine (FSM), and hash-based identity system—while radically simplifying the persistence layer. By mandating a "JSONL-only" storage strategy and leveraging Rust's high-performance characteristics for real-time parsing, Pearls eliminates the need for SQLite and background daemons. This architecture ensures the "source of truth" remains a single, human-readable text file seamlessly integrated with Git's version control mechanisms.

## 2. The Contextual Crisis in Agentic Development

To understand the necessity of Pearls, one must first analyze the deficiencies of current tooling in the context of agentic workflows. The interaction pattern of an AI agent differs fundamentally from that of a human developer. While humans rely on visual dashboards and can tolerate seconds of latency, agents operate via API calls where every token of input costs money and consumes finite context window capacity.

### 2.1 The Markdown Ceiling and the Dementia Problem

In the absence of specialized tooling, both human developers and early-stage agents default to tracking work in markdown files (e.g., TODO.md, plans/feature-x.md). While markdown offers excellent human readability and version control compatibility, it lacks semantic structure. An agent parsing a markdown file must use heuristic reasoning to understand that indentation represents dependencies. This heuristic extraction is error-prone and token-expensive.

The "Dementia Problem" emerges as projects scale: the number of markdown plans proliferates, leading to fragmented state where agents cannot distinguish between active tasks, abandoned brainstorms, and completed work. Without a formal schema, agents might mark tasks as "done" despite incomplete prerequisites or hallucinate task statuses that don't exist. This unstructured memory failure mode prevents agents from reliably executing tasks spanning multiple sessions or days.

### 2.2 The Latency of Centralized Trackers

Traditional issue trackers provide structure but fail on locality and latency. For an agent to check the status of a GitHub Issue, it must perform network requests, parse complex JSON API responses, and handle authentication secrets. This introduces friction cost. Furthermore, centralized trackers are often decoupled from code version history. If an agent checks out an old branch to fix a regression, the issue tracker does not automatically revert to the project's state at that time, leading to context mismatch where code and plans diverge.

### 2.3 The Requirements for Agentic Memory

A viable agentic memory system must satisfy four non-negotiable requirements:

- **Locality**: Data must reside in the repository (Git-backed) to ensure it travels with the code and supports offline operation.
- **Structure**: Data must adhere to a strict schema (JSON/JSONL) to enable type-safe interaction and prevent hallucinations.
- **Graph Awareness**: The system must enforce dependencies (A blocks B) to guide execution order.
- **Zero-Conflict Identity**: The system must support concurrent modifications by multiple agents without ID collisions (hash-based IDs).

Pearls is designed to satisfy these requirements with absolute minimum architectural weight.

## 3. Deconstructing Beads: The Predecessor Analysis

Beads represents the current state-of-the-art in this domain. Analysis of its architecture reveals both the features that must be retained and the complexities that Pearls aims to eliminate.

### 3.1 The Dual-Storage Architecture

Beads employs a hybrid storage model to balance performance with Git compatibility. The "source of truth" is stored in `.beads/issues.jsonl`, a text-based append-log friendly to Git versioning. However, to support fast queries, Beads maintains a "shadow" database in SQLite (`.beads/beads.db`).

| Component | Function | Persistence | Pros | Cons |
|-----------|----------|-------------|------|------|
| `issues.jsonl` | Canonical Record | Git Committed | Git-friendly, diffable | Slow to parse linearly at scale |
| `beads.db` | Query Cache | Local/Ignored | SQL joins, lookups | Binary bloat, sync drift |
| Daemon | Sync Manager | Process | Auto-syncs DB/JSONL | Complexity, resource usage |

### 3.2 The Daemon Tax

The background daemon in Beads is responsible for "invisible infrastructure." It watches the file system for changes to the SQLite database and exports them to JSONL, and conversely, watches the JSONL file for changes (from git pull) to import them into SQLite. While intended to be transparent, this introduces significant complexity:

- **Synchronization Latency**: A configurable debounce window (e.g., 30 seconds) batches writes, creating periods where disk state and Git state are inconsistent.
- **Process Management**: The daemon requires PID files, lock files (daemon.lock), and socket management (bd.sock), all of which can fail, leaving orphan daemons or locked databases.
- **Platform Friction**: Implementing robust background processes across Linux, macOS, and Windows requires non-trivial code and OS-specific handling.

### 3.3 Graph Logic and Semantics

Despite infrastructure weight, the logical model of Beads is highly effective. It treats issues as nodes in a graph with typed edges. The core insight is that agents do not need a list of everything; they need a list of what is ready. The `bd ready` command performs a topological sort to surface unblocked leaf nodes, drastically reducing cognitive load on the agent. This logical model is the "crown jewel" that Pearls must preserve.

## 4. The Pearls Paradigm: Architectural Simplification

Pearls defines itself by what it removes. By eliminating the SQLite cache and background daemon, Pearls reduces the architecture to a single executable interacting with a single data file. This simplification is made possible by Rust's performance characteristics and modern hardware, which allow real-time processing of JSONL files that would historically have required database indexing.

### 4.1 The "No-Daemon" Manifesto

In Pearls, there is no background process. Every state change is atomic and synchronous. When a user or agent runs `prl create`, the tool:

1. Locks the `.pearls/issues.jsonl` file.
2. Reads and parses the necessary context.
3. Appends the new record.
4. Releases the lock.

This "CLI-only" approach aligns with the Unix philosophy of simple, composable tools. It eliminates bugs related to daemon crashes, socket permissions, and stale caches, and simplifies the mental model: if the command returns success, the data is on disk.

### 4.2 Rust as the Enabler

The feasibility of this architecture hinges on deserialization speed. The Rust serde_json crate can parse JSON at rates exceeding 400 MB/s. For a hypothetical project with 10,000 active issues (averaging 500 bytes each), the total dataset is 5 MB. Rust can load, parse, and graph this dataset in roughly 10-20 milliseconds—below human perception and negligible compared to LLM API latency. Thus, the SQLite cache in Beads is architecturally redundant for Pearls' target scale (projects with <100,000 issues).

## 5. Storage Engine Design: The JSONL Monolith

JSONL (JSON Lines) is central to the Pearls design. Unlike monolithic JSON files (arrays of objects), JSONL stores one object per line, critical for Git integration as it allows line-based diffs and reduces merge conflicts compared to pretty-printed JSON.

### 5.1 Schema Definition

The Pearls schema is strict to ensure type safety for the FSM. Each line in `issues.jsonl` corresponds to a Pearl struct serialized to JSON.

| Field | Type | Description | Requirement |
|-------|------|-------------|-------------|
| `id` | String | Hash-based unique identifier (e.g., prl-7f2a) | Mandatory, immutable |
| `title` | String | One-line summary of the task | Mandatory |
| `description` | String | Full markdown-supported body | Optional |
| `status` | Enum | open, in_progress, blocked, deferred, closed | Mandatory |
| `priority` | Int | 0 (Critical) to 4 (Trivial) | Default: 2 |
| `created_at` | Int | Unix timestamp (UTC) | Mandatory |
| `updated_at` | Int | Unix timestamp (UTC) | Mandatory |
| `author` | String | Git user name or Agent ID | Mandatory |
| `labels` | Array | List of string tags (e.g., ["bug", "frontend"]) | Default: [] |
| `deps` | Array | List of Dependency objects | Default: [] |
| `metadata` | Map | Flexible K-V store for agent usage | Optional |

### 5.2 Persistence Strategy: Append-Only vs. Compaction

Pearls employs a "Log-Structured" persistence strategy:

- **Append-Only Logic**: For auditability, new updates can be appended to the file, creating a history of changes (Event Sourcing lite). However, infinite appending bloats the file and slows parsing.
- **Snapshot Logic (Default)**: Pearls defaults to a "rewrite on update" strategy. When a status changes, the corresponding line is rewritten. To preserve history without bloating the main file, previous state is written to a separate `history.jsonl` file (which can be git-ignored or log-rotated) or relies on Git's own reflog.
- **Optimization**: The most idiomatic Rust approach for Pearls is to maintain the file as a "Current State Snapshot." Since Git already tracks every version of the file, relying on Git for the audit trail (via `git blame` or `git log -p .pearls/issues.jsonl`) avoids duplicating version control logic inside the application.

### 5.3 Schema Evolution

To support future changes without breaking older CLI versions, the schema utilizes Rust's `#[serde(default)]` and `#[serde(rename_all = "camelCase")]` attributes. New fields added in future versions will have default values when read by older binaries (forward compatibility), and unknown fields will be preserved or ignored depending on strictness settings.

## 6. The Graph Data Model and Dependency Logic

The logical core of Pearls is a Directed Acyclic Graph (DAG) where nodes are Issues and edges are Dependencies. This model is superior to flat lists because it encodes the sequence of work, vital for agents to plan effectively.

### 6.1 Dependency Types

Pearls supports standard Beads dependency types with specific semantic weights in the FSM:

| Type | Semantic Rule | Graph Behavior |
|------|---------------|-----------------|
| Blocks (A blocks B) | B cannot transition to in_progress or closed until A is closed. | Strict constraint. B is hidden from `prl ready`. |
| Parent/Child | Hierarchy. Parent progress is a function of Children. | Parent is effectively a container. |
| Related | Informational link. No FSM constraints. | Edge exists for traversal but ignores blocking logic. |
| Discovered-From | Provenance. B was created while working on A. | Helps trace the explosion of scope. |

### 6.2 Cycle Detection

A critical safety feature is cycle detection. If Task A blocks Task B and Task B blocks Task A, an agent could enter an infinite loop. Pearls utilizes the petgraph Rust crate to perform cycle detection upon every link creation. If a user attempts to create a dependency introducing a cycle, the CLI rejects the operation with a descriptive error.

### 6.3 Topological Sorting and the "Ready" Queue

The `prl ready` command is the primary interface for agents, implementing topological sort to produce a linearized list of tasks with zero open blocking dependencies:

1. Load all issues into the Graph.
2. Filter out issues with status closed or deferred.
3. For every remaining node, check in-degree of edges of type "blocks."
4. Return nodes where in-degree is 0.
5. Sort by Priority (ascending) and Update Time (descending).

This ensures the agent always works on the highest-priority unblocked task.

## 7. Identity and Collision Resistance in Distributed Systems

In distributed environments where multiple agents work on different branches simultaneously, sequential integer IDs (1, 2, 3...) lead to inevitable merge conflicts. If Agent A creates "Issue #5" on Branch X and Agent B creates "Issue #5" on Branch Y, merging requires manual renumbering—a task agents struggle with.

### 7.1 Hash-Based Identity

Pearls uses content-addressable hashing for IDs. The ID is generated by taking the SHA-256 hash of the (title, author, timestamp, nonce) tuple and truncating it to readable length (e.g., 6-8 characters). Example: `prl-a1b2c3`. This approach makes collision probability negligible, even in large teams. If a collision occurs, the nonce is incremented and the ID regenerated.

### 7.2 Human-Readable vs. Agent-Readable

While hashes are perfect for machines, they can be hard for humans to type. Pearls supports prefix-matching lookup. A user can type `prl show a1` and the system resolves it to `prl-a1b2c3` if unique. For agents, the full ID is always enforced to ensure precision.

## 8. Finite State Machine (FSM) Enforcement

The FSM in Pearls is strict to prevent invalid states that confuse agents.

### 8.1 State Transitions

Valid transitions are enforced by the Rust type system:

- **Open → In Progress**: Valid only if the issue is not blocked.
- **In Progress → Closed**: Valid only if acceptance criteria are met.
- **In Progress → Open**: If work is stopped.
- **Any → Deferred**: Moves issue out of the active graph.
- **Closed → Open**: Re-opening a regression.

### 8.2 The "Blocked" Meta-State

In Pearls, blocked is not a static field but a derived state. An issue is effectively "blocked" if graph traversal determines it has open dependencies. However, for UX purposes, the status field may explicitly reflect blocked to allow quick filtering without traversing the entire graph. Pearls auto-updates this status: when a blocking dependency is closed, the dependent issue automatically reverts to open (or ready).

## 9. Rust Implementation Strategy: The Core

The choice of Rust is strategic. It offers the memory safety required for processing untrusted input (agent-generated JSON) and the performance required to treat a file system as a database.

### 9.1 Crate Ecosystem Selection

Pearls is built upon a curated stack of idiomatic Rust crates:

| Category | Crate | Rationale |
|----------|-------|-----------|
| CLI Framework | clap (derive feature) | Type-safe argument parsing, auto-generated help. |
| Serialization | serde, serde_json | Industry standard, zero-copy deserialization support. |
| Git Integration | git2 (libgit2 bindings) | High-performance, thread-safe Git operations without shelling out. |
| Graph Logic | petgraph | Optimized graph algorithms (cycles, toposort). |
| Output | ratatui or tabled | Rich terminal UI for human users. |
| Concurrency | rayon | Parallel iterator support for processing JSONL lines. |
| Error Handling | thiserror (lib) / anyhow (app) | Idiomatic error propagation. |

### 9.2 Error Handling and Recovery

Agentic tools must provide extremely precise error messages. If an agent tries to close a blocked task, the tool should return a structured error:

> Error: Cannot close issue 'prl-123' because it is blocked by 'prl-456' (Status: Open). Resolve dependencies first.

This textual feedback allows the agent to self-correct (agentic reflection).

## 10. Deep Git Integration: Hooks and Merge Drivers

Tight and seamless Git integration is vital. Pearls interacts with Git not just as a storage medium, but as an extension of the version control workflow.

### 10.1 The Custom Merge Driver

The biggest risk of storing data in Git is merge conflicts. If two agents edit `issues.jsonl` simultaneously, Git's default textual merge may corrupt JSON syntax. Pearls includes a binary merge driver `pearls-merge`:

**Mechanism:**
- **Installation**: `prl init` modifies `.git/config` and `.gitattributes`.
- **Trigger**: When Git detects a conflict in `issues.jsonl`, it invokes `pearls-merge %O %A %B` (Ancestor, Ours, Theirs).
- **Logic**: The driver parses all three files into HashMaps, identifies issues modified in both branches, applies field-level merge (e.g., Branch A changed Title, Branch B changed Status → Result has new Title and new Status), handles list merging, and re-serializes to valid JSONL.

This ensures `issues.jsonl` remains syntactically valid even after complex merges.

### 10.2 Git Hooks

Pearls installs lightweight hooks to maintain context:

- **pre-commit**: Scans `issues.jsonl` for formatting errors. Checks if the commit message references a Pearls ID (e.g., `Fixes (prl-123)`) and auto-closes the issue if configured.
- **post-merge**: After a `git pull`, this hook runs a quick `prl doctor` check to ensure graph integrity is maintained (e.g., no orphaned dependencies).

## 11. Performance Engineering: No-Daemon Optimization

Without a daemon, Pearls must start, run, and exit in milliseconds.

### 11.1 Streaming Deserialization

Loading the entire file into memory is fast for small files, but for scalability, Pearls uses `serde_json::Deserializer::from_reader(file).into_iter::<Pearl>()`. This allows streaming processing. When running `prl show <id>`, the parser stops as soon as the ID is found, rather than parsing the entire file.

### 11.2 The Index File (Optional)

For very large repositories (50k+ issues), Pearls can optionally generate a local, non-versioned index file (`.pearls/index.bin`). This maps HashID → FileOffset.

- **Write Path**: When `prl create` appends to the JSONL, it appends the new offset to the index.
- **Read Path**: `prl show` does a binary search on the index, seeks to the offset, and reads one line. This keeps lookup time O(log n) without a database, maintaining the "simplicity" requirement.

## 12. Agentic Workflow Integration

Pearls is designed to be driven by LLMs.

### 12.1 The "Land the Plane" Protocol

Pearls enforces the "Land the Plane" workflow described in Beads documentation. Before an agent signs off, it runs `prl status`. The output provides a checklist:

```
[ ] Clean working directory
[ ] No open P0 issues
[x] Tests passed
[ ] Sync with remote
```

The agent must satisfy these conditions. Pearls facilitates this with the `prl sync` command, which wraps `git pull --rebase && prl merge && git push` into a single atomic operation.

### 12.2 Context Window Optimization

A key advantage of Pearls over markdown is "Compaction." When an agent runs `prl compact`, the tool looks for closed issues older than a threshold (e.g., 7 days). It can:

- Move them to `archive.jsonl`.
- (Advanced) Call an LLM API to summarize closed tasks into a single "Changelog" entry and store that in the parent Epic, then delete individual tasks.

This keeps the active `issues.jsonl` small and the agent's context window focused on current work.

## 13. Migration and Interoperability

### 13.1 Migrating from Beads

Since Pearls uses the same JSONL structure (or a subset thereof), migration is trivial. A `prl import beads` command simply copies `.beads/issues.jsonl` to `.pearls/issues.jsonl`, validating the schema along the way.

### 13.2 Co-existence

Pearls can coexist with other tools. Since it relies on standard Git, a team could theoretically use Pearls for agents and a web-based viewer (like a simple React app rendering the JSONL) for humans, provided the web app pushes commits to the repo.

## 14. Comparative Analysis

| Feature | Pearls (Rust) | Beads (Go) | Markdown (TODO.md) | Jira/GitHub |
|---------|---------------|-----------|-------------------|------------|
| Architecture | CLI + JSONL | Daemon + SQLite + JSONL | Text File | Cloud Database |
| Setup | Single Binary | Binary + Init + Daemon | None | Account + Auth |
| Performance | Millisecond (Native) | Microsecond (Cached) | Instant | Network Latency |
| Agent Friendly | High (Typed, Graph) | High (Typed, Graph) | Low (Unstructured) | Low (API Overhead) |
| Merge Conflicts | Semantic Driver | Semantic Driver | Manual Text Merge | None (Last write wins) |
| Complexity | Low | High | Very Low | High |

## 15. Conclusion

Pearls represents the logical evolution of the "Git-as-Database" paradigm for the era of autonomous software engineering. By stripping away operational complexity—specifically the background daemon and SQLite synchronization—Pearls delivers a tool adhering to the strictest minimalism and portability requirements. Rust ensures this simplification does not compromise performance, while strict JSONL adherence and Git integration guarantee that agent memory is as durable and versioned as the code it writes.

For users seeking a "lightweight version of Beads," Pearls offers the precise feature set required—dependencies, labels, strict FSM—wrapped in idiomatic Rust code respecting the "simplicity is key" mandate. It transforms the ephemeral "vibe coding" of markdown plans into rigorous "mechanical coding" of a directed graph, providing necessary infrastructure for agents to transition from experimental curiosities to reliable contributors.
