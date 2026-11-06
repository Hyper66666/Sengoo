## ADDED Requirements

### Requirement: Persisted compiler caches SHALL follow compiler content identity

Build, run, frontend-session and generic-instance caches SHALL include the
running compiler executable's SHA-256 identity. Missing or differing identity
SHALL invalidate reuse before any codegen, link or artifact recovery shortcut.
Package version, Git revision, executable path, file size and timestamp alone
SHALL NOT qualify a cache. Equal executable contents at another path MAY reuse.

#### Scenario: Dirty compiler rebuild keeps its version
- **WHEN** compiler bytes change while source, version and revision stay equal
- **THEN** old persisted caches are ignored and frontend/codegen rebuild
- **AND** the new metadata records the new content identity

#### Scenario: Identical compiler moves
- **WHEN** an identical compiler executable is copied to another path
- **THEN** an otherwise valid cache remains reusable

#### Scenario: Legacy cache has no executable identity
- **WHEN** a cache from an earlier compiler lacks the identity field
- **THEN** it is treated as a cache miss without changing manifest or lockfiles
