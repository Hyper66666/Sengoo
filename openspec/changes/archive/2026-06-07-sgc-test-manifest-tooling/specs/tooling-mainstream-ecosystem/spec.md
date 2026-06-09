## ADDED Requirements

### Requirement: Project tooling SHALL stabilize the existing manifest and lockfile baseline

Sengoo tooling SHALL treat `Sengoo.toml` and `Sengoo.lock` as the existing
project model and stabilize their schema, compatibility diagnostics, and
lockfile freshness behavior.

#### Scenario: A lockfile is stale

- **WHEN** a project command runs in locked mode and `Sengoo.lock` does not match the current dependency graph
- **THEN** the command fails before compiling or running package code
- **AND** the diagnostic identifies the manifest, stale dependency edge, and remediation command

#### Scenario: A project has no manifest

- **WHEN** `sgc check`, `sgc build`, `sgc run`, or `sgc test` is invoked on a standalone source path without `Sengoo.toml`
- **THEN** the command continues to support standalone mode
- **AND** package-only features report that no manifest was selected when needed

### Requirement: sgpm registry and cache behavior SHALL be protocol-stable and diagnosable

The existing sgpm registry and cache surfaces SHALL have stable metadata,
source-id, lockfile, and cache-layout rules for local and remote registries.

#### Scenario: A registry package is resolved

- **WHEN** sgpm resolves a dependency from a local or remote registry
- **THEN** the lockfile records a stable source id, package name, version, and registry identity
- **AND** the cache path is deterministic and safe to inspect

#### Scenario: A registry cache is corrupt

- **WHEN** a cached registry package is missing required metadata, has a checksum mismatch, or cannot be unpacked
- **THEN** sgpm returns a stable diagnostic
- **AND** `sgpm update --refresh` or the documented cache command can rebuild the cache entry

### Requirement: sgc test SHALL provide direct test discovery and reporting

`sgc test` SHALL discover Sengoo test files, filter tests, run them shell-free,
capture output, report pass/fail status, and optionally emit JSON output.

#### Scenario: A directory contains tests

- **WHEN** a user runs `sgc test` for a directory or manifest with `tests/**/*.sg`
- **THEN** each discovered test source is run through the same native execution policy as `sgc run`
- **AND** text output reports pass/fail counts and failing test details

#### Scenario: A user requests JSON output

- **WHEN** a user runs `sgc test --format json`
- **THEN** output contains a schema-tested JSON object with command status, test list, captured output policy, failures, and exit status

#### Scenario: No tests are present

- **WHEN** no tests match discovery or filters
- **THEN** `sgc test` reports zero tests deterministically
- **AND** the exit status follows the documented no-test policy

### Requirement: sgpm test SHALL align with sgc test

The existing `sgpm test` command SHALL preserve its package/workspace behavior
while delegating to, or producing behavior equivalent to, `sgc test`.

#### Scenario: sgpm runs package tests

- **WHEN** a package contains library mappings and `tests/**/*.sg`
- **THEN** `sgpm test` exposes package modules to tests as before
- **AND** the per-test execution and reporting semantics match `sgc test`

#### Scenario: Release mode is selected

- **WHEN** a user runs `sgpm test --release`
- **THEN** the selected optimization/profile is passed to the underlying `sgc test` or equivalent runner

### Requirement: Existing formatter, docs, LSP, bench, and templates SHALL be CI-stable

Existing `sgfmt`, `sgc doc`, `sglsp`, `sgc bench`, and project-template surfaces SHALL produce deterministic, documented behavior suitable for CI and editor workflows.

#### Scenario: Formatting is checked in CI

- **WHEN** a user runs `sgfmt --check` or the package-manager equivalent
- **THEN** unchanged formatted files pass without rewriting
- **AND** unformatted files produce deterministic diagnostics and nonzero exit status

#### Scenario: API docs are generated

- **WHEN** a user runs `sgc doc` or `sgpm doc`
- **THEN** public modules, functions, structs, enums, impls, and examples are rendered into deterministic output paths
- **AND** missing or malformed source docs do not crash the generator

#### Scenario: LSP features are exercised on project examples

- **WHEN** `sglsp` analyzes source examples and package fixtures
- **THEN** completion, hover, definition, diagnostics, code actions, formatting, and workspace symbols are covered by tests

#### Scenario: Bench output is consumed by automation

- **WHEN** a user runs `sgc bench` with machine-readable output
- **THEN** the output schema is stable enough for CI comparisons
- **AND** profile/RSS metrics are either present or explicitly unsupported on the host
