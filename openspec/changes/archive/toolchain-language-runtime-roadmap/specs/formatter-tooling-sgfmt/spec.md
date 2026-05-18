## ADDED Requirements

### Requirement: sgfmt SHALL produce deterministic formatting
`sgfmt` SHALL transform valid Sengoo source into a deterministic canonical format.

#### Scenario: Reformatting is idempotent
- **WHEN** a file is formatted by `sgfmt` and immediately formatted again
- **THEN** the second run produces no textual changes

### Requirement: sgfmt SHALL support configurable formatting style
`sgfmt` SHALL read formatter settings from a project configuration file with rustfmt-like semantics.

#### Scenario: Project style configuration overrides defaults
- **WHEN** a project provides formatter configuration values
- **THEN** `sgfmt` applies configured style rules instead of built-in defaults

### Requirement: Formatting SHALL be invokable through sengoo/sgc command surface
The toolchain SHALL expose formatting through `sengoo fmt` (or `sgc fmt`) as a first-class workflow.

#### Scenario: Unified CLI format command runs formatter
- **WHEN** a developer runs the integrated format command in a Sengoo project
- **THEN** the command executes `sgfmt` behavior and returns formatter success or failure status
