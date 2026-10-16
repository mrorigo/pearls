# Changelog

## [0.4.0] - Unreleased

### Added

- An embeddable Rust graph service in `pearls-app` with explicit repository roots, consistent active and archived snapshots, typed scheduling sets, and ordered atomic mutation batches.
- Durable transaction publication and recovery, monotonic graph revisions, optimistic concurrency checks, operation receipts with idempotent replay, and stable external task identities.
- Configurable receipt retention. Full receipts have a 30-day replay window by default; on the next supported access after expiry, Pearls replaces the receipt with a permanent key tombstone that cannot be replayed or reused.
- A graph service guide and runnable example.

### Changed

- CLI, MCP, hook, import, doctor, initialization, and compaction task writers now use the shared recoverable transaction service.
- `prl ready` now includes only unblocked tasks with Open status. MCP scheduling views also expose running, blocked, deferred, and closed tasks.
- Graph validation checks relationship endpoints across active and archived tasks and rejects cycles in blocking relationships.
- Post-merge validation now rejects missing dependency endpoints.
- Import validates the complete graph and status transitions for existing tasks. Compaction and doctor preserve transaction receipts and archived relationship targets.
- Transaction metadata uses schema version 2. The application batch request and receipt digest protocol remains version 1.

### Upgrade notes

- Back up the complete `.pearls` directory before upgrading. Do not run older Pearls writers concurrently with v0.4.0; older versions can change task files without updating transaction metadata.
- A repository without transaction metadata is bootstrapped on its first supported v0.4.0 read. This preserves the existing task-file bytes. Opening the repository through the embedded service alone does not create files.
- A repository with schema version 1 transaction metadata requires explicit migration. Run `prl init`, or run `prl doctor --fix` if task records also need repair. Migration preserves retained receipts and gives them a new replay window starting at migration time.
- Expired receipt keys stay reserved permanently. On the next supported access after the configured retention period, Pearls removes the full receipt data and keeps a small tombstone. The default window is 30 days. Set `receipt_retention_days` in `.pearls/config.toml` or `PEARLS_RECEIPT_RETENTION_DAYS` to change the window for new receipts.
- See [Upgrade and rollback](docs/GRAPH-SERVICE.md#upgrade-and-rollback) for backup, recovery, and downgrade requirements.
