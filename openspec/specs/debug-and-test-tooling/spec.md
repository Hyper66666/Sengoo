# debug-and-test-tooling Specification

## Purpose
TBD - created by archiving change debugger-and-test-framework. Update Purpose after archive.
## Requirements
### Requirement: Source-level debugging SHALL be validated for the core language

A debugger SHALL be able to set breakpoints, single-step, show a correct
backtrace, and inspect locals and parameters with correct types and values for
core-language programs.

#### Scenario: Breakpoint, step, and inspect

- **WHEN** a debugger sets a breakpoint in a Sengoo function and steps over a few
  statements
- **THEN** execution stops at the correct source lines
- **AND** locals and parameters (scalars, structs, enums, strings, `Vec`) display
  with correct names, types, and values
- **AND** an automated test drives lldb or cdb to assert this behavior

### Requirement: Debugging SHALL be available from the editor

The toolchain SHALL provide a DAP bridge or a documented lldb/cdb launch wired
into the VS Code extension.

#### Scenario: Debug from the editor

- **WHEN** a user starts a debug session on a Sengoo file from VS Code
- **THEN** they can set breakpoints, step, and inspect variables from the editor
  UI per the documented setup

### Requirement: The test framework SHALL support discovery, fixtures, and parametrization

`sgc test` SHALL discover tests via a `#[test]` attribute or `test_*` convention
and SHALL support setup/teardown fixtures and parametrized cases.

#### Scenario: Discovery and fixtures

- **WHEN** a package defines `#[test]` functions with fixtures and a parametrized
  case
- **THEN** `sgc test` discovers and runs each case, applying fixtures in the
  documented order
- **AND** a parametrized case runs once per parameter row

### Requirement: Test output SHALL be structured and coverage SHALL be reportable

Assertion failures SHALL include expected/actual and file/line in the existing
JSON envelope, and `sgc test --coverage` SHALL report line coverage.

#### Scenario: Structured failure and coverage

- **WHEN** an assertion fails and the suite is run with `--coverage`
- **THEN** the failure reports expected, actual, file, and line in the JSON
  envelope
- **AND** a line-coverage summary and machine-readable report are produced
- **AND** existing `sgc test` JSON fields remain present

