## ADDED Requirements

### Requirement: Bounded SG tar scan

The implementation SHALL parse tar framing and metadata in SG, with caller-owned
disjoint byte extents, explicit effective-content and metadata budgets, located
entry records and stable negative failure codes. C SHALL contain no tar parser,
checksum, numeric-field parser or path policy.

#### Scenario: Real GNU package structure
- **WHEN** a valid archive from the repository's GNU builder contains ordinary files and long names
- **THEN** the SG scanner returns their exact effective paths, content ranges and types
- **AND** its output is compared to independently executed pinned Rust tar and Python tarfile

#### Scenario: Bounds and failed publication
- **WHEN** input is truncated, a number overflows, an arena is short or a resource budget is exceeded
- **THEN** the scanner returns the documented negative error without unauthorized memory access
- **AND** no success header is published and the caller discards any partial output

### Requirement: Safe package path identity

The scanner SHALL reject traversal, absolute/drive/UNC paths, invalid UTF-8,
Windows aliases and reserved package marker paths. Duplicate and file-ancestor
conflicts SHALL be detected after normalization, independent of record order.

#### Scenario: Alias and prefix collisions
- **WHEN** equivalent case/slash/dot paths or a file and its descendant appear in either order
- **THEN** the scan fails with the stable identity-conflict code

#### Scenario: Link entry
- **WHEN** a symbolic link, hard link, sparse or other unsupported special entry occurs
- **THEN** the scan fails before filesystem mutation

### Requirement: Honest package budget integration

The change SHALL distinguish scanning a bounded tar buffer from complete package
consumption. Completing package integration SHALL require 64 MiB compressed,
256 MiB cumulative file content and 10,000-entry budgets, complete gzip CRC
validation, manifest identity checks and transactional staging/publication.

#### Scenario: Foundation has only a 64 MiB gzip output decoder
- **WHEN** the source-only tar scanner passes while the existing gzip output bound remains 64 MiB
- **THEN** the package integration tasks remain incomplete
- **AND** the implementation does not claim replacement of the current sgpm resolver
