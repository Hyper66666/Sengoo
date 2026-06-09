## ADDED Requirements

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
