## ADDED Requirements

### Requirement: The v0.2 baseline SHALL preserve and classify all valuable work

Before v0.2 implementation begins, the project SHALL inventory dirty worktrees,
local and remote branches, untracked source/evidence, stashes, and patches, and
SHALL preserve every unique source, test, specification, and documentation
change in a recoverable checkpoint.

#### Scenario: A dirty worktree contains unique implementation

- **WHEN** M0 discovers changes not reachable from the integrated baseline
- **THEN** those changes are checkpointed in a named commit, branch, patch, or
  bundle before reconciliation
- **AND** the inventory records how to recover them
- **AND** no destructive reset or cleanup removes them

### Requirement: The v0.2 baseline SHALL assign one owner per canonical requirement

Every active canonical requirement SHALL have one implementation-owning change;
umbrellas and dependent milestones SHALL reference that owner rather than
redefining the same public behavior.

#### Scenario: A new milestone overlaps an active child

- **WHEN** a v0.2 milestone needs behavior owned by an existing active child
- **THEN** the milestone records an archive dependency on that child
- **AND** does not add a duplicate requirement or conflicting task

### Requirement: The v0.2 starting point SHALL be reproducibly green

The baseline SHALL identify one remote commit SHA that passes formatting, lint,
native tests, installed realworld loops, safety, compatibility, performance, and
strict OpenSpec validation.

#### Scenario: Evidence comes from different revisions

- **WHEN** required baseline jobs reference different commit SHAs
- **THEN** M0 remains open
- **AND** the complete gate is rerun on one integrated revision
