## MODIFIED Requirements

### Requirement: The integrated baseline SHALL be reproducibly green

The release integration baseline SHALL start from the latest configured remote
mainline and SHALL integrate release-relevant work only through reviewed,
owner-attributed commits. The same integrated revision SHALL pass formatting,
lint, tests, realworld package loops, distribution smoke, compatibility, and
strict OpenSpec validation. Required evidence SHALL NOT exist only in an
untracked file, dirty worktree, obsolete branch, or local-only commit.

#### Scenario: A release baseline is selected

- **WHEN** a candidate integration SHA is proposed
- **THEN** it descends from the latest reviewed remote mainline baseline
- **AND** every release-relevant branch is recorded as merged, superseded,
  deferred, or still blocking
- **AND** all required verification jobs and retained evidence reference that
  same full commit SHA
- **AND** the SHA is visible on the configured remote

#### Scenario: A dirty worktree contains unique release work

- **WHEN** inventory finds release-relevant source, tests, fixtures, or evidence
  that are not present in a remote commit
- **THEN** that state is checkpointed and reviewed before integration
- **AND** destructive reset or worktree cleanup is blocked until the unique
  work is preserved
