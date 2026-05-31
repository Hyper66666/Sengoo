## ADDED Requirements

### Requirement: Standard library modules SHALL be wired through compiler, CLI, LSP, docs, and examples
Every new source-level standard-library module SHALL be available through `sgc` stdlib import expansion, `sglsp` stdlib symbol/signature discovery, stdlib docs, and a runnable example.

#### Scenario: A new stdlib module is imported by a program
- **WHEN** a Sengoo program imports `std::<module>`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures when the import is present
- **AND** `examples/stdlib` contains a runnable example for the module

### Requirement: Path utilities SHALL support common cross-platform path operations
The standard library SHALL provide `std::path` helpers for path separator discovery, absolute-path checks, joining, file-name/stem/extension extraction, parent extraction, and lexical normalization.

#### Scenario: A program manipulates paths without raw pointer choreography
- **WHEN** a Sengoo program imports `std::path`
- **THEN** it can call safe wrappers using `&str` inputs and managed `Buffer` outputs
- **AND** fallible string-producing helpers return `Result<i64, i64>` with the byte count on success

#### Scenario: Absolute paths are recognized conservatively
- **WHEN** a program checks a Unix root path, a Windows drive-root path, or a UNC-like path
- **THEN** `path_is_absolute` returns true
- **AND** relative paths return false

#### Scenario: Path normalization is lexical
- **WHEN** a program normalizes a path containing duplicate separators, `.` segments, or simple `..` segments
- **THEN** the result is normalized lexically into the provided `Buffer`
- **AND** the helper does not resolve symlinks or require the path to exist on disk

#### Scenario: Joining with an absolute right-hand side
- **WHEN** a program joins a base path with a right-hand side that is already absolute
- **THEN** `path_join` writes the absolute right-hand side into the provided `Buffer`
- **AND** it does not prefix the base path

### Requirement: Runtime-produced string outputs SHALL use managed Buffer handles
Until Sengoo has a specified owned-string return ABI, stdlib runtime helpers that produce string-like output SHALL copy into managed `Buffer` handles and report byte counts.

#### Scenario: A helper produces a string-like result
- **WHEN** a stdlib helper such as `path_join`, `path_parent`, or `path_extension` needs to return text
- **THEN** it accepts a managed `Buffer`
- **AND** it returns `Result<i64, i64>` indicating bytes written or an error code

### Requirement: Process and data-format usability SHALL be gated by explicit follow-up design
Process execution and JSON-like data-format helpers SHALL NOT be added opportunistically in the path phase.

#### Scenario: A later phase proposes command execution or JSON helpers
- **WHEN** a future implementation needs process execution or JSON-like parsing/formatting
- **THEN** it first updates OpenSpec with API shape, portability constraints, security constraints, and tests
