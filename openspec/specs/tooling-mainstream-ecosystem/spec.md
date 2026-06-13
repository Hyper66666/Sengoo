# tooling-mainstream-ecosystem Specification

## Purpose
TBD - created by archiving change sgc-test-manifest-tooling. Update Purpose after archive.
## Requirements
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

### Requirement: sgc documents and supports cross-compilation for reference targets

`sgc build` SHALL accept an explicit `--target <triple>` flag for supported host
pairs and SHALL document required SDK/sysroot environment variables.

#### Scenario: Windows host builds Linux gnu triple with documented sysroot

- **WHEN** a developer runs `sgc build main.sg --target x86_64-unknown-linux-gnu`
  with the documented sysroot environment on the reference Windows host
- **THEN** the build produces a runnable Linux artifact or a documented linker error
  with remediation steps
- **AND** `docs/cross-compilation.md` lists supported triples and env vars

#### Scenario: Unsupported triple fails with actionable diagnostic

- **WHEN** a developer passes an unsupported `--target` triple
- **THEN** `sgc` exits non-zero with a diagnostic naming the triple and pointing to
  `docs/cross-compilation.md`

### Requirement: sgc emits machine-readable compile timings

`sgc build` SHALL support `--timings-json <path>` exporting schema-version-1 phase
timings aligned with `frontend-compile-perf` phase names.

#### Scenario: Timings JSON includes frontend sub-phases

- **WHEN** `sgc build --timings-json out.json` completes a native build
- **THEN** `out.json` contains per-phase milliseconds for parse, typeck, hir_lower,
  mir_lower, mir_opt, and codegen
- **AND** the schema version field is integer `1`

### Requirement: sglsp resolves symbols across dependency sources

`sglsp` SHALL provide go-to-definition for symbols defined in direct path or git
dependencies resolved by `sgpm` in the current workspace graph.

#### Scenario: Go-to-definition reaches a path dependency module

- **WHEN** a workspace imports a symbol from a path dependency and the editor requests
  go-to-definition
- **THEN** `sglsp` opens the dependency source location
- **AND** missing sources produce a stable diagnostic rather than a silent no-op

### Requirement: sgc test SHALL report structured assertion failures

`sgc test` SHALL extend the existing JSON and text reporting protocol with a
machine-parseable assertion envelope when a test exits non-zero because of a
failed `std::assert` helper.

#### Scenario: Assertion failures use a frozen machine envelope

- **WHEN** a test process calls a typed helper from `std::assert` such as
  `assert_eq_i64(expected, actual)` and the assertion fails
- **THEN** the process exits with status code `1`
- **AND** `sgc test` has provided a unique absolute result path through
  `SENGOO_ASSERT_REPORT`
- **AND** the process writes one bounded UTF-8 JSON line to that path before exit
  using this schema:

```json
{
  "schema_version": 1,
  "kind": "assertion_failure",
  "helper": "assert_eq_i64",
  "message": "expected 7, got 9",
  "file": "tests/smoke.sg",
  "line": 12,
  "expected": "7",
  "actual": "9"
}
```

- **AND** field `file` and `line` identify the callsite when source location is
  available; otherwise those fields are omitted
- **AND** fields `expected` and `actual` are omitted when the helper has no typed
  operands to serialize
- **AND** `schema_version` is integer `1`, `kind` is the literal
  `assertion_failure`, `helper` and `message` are UTF-8 strings, and `line` is a
  positive integer when present
- **AND** `sgc test` reads at most 64 KiB, accepts only envelope schema version
  `1`, removes the result file after reading, and MUST NOT depend on parsing
  panic text from stderr

#### Scenario: Assertion transport works with and without stream capture

- **WHEN** `sgc test` runs in default capture mode or with `--nocapture`
- **THEN** the assertion result file is still created, passed, read, and removed
  on Windows and POSIX
- **AND** stdout/stderr inheritance does not disable structured assertion reporting

#### Scenario: Missing or malformed envelopes do not hide test failures

- **WHEN** a non-zero test exit produces no envelope, an oversized envelope, an
  unknown schema version, or malformed JSON
- **THEN** the test remains failed
- **AND** a missing envelope follows the ordinary failure-report path
- **AND** malformed, oversized, or unsupported envelopes add a bounded
  assertion-transport diagnostic without being treated as trusted assertion data

#### Scenario: Assertions outside sgc test preserve existing failure behavior

- **WHEN** an assertion fails and `SENGOO_ASSERT_REPORT` is not set
- **THEN** the process still terminates non-zero through the existing assertion
  panic path
- **AND** no implicit report path is guessed or created

#### Scenario: Text output remains human-readable

- **WHEN** `sgc test` runs in text mode and an assertion envelope is present
- **THEN** the failing test section prints the assertion `message`
- **AND** captured stdout/stderr from the test process are still shown for the case

#### Scenario: JSON output extends the existing report without breaking schema

- **WHEN** `sgc test --format json` runs and a test fails with an assertion envelope
- **THEN** the failing test entry includes an `assertion` object containing the
  envelope fields
- **AND** existing top-level fields (`schema_version`, `capture`, `exit_status`,
  `tests`) remain backward compatible

### Requirement: Realworld locked-loop verification SHALL use real toolchain binaries

Project integration tests for `examples/realworld` SHALL execute locked `sgpm`
commands against real `sgc`, `sgpm`, and `sgfmt` binaries rather than stub
executables.

#### Scenario: Realworld e2e job uses real tools

- **WHEN** CI runs the `realworld-e2e` job on a host with the native toolchain
- **THEN** `sgpm update`, `sgpm check --locked`, `sgpm test --locked`,
  `sgpm fmt --check --locked`, `sgpm doc --locked`, and `sgpm build --locked`
  succeed for every `examples/realworld/*` fixture
- **AND** locked commands do not rewrite `Sengoo.lock` content

#### Scenario: Missing native toolchain is an explicit skip, not a fake stub

- **WHEN** the realworld e2e job runs on a host without the required native toolchain
- **THEN** the job is skipped with documented evidence
- **AND** no fake `sgc` or `sgfmt` executable is substituted

### Requirement: Internal developer docs SHALL cover debugger, editor, and release workflows

Sengoo SHALL publish internal-only workflow docs for debugging native artifacts,
editor setup, and versioned toolchain release.

#### Scenario: Debugger quickstart exists

- **WHEN** a developer opens `docs/debugging-native.md`
- **THEN** it explains how to build with debug symbols and attach `lldb` or the
  documented Windows debugger to a `sgc build` artifact

#### Scenario: Editor setup matches CLI diagnostics

- **WHEN** a developer opens `docs/editor-setup.md`
- **THEN** it documents `sglsp` launch, fmt-on-save, and JSON diagnostic parity with
  `sgc --error-format json`

#### Scenario: Internal release channel is documented

- **WHEN** a developer opens `docs/internal-release.md`
- **THEN** it explains versioned `sgc`, `sgpm`, `sgfmt`, and `sglsp` binaries,
  smoke tests before tagging, and rollback steps

### Requirement: Internal toolchain releases SHALL have auditable smoke and rollback

Sengoo SHALL make internal toolchain releases auditable by building the full
tool set, recording archive metadata/checksums, running realworld smoke, and
documenting rollback.

#### Scenario: Release smoke builds the full tool set

- **WHEN** a release candidate is prepared on a supported host
- **THEN** the release smoke builds `sgc`, `sgpm`, `sgfmt`, and `sglsp` in the
  selected release profile
- **AND** the smoke runs realworld locked package loops with real binaries
- **AND** the smoke includes `sglsp` realworld diagnostics or documents an
  evidenced host/tooling skip

#### Scenario: Release archive has a manifest and checksums

- **WHEN** the release archive is assembled
- **THEN** its manifest records tool versions, git SHA, host triple, bundled
  stdlib/runtime contents, archive filename, and sha256 checksums
- **AND** `docs/internal-release.md` documents how maintainers verify the archive
  before tagging

#### Scenario: Rollback verifies package compatibility

- **WHEN** a maintainer rolls back to a previous toolchain archive
- **THEN** the documented rollback runs `sgpm update --check` and the locked
  package loop before declaring the rollback healthy
- **AND** any lockfile incompatibility produces an actionable diagnostic rather
  than silently rewriting lockfiles

#### Scenario: Quickstart documents release package workflow

- **WHEN** a maintainer opens `docs/sgpm-quickstart.md`
- **THEN** it shows deterministic publish dry-run, local registry publish,
  remote registry credential guidance, and
  `sgpm metadata --format json --locked` verification
- **AND** examples avoid leaking registry tokens in commands or expected output

### Requirement: Graphics ecosystem packages SHALL publish through sgpm as third-party-style packages

The repository SHALL treat `sgplatform`, `sggame`, and `sggui` as sgpm packages
under `packages/` with manifests, lockfiles, tests, and documentation rather
than as ad hoc examples or premature `tools/stdlib/` modules.

Graphics package builds SHALL use the existing package manifest schema in this
change. Native SDL2 libraries SHALL be carried by source-level FFI link metadata
and documented environment/toolchain setup rather than a new manifest section.

#### Scenario: Each graphics package is sgpm-shaped

- **WHEN** a user inspects `packages/sgplatform`, `packages/sggame`, and
  `packages/sggui`
- **THEN** each directory contains `Sengoo.toml`, source entry files, tests,
  and README instructions
- **AND** dependency edges declare `sggame -> sgplatform` and `sggui ->
  sgplatform`

#### Scenario: Locked package loop applies to graphics packages

- **WHEN** CI or a user runs `sgpm update` followed by `sgpm test --locked` and
  `sgpm build --locked` inside a graphics package on a supported host
- **THEN** the commands succeed or record an accepted platform skip documented
  in the graphics support matrix
- **AND** stale lockfiles are rejected before invoking `sgc` where locked mode
  is used

#### Scenario: Native link schema is not invented by graphics packages

- **WHEN** a reviewer inspects `packages/*/Sengoo.toml` for the graphics packages
- **THEN** the manifests use existing package, dependency, target, and test
  fields only
- **AND** any need for manifest-level native libraries is documented as a
  follow-up OpenSpec rather than implemented ad hoc

### Requirement: Graphics packages SHALL be discoverable from repository docs

The examples or packages documentation SHALL link to `sgplatform`, `sggame`, and
`sggui` quickstarts and to the graphics support matrix.

#### Scenario: User discovers graphics packages from the repo root docs

- **WHEN** a user reads the examples or packages index linked from README
- **THEN** they can find entry points for blank window, snake, and counter demos
- **AND** they can find native dependency installation instructions

