# mainstream-usable-loop Specification

## Purpose
Define the realworld package loop, support matrix, and editor/tooling evidence
that make mainstream usability claims traceable to package-shaped examples.

## Requirements
### Requirement: Realworld examples SHALL prove a package-shaped user workflow

Sengoo SHALL provide a realworld examples catalog that demonstrates how a
mainstream user creates, checks, tests, documents, formats, and builds useful
package-shaped projects rather than isolated source snippets.

#### Scenario: The realworld catalog contains at least three packages

- **WHEN** a user opens `examples/realworld`
- **THEN** the directory contains committed fixtures named `cli-json-audit`,
  `http-client-status`, and `workspace-doc-loop`
- **AND** each fixture has `Sengoo.toml`, source files, tests, and
  documentation comments
- **AND** the examples are small enough for repository integration tests
- **AND** the examples are realistic enough to exercise package workflows
- **AND** the fixtures are not generated only at test time

#### Scenario: The examples cover mainstream stdlib modules

- **WHEN** all realworld examples are considered together
- **THEN** they exercise `std::args`, `std::file`, `std::dir`, `std::json`,
  `std::process`, `std::http`, `std::log`, `std::status`, and
  `std::collections`
- **AND** each used module is imported through the normal `std::<module>`
  source import path
- **AND** examples avoid raw runtime symbols when a public stdlib wrapper exists

#### Scenario: Required modules are assigned to realworld fixtures

- **WHEN** reviewers inspect the realworld catalog
- **THEN** `cli-json-audit` covers `std::args`, `std::file`, `std::dir`,
  `std::json`, `std::log`, `std::status`, and at least one
  `std::collections` helper
- **AND** `http-client-status` imports `std::http` and `std::log` through
  public wrappers, uses JSON/status handling, and documents TLS/HTTPS or
  host-specific unsupported behavior
- **AND** `workspace-doc-loop` covers package/workspace selection, `[lib]`
  documentation, package tests, lockfile validation, and `std::process`
  invocation

#### Scenario: HTTP examples use the stable public HTTP surface

- **WHEN** `http-client-status` demonstrates HTTP behavior
- **THEN** it uses `std::http` as the primary example surface
- **AND** it does not use legacy `std::net` HTTP compatibility names as the
  primary public example
- **AND** HTTP/TLS unsupported behavior follows the upstream stdlib/runtime
  status semantics rather than redefining support in this change

#### Scenario: The examples distinguish supported and unsupported behavior

- **WHEN** an example touches a capability that may be host-specific or
  unsupported
- **THEN** the example either uses the supported subset or checks for stable
  unsupported/status diagnostics
- **AND** the example documentation does not imply broader support than the
  tested behavior proves

### Requirement: The locked sgpm project loop SHALL pass for realworld packages

Every realworld package SHALL participate in the normal package-manager loop
used by local development and CI.

#### Scenario: A package lockfile is prepared

- **WHEN** a realworld package is selected for verification
- **THEN** `sgpm update` writes or refreshes its `Sengoo.lock`
- **AND** later locked commands use that lockfile without rewriting it
- **AND** tests compare lockfile content or timestamp where practical

#### Scenario: Locked check, test, format, docs, and build pass

- **WHEN** a user runs the documented locked workflow for a realworld package
- **THEN** `sgpm check --locked` succeeds
- **AND** `sgpm test --locked` succeeds and runs package tests
- **AND** `sgpm fmt --check --locked` succeeds without rewriting files
- **AND** `sgpm doc --locked` succeeds and emits deterministic documentation
- **AND** `sgpm build --locked` succeeds or records an accepted
  platform-specific unsupported path
- **AND** the documented commands start from the repository root and then `cd`
  into the selected `examples/realworld/<example>` directory

#### Scenario: Locked failures are actionable

- **WHEN** a lockfile is stale, a manifest is malformed, a package is
  ambiguous, or a selected feature is unsupported
- **THEN** `sgpm` fails before delegating to unrelated tools when possible
- **AND** the diagnostic identifies the manifest or package involved
- **AND** the diagnostic includes the remediation command or support category

### Requirement: CLI, test, doc, and LSP diagnostics SHALL be consistent

Sengoo SHALL use the realworld examples, or reduced fixtures derived from them,
to verify that developer-facing tools agree on imports, diagnostics, formatting,
and symbols.

#### Scenario: sgc diagnostics are machine-readable

- **WHEN** a representative realworld source failure is checked with
  `sgc --error-format json`
- **THEN** the output is machine-readable and schema-tested enough for tools
- **AND** the diagnostic location points at the failing source

#### Scenario: Representative failure classes are covered

- **WHEN** diagnostic consistency tests are reviewed
- **THEN** they include a stale-lockfile failure that checks package or
  manifest context plus a remediation command
- **AND** they include a missing or malformed import failure that checks source
  location and import name through `sgc --error-format json` and `sglsp`
  diagnostics
- **AND** they include an unsupported runtime capability failure or accepted
  skip that checks a stable status, compiler diagnostic, or support category

#### Scenario: sgpm diagnostics preserve package context

- **WHEN** `sgpm check`, `sgpm test`, `sgpm fmt`, `sgpm doc`, or `sgpm build`
  reports a realworld failure
- **THEN** the diagnostic preserves package or workspace selection context
- **AND** failures do not appear as unrelated delegated-tool crashes when sgpm
  can detect the problem first

#### Scenario: sglsp understands realworld imports

- **WHEN** `sglsp` analyzes a realworld example or a reduced fixture with the
  same imports
- **THEN** imported stdlib symbols are available for completion, hover,
  signature help, definition, diagnostics, and formatting tests
- **AND** missing or malformed imports produce editor diagnostics consistent
  with `sgc`
- **AND** reduced fixtures are named and documented so reviewers can trace them
  back to a realworld example import set

### Requirement: Gaps SHALL be classified as supported, unsupported, deferred, platform-specific, or accepted-risk

The repository SHALL publish a current support matrix for gaps that materially
affect mainstream usability. `examples/realworld/SUPPORT_MATRIX.md` SHALL be
the single user-facing fact source for this lane's support/gaps matrix, and
README/quickstart/example docs SHALL link to it rather than duplicating support
semantics. `Accepted risk` rows SHALL be used only when a capability exists and
has internal/runtime evidence, but does not yet have enough realworld or
reference-host proof to be claimed as supported.

#### Scenario: The support matrix has a stable table shape

- **WHEN** a user opens `examples/realworld/SUPPORT_MATRIX.md`
- **THEN** it includes rows with `Capability`, `Status`, `Host scope`,
  `Proof example/test`, `Stable diagnostic/status`, and `Upstream spec/change`
  columns
- **AND** each row links support claims to a realworld example, test, upstream
  spec/change, or documented accepted platform skip

#### Scenario: Async and task lifecycle gaps are classified

- **WHEN** the support matrix describes async behavior
- **THEN** it classifies async IO wakeups, user-defined Future support,
  multi-operand select, select loser cancellation, timeout behavior, and task
  cancellation boundaries as supported, unsupported, deferred,
  platform-specific, or accepted-risk
- **AND** unsupported async behavior maps to a stable compiler diagnostic or
  `STATUS_UNSUPPORTED` path where applicable

#### Scenario: Stdlib deferred features are classified

- **WHEN** the support matrix describes standard-library behavior
- **THEN** it classifies compression, TLS/HTTPS behavior, terminal/fd APIs,
  recursive file transfer, shell pipelines, background processes, process
  cancellation, dynamic FFI availability, and owned-string return boundaries
- **AND** each unsupported runtime behavior has a stable status or documented
  accepted skip
- **AND** accepted-risk rows cite the runtime/internal proof and name the missing
  realworld or reference-host evidence needed to promote the row to supported

#### Scenario: Tooling gaps are classified

- **WHEN** the support matrix describes package, test, doc, and LSP behavior
- **THEN** it identifies which workflows are verified by realworld examples
- **AND** it identifies known unsupported or deferred behavior without requiring
  users to inspect OpenSpec archives or source code

### Requirement: User-facing documentation SHALL teach the realworld loop

README, quickstart, and examples documentation SHALL point users to the
realworld examples and the locked project workflow.

#### Scenario: A user follows the README path

- **WHEN** a user reads the top-level README
- **THEN** it links to `examples/realworld`
- **AND** it shows the high-level locked workflow commands
- **AND** it links to the support/gaps matrix

#### Scenario: A user follows the sgpm quickstart path

- **WHEN** a user reads `docs/sgpm-quickstart.md`
- **THEN** it explains how to run the realworld package loop with `sgpm update`
  followed by locked check, test, format-check, doc, and build commands
- **AND** it explains how unsupported or platform-specific behavior is surfaced
  in that workflow
