## Decisions

### D1: Preserve before reconciling

Every dirty or ahead branch with unique changes gets a named commit, patch,
bundle, or remote branch before any integration attempt. Generated artifacts and
temporary files are classified separately and removed only after their paths are
verified.

### D2: Code and executable tests outrank task prose

Task state, README, and support claims are corrected from implementation and
test evidence. A checked task without evidence is reopened; implemented behavior
with an open task is checked only after all stated scenarios pass.

### D3: One canonical requirement, one active owner

Umbrellas own ordering and integration. Child changes own public semantics. If
an active legacy umbrella duplicates a v0.2 child, it is archived, superseded,
or narrowed before code work starts.

### D4: Baseline evidence shares one SHA

Formatting, lint, tests, realworld loops, and OpenSpec validation must reference
the same remote commit. Evidence from a different branch or untracked artifact
does not close M0.

## Integration order

1. Snapshot branch/worktree state and classify unique changes.
2. Checkpoint the in-progress `enhance-sglsp-smart-completion` lane.
3. Reconcile the four active changes and their umbrella relationships.
4. Correct stale README/reference/support-matrix claims.
5. Run the full baseline gate and publish its SHA.

## Rollback

Every integration slice is a normal commit. A failed slice is reverted with a
new commit after preserving diagnostics; destructive reset is prohibited.
