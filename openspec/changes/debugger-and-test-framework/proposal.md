## Why

Two developer-experience gaps remain after `six-pillar-gap-closure`:

- **Debugging**: the compiler emits DWARF (`--debug-info`/`-g`,
  `native-debug-info`), and `docs/debugging-native.md` documents manual
  lldb/Windows steps, but there is no validated source-level stepping and
  variable-inspection experience — and no integrated/editor debugging.
- **Testing**: `sgc test` exists and `std::assert` has typed helpers with a JSON
  envelope, but the framework lacks fixtures/setup-teardown, parametrized cases,
  test discovery conventions beyond smoke tests, and coverage reporting.

Mainstream languages ship a real debugger experience and a batteries-included
test framework. This change is largely independent of P0 and can start early.

## Proposal

**Debugging**
- Validate and harden DWARF so a debugger can set breakpoints, single-step, show
  a correct backtrace, and inspect locals/params with correct types and values
  for the core language (scalars, structs, enums, strings, `Vec`).
- Provide a Debug Adapter Protocol (DAP) bridge (or a documented lldb/cdb launch)
  wired into the VS Code extension for step/inspect from the editor.

**Test framework**
- A `#[test]` attribute (or `def test_*` convention) for test discovery.
- Setup/teardown fixtures and parametrized/table-driven cases.
- Structured failure output (expected/actual, file/line) extending the existing
  `std::assert` JSON envelope.
- Coverage reporting (line coverage) surfaced by `sgc test --coverage`.

## What changes

- ADDED: validated source-level debugging (breakpoints, stepping, backtrace,
  variable inspection) + DAP/editor integration.
- ADDED: `#[test]` discovery, fixtures, parametrization, and coverage in
  `sgc test`.
- MODIFIED (additive): `sgc test` JSON schema gains optional coverage and
  parametrization fields without removing existing fields.

## Non-goals

- Time-travel/reverse debugging, hot reload, or a custom debugger UI (reuse
  lldb/cdb/DAP).
- Mutation testing or fuzzing (proposable later).
