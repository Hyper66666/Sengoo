## ADDED Requirements

### Requirement: Official CLI commands SHALL support bounded hierarchical paths

The official CLI package SHALL add registered subcommands up to 16 segments and
1024 ASCII bytes, preserving single-level APIs. Selected commands SHALL return
their full space-separated path. Ancestor options SHALL apply to descendants;
inherited name conflicts SHALL be rejected while siblings MAY reuse names.
Parsing SHALL remain separate from output/exit and preserve `--` literal mode.

#### Scenario: Three-level command with inherited options
- **WHEN** `config profile show` receives a global flag, an ancestor value and a leaf integer
- **THEN** its path and all applicable values are parsed without printing
- **AND** a sibling may use the same leaf option name with a different type

#### Scenario: Hierarchy errors and literals
- **WHEN** an unknown child or an option for an unselected leaf is supplied
- **THEN** parsing reports status 2 with a diagnostic
- **AND** tokens after `--` remain positionals even if they match a child name

#### Scenario: Deterministic help and ownership
- **WHEN** nested command help is requested repeatedly
- **THEN** registration order and full paths remain stable
- **AND** owning parser/match/String resources return to their initial counts
