## ADDED Requirements

### Requirement: Assertion helpers SHALL migrate to std::assert without breaking std::error

The standard library SHALL expose `std::assert` as the primary assertion-helper
module while preserving existing `std::error` assertion helper behavior during a
compatibility period.

#### Scenario: A new example imports assertions

- **WHEN** a new stdlib or tutorial example needs assertion helpers
- **THEN** it imports `std::assert`
- **AND** `std::error` examples that already use assertion helpers continue to compile

#### Scenario: Runtime status helpers are needed

- **WHEN** a program needs stable runtime status names or messages
- **THEN** it imports `std::status`
- **AND** `std::error` is not extended with runtime status-classification responsibilities

### Requirement: String and formatting utilities SHALL support mainstream text workflows

The standard library SHALL provide bounded `std::string` and `std::fmt` helpers
for construction, split, join, trim, replace, primitive formatting, and explicit
byte/Unicode boundary behavior.

#### Scenario: A program formats primitive values

- **WHEN** a program formats integers, booleans, status names, or byte-stable text
- **THEN** it can produce output through an owned `String` when available or a managed `Buffer` otherwise
- **AND** formatting failure returns a status-category error rather than panicking

#### Scenario: Unicode-sensitive behavior is requested

- **WHEN** a future implementation needs grapheme clusters, normalization, locale-aware formatting, or collation
- **THEN** it first updates OpenSpec with API shape, portability constraints, and tests

### Requirement: Regex utilities SHALL provide bounded matching and captures

The standard library SHALL provide regex compile, match, capture extraction, and
replace helpers with documented pattern/input limits and deterministic error
categories.

#### Scenario: A program extracts captures

- **WHEN** a program compiles a regex and matches text with capture groups
- **THEN** it can copy the full match and indexed or named captures into accepted text outputs
- **AND** missing captures return `STATUS_NOT_FOUND`

#### Scenario: A regex exceeds limits

- **WHEN** a pattern, input, capture count, or replacement output exceeds documented limits
- **THEN** the helper returns a stable resource or unsupported status
- **AND** it does not enter unbounded catastrophic backtracking

### Requirement: Logging and time utilities SHALL support common CLI and service output

The standard library SHALL provide `std::log` and `std::time` helpers for
level-based logging, deterministic testable sinks, monotonic durations, and
date/time formatting and parsing with explicit timezone rules.

#### Scenario: A program logs at a configured level

- **WHEN** a program emits log records below and above the configured level
- **THEN** records below the level are skipped
- **AND** records at or above the level are written to the configured supported sink

#### Scenario: A program parses a date/time string

- **WHEN** a program parses a date/time string through `std::time`
- **THEN** accepted formats, timezone assumptions, and invalid-input behavior are documented
- **AND** parse failure returns `STATUS_PARSE`

### Requirement: Filesystem, config, hash, encoding, compression, and HTTP helpers SHALL be practical and bounded

The standard library SHALL provide bounded helpers for glob, file watch support
detection, recursive copy/delete policies, TOML/INI config data, SHA-style
hashing, base64/hex encoding, gzip/zlib-class compression, and HTTP workflows
with explicit support limits.

#### Scenario: A program uses config and encoding helpers

- **WHEN** a program parses TOML or INI, hashes bytes, encodes base64 or hex, or compresses/decompresses data
- **THEN** each helper documents input/output limits and copies diagnostics where applicable
- **AND** invalid input returns `STATUS_PARSE`, `STATUS_INVALID_ARGUMENT`, or another stable category

#### Scenario: A program uses filesystem policy helpers

- **WHEN** a program glob-lists paths, recursively copies, recursively deletes, or requests file-watch behavior
- **THEN** ordering, symlink policy, overwrite/delete flags, and unsupported-platform behavior are explicit and tested

### Requirement: Network helpers SHALL stabilize the existing std::net baseline before expansion

The standard library SHALL inventory and stabilize the existing `std::net` and
HTTP runtime baseline before adding broader client or server APIs.

#### Scenario: Existing net helpers are classified

- **WHEN** implementation begins this lane
- **THEN** each existing `std::net` or HTTP helper is classified as stable public API, compatibility-only API, or internal bridge
- **AND** docs and examples use only stable public API names

#### Scenario: A network feature is unsupported

- **WHEN** TLS, DNS, bind, listen, connect, or socket behavior is unsupported on the host
- **THEN** the helper returns `STATUS_UNSUPPORTED` or a more specific stable status
- **AND** native linking does not fail because of unresolved optional network symbols

## MODIFIED Requirements

### Requirement: Standard library modules SHALL be wired through compiler, CLI, LSP, docs, and examples

Every new or stabilized source-level standard-library module SHALL be available
through `sgc` stdlib import expansion, `sglsp` stdlib symbol/signature
discovery, stdlib docs, and a runnable example. Stabilizing an existing partial
module such as `std::net` SHALL include compatibility notes for old names that
remain supported but are not the preferred public surface.

#### Scenario: A new stdlib module is imported by a program

- **WHEN** a Sengoo program imports `std::<module>`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures when the import is present
- **AND** `examples/stdlib` contains a runnable example for the module

#### Scenario: An existing partial module is stabilized

- **WHEN** this change stabilizes an existing partial module such as `std::net`
- **THEN** docs identify stable public names and compatibility-only names
- **AND** examples use stable public names

### Requirement: Later process and data-format extensions SHALL remain gated by explicit follow-up design

This requirement SHALL keep later process and data-format extensions gated by
explicit follow-up design. The `stdlib-next-usability-wave` change satisfies the
follow-up design gate for handle-based `std::json` and shell-free process
command/output helpers. This
`stdlib-breadth-mainstream` change separately satisfies the gate for bounded
TOML/INI config helpers, regex helpers, encoding/compression helpers, and the
stabilized HTTP/network APIs described here. Later expansions beyond these
accepted APIs SHALL NOT be added opportunistically.

#### Scenario: This breadth wave proposes additional data helpers

- **WHEN** implementation agents add TOML, INI, regex, encoding, compression, or stabilized HTTP/network helpers
- **THEN** they follow this change's API shape, portability constraints, resource constraints, lifecycle semantics, and tests
- **AND** they do not add streaming parsers, schema validation, shell execution, background tasks, async network execution, or implicit TLS guarantees without another OpenSpec update

#### Scenario: A future phase proposes additional process or data-format features

- **WHEN** a future implementation needs streaming JSON, JSON5, schema validation, dynamic Sengoo object mapping, implicit shell commands, pipes, background tasks, signals, cancellation, async process execution, async network execution, or unbounded watchers
- **THEN** it first updates OpenSpec with API shape, portability constraints, security constraints, lifecycle semantics, and tests
