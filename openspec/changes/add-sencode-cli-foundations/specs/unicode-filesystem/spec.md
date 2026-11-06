## ADDED Requirements

### Requirement: UTF-8 synchronous file paths on Windows
The synchronous file and directory APIs used by native CLI tools SHALL convert
UTF-8 paths to OS-wide paths and directory entry names back to strict UTF-8.
Existing API signatures, overwrite behavior and status representation SHALL
remain compatible. No operation may fall back to lossy ANSI path conversion.

#### Scenario: Read and atomically replace a Unicode project file
- **WHEN** a caller creates a directory and file containing Chinese and emoji,
  reads UTF-8 content, and replaces it through an adjacent temporary file
- **THEN** file length, read, write, move, kind and timestamp operations succeed
- **AND** managed reads record initialized Buffer lengths for String construction

#### Scenario: Search, copy, list and remove Unicode paths
- **WHEN** a caller copies, appends, lists and walks Unicode names
- **THEN** returned names preserve Unicode and deterministic UTF-8 sorting
- **AND** existing overwrite and same-file rejection behavior remains enforced
- **AND** files and empty directories can be removed through their original paths

#### Scenario: Malformed UTF-8 path
- **WHEN** a raw caller supplies a malformed UTF-8 path on Windows
- **THEN** result-valued synchronous operations return existing status 20
- **AND** boolean existence/removal APIs reject it without touching a lossy alias
