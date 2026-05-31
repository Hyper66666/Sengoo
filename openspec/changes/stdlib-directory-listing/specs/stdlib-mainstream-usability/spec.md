## ADDED Requirements

### Requirement: Directory utilities SHALL support deterministic non-recursive listing
The standard library SHALL provide `std::dir` helpers for counting immediate
directory entries and copying one entry name into a managed Buffer.

#### Scenario: A program counts immediate directory entries
- **WHEN** a Sengoo program imports `std::dir`
- **AND** calls `dir_entry_count(path)` on a readable directory
- **THEN** the helper returns an ok-shaped `Result<i64, i64>` containing the
  number of immediate child entries
- **AND** the count excludes `.` and `..`

#### Scenario: A program copies a deterministic entry name
- **WHEN** a directory contains entries named `b.txt` and `a.txt`
- **AND** a program calls `dir_entry_name(path, 0, buffer)`
- **THEN** the helper copies `a.txt` into the managed Buffer
- **AND** returns an ok-shaped `Result<i64, i64>` with the number of bytes
  copied
- **AND** it does not append a NUL terminator

#### Scenario: Listing order is stable across host iteration order
- **WHEN** a directory contains multiple entries
- **THEN** `dir_entry_name` indexes entries after sorting names by unsigned
  byte order

#### Scenario: Invalid listing requests are rejected
- **WHEN** a program lists a non-directory path, uses a negative or out-of-range
  index, or provides an output Buffer that is too small
- **THEN** the helper returns an error-shaped `Result<i64, i64>`

#### Scenario: Advanced directory traversal features remain explicitly deferred
- **WHEN** a future implementation needs recursive traversal, recursive
  deletion, glob matching, metadata structs, owned-string entry returns, or a
  persistent iterator/list API
- **THEN** it first updates OpenSpec with API shape, ownership constraints,
  portability constraints, safety constraints, and tests
