# Embedded graph service

The `pearls-app` crate exposes `pearls_app::service::GraphService` as a Rust application programming interface (API).
The application request protocol is version `API_VERSION = 1`.
Command-line interface (CLI) commands, Model Context Protocol (MCP) tools, and hooks share mutation validation and publication.

## Open a repository

Pass the initialized repository root explicitly:

```rust,no_run
use pearls_app::{service::GraphService, Result};

fn open_repository() -> Result<GraphService> {
    GraphService::open("/path/to/repository")
}
```

An empty path or missing `.pearls` directory returns an error.
Opening does not discover another repository, initialize files, install hooks, or change Git settings.
Initialize separately with `prl init`.

Service methods return `Result` values.
They do not print protocol output or terminate the process.
Reads can recover a pending publication and record externally changed task files.
Those actions can modify transaction metadata.

## Read and schedule tasks

`snapshot()` returns all active and archived records with one repository revision.
`snapshot_for(&TaskSelection)` returns the requested tasks and their transitive outgoing relationship targets.
It follows all relationship types, including relationships through archived tasks.
Incoming dependents are included only when another selected relationship reaches them.

`TaskSelection.required_ids` is a required `Vec<String>` of exact full task identifiers (IDs).
IDs are case-sensitive. Partial IDs are unsupported in this API.
Duplicates are evaluated once. An empty list produces empty results.
Unknown IDs and corruption anywhere in the repository return errors.

`schedule(&TaskSelection)` returns these disjoint sets:

| Field | Contents |
| --- | --- |
| `ready` | Open tasks without non-closed blocking dependencies |
| `running` | Tasks with InProgress status |
| `blocked` | Open tasks with blocking dependencies, plus tasks explicitly marked Blocked |
| `deferred` | Tasks with Deferred status |
| `closed` | Tasks with Closed status, including archived tasks |

Running, Deferred, and Closed status take precedence over dependency facts.
Archive membership alone does not imply Closed.
Each blocked entry contains the task, typed reasons, and direct blocking IDs with their statuses.
Blockers can be outside the requested set.
Blocker lists use full-ID order and contain no duplicates.

Each result set uses ascending priority, descending `updated_at`, then ascending full ID.
The revision covers the complete repository, including tasks outside the selection.
An empty ready set does not imply completion. The consumer defines completion.

## Apply an atomic batch

A batch requires these fields:

| Field | Rust type | Requirement |
| --- | --- | --- |
| `operation_key` | `String` | Nonempty, opaque, case-sensitive, repository-local identity |
| `expected_revision` | `GraphRevision` | Revision from a graph read; compare only for equality |
| `mutations` | `Vec<Mutation>` | Nonempty, ordered operations |

Use `TaskRef::Id` for exact full IDs.
Use `TaskRef::Local` for a creation reference declared earlier in the same batch.
Each creation requires a unique, nonempty `local_ref` within that batch.

Supported mutations create tasks, update content, change status, link or unlink tasks,
set metadata, add or delete comments, and import explicit JavaScript Object Notation Lines (JSONL) records.
There are no task delete or restore mutations.
Archived tasks can be relationship targets, but ordinary mutations cannot edit them.

Creation requires `title` and `author`.
Description defaults to empty and has a 65,536-byte limit.
Priority defaults to repository configuration and cannot exceed its configured maximum.
Labels default to an empty list. Metadata defaults to an empty object.
`external_key` defaults to absent.

Updates replace only supplied fields.
An omitted title, description, priority, or labels field retains its current value.
`Unlink` removes every relationship type between its source and target.
Comment deletion requires an exact comment ID.
`Import` preserves incoming task IDs and validates status changes to existing tasks.
Later records with the same ID replace earlier records within one import mutation.
Imports cannot overwrite archived tasks or replace reserved external identity metadata.

Validation runs under the publication lock.
Each mutation observes preceding mutations. Every intermediate graph must be valid.
Only Blocks relationships participate in cycle checks and readiness.
For a Blocks edge, `from` depends on `to`. Closing `to` can unblock `from`.

A successful receipt contains the operation key, canonical request digest,
before/after revisions, and creation-reference-to-ID mapping.
An accepted no-op still advances the repository generation.
A rejected batch publishes no requested task changes or receipt.

See the executable [three-task example](../crates/pearls-app/examples/graph_service.rs):

```sh
cargo run -p pearls-app --example graph_service -- /path/to/initialized/repository
```

## Recover a lost response

Persist the complete `BatchRequest` before calling `apply`.
After a lost response, look up `receipt(operation_key)` or replay that exact request.

Receipt lookup precedes the expected-revision check.
An identical request returns the original receipt without applying mutations again.
A different request using the same retained key returns `IdempotencyConflict`.
An unseen key with a stale revision returns `RevisionConflict`.
Two distinct requests cannot both commit against one before revision.

The digest includes API version, expected revision, and ordered mutations.
JSON object key order is ignored recursively.
Array order, mutation order, and explicit defaults affect request identity.
For example, an absent priority and an explicitly supplied default priority are different definitions.
Changing the operation key does not change the content digest.

An input/output (I/O) error after durable journaling has an unknown outcome until recovery.
Recover or replay the original key before submitting a replacement operation.
Task timestamps do not define revisions.
Revisions contain a monotonic generation and task-file fingerprints; treat their representation as opaque.

## Receipt retention

New full receipts have a 30-day replay window by default.
Configure the window in `.pearls/config.toml`:

```toml
receipt_retention_days = 30
```

`PEARLS_RECEIPT_RETENTION_DAYS` overrides that value.
The value is a positive `u32` number of days. Zero, negative values, and invalid numbers are errors.
One day is 86,400 seconds. Deadlines use the system clock in Unix seconds.
Keep the host clock accurate; a forward clock jump can expire receipts early.

Each receipt keeps the deadline assigned at commit.
Configuration changes apply to new receipts and do not change existing deadlines.
At the deadline, a supported read or mutation replaces the full receipt with a key tombstone.
A tombstone stores only the reserved operation key and survives reopening and task compaction.
Tombstones have no expiry, so their storage grows with the number of expired keys.

Lookup or replay of an expired key returns `OperationExpired`, with the operation key in structured details.
It never returns `None` and never reapplies that operation.
Different request content cannot reuse an expired key.
Reconcile task IDs and external state before creating a new operation after receipt expiry.
An external task key can identify an original task after its operation receipt expires.

Expiration is recoverable metadata maintenance. It does not change the graph revision.
Task compaction does not shorten a receipt's replay window or remove tombstones.

## Use stable external identities

`CreateTask.external_key` assigns an optional opaque external identity.
Pearls reserves these metadata fields:

| Field | Purpose |
| --- | --- |
| `pearls.external_key` | External identity string |
| `pearls.external_definition` | Original creation definition |

Keys are case-sensitive and unique across active and archived tasks.
Pearls does not interpret namespaces or key contents.
Repeated creation with the same key and original definition resolves to the original ID.
It preserves later edits, comments, status changes, and archive membership.
A different creation definition returns `ExternalKeyConflict`.
Duplicate owners return a conflict, even when their definitions match.

The original definition includes title, author, description, priority choice, labels, and initial metadata.
It excludes generated IDs, timestamps, runtime status, dependencies, and comments.
Define relationships and status changes with separate ordered mutations.
Do not modify the reserved metadata fields directly.
JSONL imports with explicit IDs preserve those IDs; use `Create` for external-key-based ID resolution.

## Attach references and close a task

Put `SetMetadata` and `SetStatus { status: Status::Closed }` in one batch.
A denied transition rolls back the reference and the close together.
Transition errors contain the task ID, original/target statuses, denial reason, and blockers.

Metadata values are opaque JSON.
Pearls does not interpret reports, content digests, execution results, or verification claims.
Consumers own those meanings.

## Handle typed errors

Use `AppError::code()` and `AppError::details()` rather than parsing display strings.
`ErrorEnvelope::from_error` exposes the same code and details for adapters.

| Code | Structured details or action |
| --- | --- |
| `not_found` | Exact task/comment ID for service mutations |
| `invalid_graph` | Duplicate ID, missing endpoint, or blocking-cycle path |
| `invalid_transition` | Task ID, statuses, reason, and blocking IDs/statuses |
| `revision_conflict` | Expected and actual revisions; read and reconcile |
| `idempotency_conflict` | Previously used operation key; recover the original request |
| `operation_expired` | Reserved expired operation key; reconcile without blindly repeating the operation |
| `external_key_conflict` | External key and existing task IDs |
| `lock_timeout` | Repository lock could not be acquired within 30 seconds |
| `corrupt_storage` | File/schema or external-identity integrity details |
| `io_error` | Operating-system failure; resolve uncertain outcomes using the operation key |

Invalid inputs and task-field validation failures have separate codes.
Repository initialization and unsupported schemas also return typed errors.

## Publication and external changes

The authoritative files are `.pearls/issues.jsonl`, `.pearls/archive.jsonl`,
and `.pearls/transaction-state.json`.
Track transaction state alongside task and archive files in Git.
It contains generations and operation history needed for replay protection.

A durable `.pearls/transaction-journal.json` is the publication decision.
It contains complete replacement images and operation state.
Participating readers recover the journal before returning data.
Recovery verifies destination fingerprints before replacing files.
It refuses to overwrite a conflicting external edit.
The derived `.pearls/index.bin` is invalidated and can be rebuilt.

Unix publication synchronizes files and directories.
Other platforms provide process-crash recovery without a power-loss guarantee.
Abandoned staging files have no authority.

Direct `Storage` writes bypass application validation and the supported concurrent protocol.
Do not run older Pearls writers concurrently with the service.
Git merges and manual task edits are external changes.
The next supported read returns a changed revision or an integrity error.
Concurrent manual writes are unsupported and can prevent recovery.

Restoring an earlier Git state can restore an earlier generation and operation history.
Replay protection does not extend across that rollback.
Reconcile task identities and external state before retrying after a history rollback.

## Upgrade and rollback

The task JSONL format remains compatible with existing records.
Missing optional fields retain their existing defaults.
CLI command names, MCP tool names, and their response field names remain available.
Ready views now contain only unblocked Open tasks.
Post-merge validation rejects missing endpoints instead of only printing warnings.
Imports validate the complete graph and status changes to existing tasks.

Transaction state and journal use schema version 2.
This schema adds receipt deadlines and permanent expired-key tombstones.
The application request protocol remains version 1; receipt fields and canonical digests remain unchanged.
A compatibility fixture protects the version 1 digest encoding.

Repositories without transaction metadata acquire schema v2 metadata on their first supported read.
That read preserves the original task-file bytes.
Opening alone creates no files.
Repositories with schema v1 metadata require an explicit upgrade.
Ordinary reads, mutations, and compaction reject v1 without rewriting its files.
Other unsupported versions remain errors, including during explicit migration.

Before upgrading, stop every Pearls writer and back up the complete `.pearls` directory.
Include any pending transaction journal in that backup.
Do not delete a pending journal to bypass a migration error.

For a valid repository, explicitly call `GraphService::initialize_files()` after opening it.
This method preserves task records, recovers any pending v1 publication, and upgrades transaction metadata.
It does not install hooks or configure Git.
If task records need repair, `prl doctor --fix` also performs the explicit upgrade.
Doctor keeps the newest active duplicate, removes malformed active lines, and removes missing endpoints.
It preserves references to archived tasks. Unrepairable graph or identity conflicts prevent publication.

Legacy v1 receipts have no recorded timestamps.
Migration gives each retained legacy receipt a new replay window starting at migration time.
It preserves their original operation keys, digests, revisions, and created-ID mappings.
New reads can replay those receipts immediately after migration.
If migration is interrupted, repeat the explicit migration with the current binary.

Before downgrading, stop every writer and recover pending publication with the current binary.
Restore a complete, consistent, journal-free pre-upgrade backup before starting the older binary.
A backup containing a pending v1 journal needs recovery with a v1-aware binary before an older writer can use it.
Such a backup is a recovery copy, not an immediately usable downgrade snapshot.
There is no automatic v2-to-v1 downgrade.
Do not combine older transaction metadata with newer task files.
Schema-aware v1 binaries reject v2 metadata.
Released binaries without transaction support can bypass the protocol and must not run concurrently.
Restoring a backup restores its earlier operation history; reconcile externally before retrying operations.

## Acceptance coverage

Run the workspace checks from the repository root:

```sh
cargo fmt --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The workspace currently has five pre-existing `unnecessary_sort_by` Clippy findings in `pearls-app/src/list.rs`.
To check for other lint failures, add `-A clippy::unnecessary_sort_by` after `-D warnings`.

| Scenario | Covered behavior | Test source |
| --- | --- | --- |
| A1 | Three-task import with two blocking links, reopen, and identical replay | [Service tests](../crates/pearls-app/tests/service_tests.rs) |
| A2 | A cyclic batch publishes no tasks or receipt | [Service tests](../crates/pearls-app/tests/service_tests.rs) |
| A3 | Competing expected revisions produce one winner | [Service tests](../crates/pearls-app/tests/service_tests.rs) |
| A4 | Process exit and I/O failure at every durable publication boundary | [Repository tests](../crates/pearls-core/src/repository/tests.rs) |
| A5 | Blocked and deferred tasks remain visible with an empty ready set | [Scheduling tests](../crates/pearls-app/tests/scheduling_tests.rs) |
| A6 | Reference attachment and close replay together | [Service tests](../crates/pearls-app/tests/service_tests.rs) |
| A7 | Real compaction preserves external identity and original receipt replay | [Adapter tests](../crates/pearls-app/tests/adapter_tests.rs) |
| A8 | External edits invalidate a stale expected revision | [Service tests](../crates/pearls-app/tests/service_tests.rs) |

[Retention tests](../crates/pearls-core/src/repository/retention_tests.rs) cover expiry boundaries, migration, and recovery of partial v1 publication.
[Compatibility tests](../crates/pearls-app/tests/compatibility_tests.rs) cover legacy records, expired-key errors, and unchanged hooks/configuration.
[CLI adapter tests](../crates/pearls-cli/tests/service_adapter_tests.rs) and MCP tests cover shared validation and revision behavior.
