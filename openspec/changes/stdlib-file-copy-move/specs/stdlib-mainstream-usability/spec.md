## ADDED Requirements

### Requirement: File utilities SHALL support explicit-overwrite copy and move
The standard library SHALL provide `std::file` helpers for copying file bytes
and moving files with an explicit overwrite choice.

#### Scenario: A program copies a file without removing the source
- **WHEN** a Sengoo program imports `std::file`
- **AND** calls `file_copy(source, destination, false)` for a readable source
  and absent destination
- **THEN** the helper writes the same bytes to the destination
- **AND** returns an ok-shaped `Result<i64, i64>` with the number of bytes
  copied
- **AND** the source remains present

#### Scenario: Existing destinations require explicit overwrite
- **WHEN** a destination already exists
- **AND** a program calls `file_copy(source, destination, false)` or
  `file_move(source, destination, false)`
- **THEN** the helper returns an error-shaped result
- **AND** does not replace the destination

#### Scenario: A program explicitly overwrites a copied destination
- **WHEN** a destination already exists
- **AND** a program calls `file_copy(source, destination, true)`
- **THEN** the destination bytes are replaced with the source bytes
- **AND** the helper returns the number of bytes copied

#### Scenario: A program moves a file with host rename semantics
- **WHEN** a program calls `file_move(source, destination, overwrite)`
- **AND** the host rename primitive succeeds
- **THEN** the helper returns an ok-shaped `Result<bool, i64>` containing
  `true`
- **AND** the source path no longer exists
- **AND** the destination path exists

#### Scenario: Advanced file-transfer features remain explicitly deferred
- **WHEN** a future implementation needs recursive directory transfer,
  cross-filesystem move fallback, metadata preservation guarantees, atomic
  copy guarantees, progress callbacks, cancellation, or async I/O
- **THEN** it first updates OpenSpec with API shape, portability constraints,
  safety constraints, and tests
