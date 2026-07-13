# integration-baseline Specification

## Purpose
TBD - created by archiving change mainline-release-baseline. Update Purpose after archive.
## Requirements
### Requirement: Development progress SHALL be recoverable before integration

The project SHALL preserve all source, test, documentation, example, and
OpenSpec changes in a verified checkpoint before reconciling divergent history.

#### Scenario: Integration must be restarted

- **WHEN** a merge or conflict resolution attempt fails
- **THEN** the pre-integration state can be restored from the checkpoint
- **AND** no unexplained user change depends only on the failed worktree

### Requirement: Mainline integration SHALL preserve capability ownership

Changes SHALL be integrated in reviewable slices associated with their owning
OpenSpec capability, and destructive reset SHALL NOT be used to erase unknown
differences.

#### Scenario: A conflict spans two capability lanes

- **WHEN** integration finds conflicting edits in a shared file
- **THEN** both owners' intended behavior and tests are inspected
- **AND** the resolution is verified by the affected focused tests
- **AND** the commit records the relevant constraint or rejected alternative

### Requirement: Repository truth sources SHALL match implementation evidence

Active tasks, archived changes, README, inventory, and support matrix SHALL be
reconciled against code and executable evidence from the integrated revision.

#### Scenario: Code is ahead of its task list

- **WHEN** implemented behavior and tests exist for a task still described as
  absent
- **THEN** the task records the actual evidence and remaining acceptance gap
- **AND** it is checked complete only if every stated scenario passes

### Requirement: The integrated baseline SHALL be reproducibly green

The same integrated revision SHALL pass formatting, lint, tests, realworld
package loops, distribution smoke, and strict OpenSpec validation.

#### Scenario: Phase 0 is proposed for archive

- **WHEN** `mainline-release-baseline` is proposed for archive
- **THEN** all required verification jobs reference the same commit SHA
- **AND** the SHA is visible on the configured remote
- **AND** no required capability evidence exists only in an untracked file or
  obsolete branch
