## Why

The latest `main` contains the archived language-maturity and production
hardening waves, but valuable editor work still exists outside `main`, four
older active changes remain open, and several documentation references describe
superseded owner states. Starting v0.2 implementation before reconciling those
facts would recreate duplicate owners and unverifiable completion claims.

## What Changes

- Inventory every worktree/branch with unmerged or untracked value.
- Preserve valuable work in reviewable commits or patches before integration.
- Reconcile active OpenSpec changes, archives, canonical specs, README, language
  reference, and support matrix against executable evidence.
- Assign one active owner per canonical requirement and record dependencies.
- Establish one remote commit SHA that passes the full v0.2 starting gate.

## Capabilities

### Modified Capabilities

- `integration-baseline`: add a v0.2 baseline reconciliation and single-owner
  gate before downstream milestone implementation.

## Impact

- Documentation/OpenSpec only until the integration inventory is approved.
- Later integration may touch all crates, but each merge remains owned by its
  original capability and focused tests.
- No destructive reset, unknown-file deletion, or history rewriting is allowed.

## Non-Goals

- Implementing M1-M4 behavior.
- Refactoring unrelated code during conflict resolution.
- Treating stale task checkboxes as implementation evidence.
