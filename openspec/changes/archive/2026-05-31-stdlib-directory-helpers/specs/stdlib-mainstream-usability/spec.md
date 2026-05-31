## ADDED Requirements

### Requirement: Directory utilities SHALL support safe portable setup operations
The standard library SHALL provide `std::dir` helpers for directory existence,
single-directory creation, recursive directory creation, and empty-directory
removal.

#### Scenario: A program prepares an output directory
- **WHEN** a Sengoo program imports `std::dir`
- **THEN** it can call `dir_exists(path)` to test for a directory
- **AND** it can call `dir_create(path)` to create one directory
- **AND** it can call `dir_create_all(path)` to create missing parent directories
- **AND** successful fallible helpers return `Result<bool, i64>` with `true`

#### Scenario: Directory creation is idempotent
- **WHEN** a program calls `dir_create(path)` or `dir_create_all(path)` for a
  directory that already exists
- **THEN** the helper returns a successful `Result<bool, i64>`

#### Scenario: Empty directory removal is bounded
- **WHEN** a program calls `dir_remove(path)` for an empty directory
- **THEN** the helper removes that directory and returns a successful
  `Result<bool, i64>`
- **AND** the helper does not recursively delete populated directory trees

#### Scenario: Directory helpers are wired through the stdlib toolchain
- **WHEN** a Sengoo program imports `std::dir`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its
  declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures
- **AND** `examples/stdlib` contains a runnable directory example
