# Pearls Workflow Example

This example demonstrates a typical end-to-end workflow using the `prl` CLI.

```bash
# Initialize repository
prl init

# Create Pearls
prl create "Design storage index" --priority 1 --label storage,performance
prl create "Implement index rebuild" --priority 2 --label storage

# Link dependencies
prl link prl-abc123 prl-def456 blocks

# Update status and add metadata
prl update prl-abc123 --status in_progress --add-label urgent
prl meta set prl-abc123 owner \"alice\"

# List and show
prl list --status open --sort updated_at
prl show prl-abc123

# Close and compact
prl close prl-abc123
prl compact --threshold-days 30
```
