## 1. OpenSpec

- [x] 1.1 Inventory existing stdlib, runtime hardening, sgc test, sgpm, sgfmt,
  and sglsp specs so this change builds on current work.
- [x] 1.2 Add this proposal and design.
- [x] 1.3 Add `mainstream-usable-loop` requirements.
- [x] 1.4 Run `openspec validate mainstream-usable-loop --strict`.
- [x] 1.5 Run `openspec validate --all --strict`.
- [x] 1.6 Have a subagent review the spec for missing acceptance criteria and
  overlap with active stdlib/tooling changes.
- [x] 1.7 Before tasks 2-6 begin, merge all spec-review acceptance-gap patches
  into `proposal.md`, `design.md`, `tasks.md`, and `spec.md`, then re-run
  strict OpenSpec validation.

## 2. Realworld Examples

- [x] 2.1 Add `examples/realworld/README.md` with the project-loop commands and
  support matrix summary.
- [x] 2.2 Add committed package fixture `examples/realworld/cli-json-audit`
  with `Sengoo.toml`, `src/main.sg`, `tests/**/*.sg`, docs, and sample data;
  it must cover `std::args`, `std::file`, `std::dir`, `std::json`,
  `std::log`, `std::status`, and at least one `std::collections` helper.
- [x] 2.3 Add committed package fixture `examples/realworld/http-client-status`
  with `Sengoo.toml`, `src/main.sg`, `tests/**/*.sg`, docs, and stable
  supported or unsupported-path HTTP coverage through public `std::http` and
  `std::log` wrappers.
- [x] 2.4 Add committed workspace or dual-target fixture
  `examples/realworld/workspace-doc-loop` with root `Sengoo.toml`, at least
  one `[lib]`, docs, tests, package/workspace selection behavior, and
  `std::process` invocation.
- [x] 2.5 Add tests and fixtures so each example has at least one package test
  under `tests/**/*.sg`.
- [x] 2.6 Ensure every example has package docs discoverable through `sgpm doc`.

## 3. Locked Project Loop

- [x] 3.1 Add integration tests that run `sgpm update` and `sgpm check
  --locked` for every realworld package.
- [x] 3.2 Add integration tests that run `sgpm test --locked` for every
  realworld package.
- [x] 3.3 Add integration tests that run `sgpm fmt --check --locked` for every
  realworld package.
- [x] 3.4 Add integration tests that run `sgpm doc --locked` for every
  realworld package.
- [x] 3.5 Add integration tests that run `sgpm build --locked` for every
  realworld package.
- [x] 3.6 Add failure-path tests for stale lockfiles, unsupported runtime
  surfaces, and manifest/package selection diagnostics.
- [x] 3.7 Assert that locked commands after `sgpm update` do not rewrite
  `Sengoo.lock` content or timestamp where practical.

## 4. CLI, LSP, And Stdlib Coverage

- [x] 4.1 Add `sgc` coverage for checking/running/building the realworld
  source entries or reduced fixtures derived from them.
- [x] 4.2 Add `sglsp` tests for stdlib completion, signature help, hover,
  diagnostics, formatting, and definitions on realworld imports.
- [x] 4.3 Add stdlib/compiler/runtime coverage for any module behavior used by
  the realworld examples that is not already tested.
- [x] 4.4 Ensure JSON output or structured diagnostics used by automation have
  schema-like assertions.
- [x] 4.5 Cover the representative failure matrix: stale lockfile, missing or
  malformed import, and unsupported runtime capability or accepted platform
  skip.
- [x] 4.6 If `sglsp` uses reduced fixtures instead of a package harness, place
  them in `tools/sglsp/src/stdlib.rs` or a named sibling test module and name
  each fixture after the realworld example import set it derives from.

## 5. Gaps Matrix And Documentation

- [x] 5.1 Add `examples/realworld/SUPPORT_MATRIX.md` as the single
  user-facing support/gaps fact source with columns `Capability`, `Status`,
  `Host scope`, `Proof example/test`, `Stable diagnostic/status`, and
  `Upstream spec/change`.
- [x] 5.2 Cover async IO, task cancellation, select limitations, process
  cancellation/background execution, compression, TLS/HTTP, dynamic FFI,
  package/test/doc diagnostics, and LSP coverage in the support matrix.
- [x] 5.3 Ensure unsupported behavior is explicitly rejected or returns stable
  statuses/diagnostics instead of silent success, crashes, or unresolved
  symbols.
- [x] 5.4 Update `README.md` and `README.zh-CN.md` with the realworld workflow.
- [x] 5.5 Update `docs/sgpm-quickstart.md` with locked realworld package
  commands.
- [x] 5.6 Update `examples/README.md` to point to `examples/realworld`.

## 6. Parallel Development And Review

- [x] 6.1 Split implementation into parallel lanes for examples, sgpm/sgc
  integration, LSP/diagnostics, and docs/gaps matrix.
- [x] 6.2 Use subagents for at least spec review and independent implementation
  lanes.
- [x] 6.3 Integrate subagent outputs without reverting unrelated local changes.
- [x] 6.4 Run the full verification baseline before marking this change done.

## 7. Verification

- [x] 7.1 Run `cargo fmt --check`.
- [x] 7.2 Run `cargo test -p sengoo-compiler --lib`.
- [x] 7.3 Run `cargo test -p sgc`.
- [x] 7.4 Run `cargo test -p sengoo-runtime --lib`.
- [x] 7.5 Run `cargo test -p sgpm`.
- [x] 7.6 Run `cargo test -p sgfmt`.
- [x] 7.7 Run `cargo test -p sglsp`.
- [x] 7.8 Run every documented realworld `sgpm update/check/test/fmt/doc/build
  --locked` command or its exact integration-test equivalent.

## Done Definition

- [x] `examples/realworld` contains at least three end-to-end package examples.
- [x] The realworld examples collectively cover all requested stdlib modules.
- [x] Locked `sgpm` project-loop commands are documented and tested.
- [x] CLI/LSP/package diagnostics are consistent for representative failures.
- [x] Async/runtime/stdlib/tooling gaps are documented as supported,
  unsupported, deferred, or platform-specific.
- [x] README, quickstart, examples docs, OpenSpec, and tests are updated.
