## 1. Debug info validation

- [ ] 1.1 Verify DWARF line tables map to source lines for stepping across the
  core language (scalars, structs, enums, strings, `Vec`, calls, closures).
  - Partial: `cargo test -p sgc debug_info_line_table_survives_object_compilation`
    now compiles `--debug-info` LLVM IR to a native object and uses
    `llvm-dwarfdump --debug-line` to assert that the Sengoo source file appears
    in the DWARF line table.
  - Partial: `cargo test -p sgc debug_info_tracks_multi_surface_function_entry_lines`
    now compiles a single debug-info object that exercises scalar, struct,
    enum, string, `Vec`, call, and closure surfaces, then uses
    `llvm-dwarfdump --debug-line` plus `--debug-info` to assert that each
    surface's function entry line survives object compilation as a source line
    row and `DW_AT_decl_line`.
  - Remaining: this still validates function entry/source mapping, not
    end-to-end debugger stepping through every statement on every surface.
- [ ] 1.2 Verify variable/param location + type info so a debugger shows correct
  names, types, and values.
  - Partial: LLVM debug-info emission now preserves source-level parameter and
    user-local names through MIR and emits `llvm.dbg.value` / `llvm.dbg.declare`
    plus `DILocalVariable` metadata. `cargo test -p sgc
    debug_info_emits_parameter_and_local_variable_dies -- --nocapture`
    compiles a debug-info object and uses `llvm-dwarfdump --debug-info` to
    assert `DW_TAG_formal_parameter` (`value`), `DW_TAG_variable` (`doubled`),
    and the `i64` base type survive object compilation.
  - Partial: `cargo test -p sgc --test debugger_native -- --nocapture` now
    reads scalar parameter `value` and local `doubled` through LLDB/CDB when
    the platform debugger is installed. Broader type/value coverage remains
    coupled to the multi-surface stepping work in task 1.1.
  - Partial: struct debug metadata now emits named `DW_TAG_member` entries
    with base types, sizes, and aligned offsets. `cargo test -p sgc
    debug_info_emits_struct_member_names_types_and_offsets -- --nocapture`
    verifies `Pair.left: i64` and `Pair.enabled: bool` after native object
    compilation.
  - Partial: `cargo test -p sgc
    debug_info_emits_enum_tuple_string_and_vec_composite_layouts --
    --nocapture` verifies native-object DIEs for tuple fields, owned
    `String.handle`, monomorphized `Vec_i64.handle/marker`, and the enum ABI's
    `discriminant: i64` plus bounded `payload: u8[N]` storage. The regression
    also requires each composite local to retain a `DW_AT_location` and type
    reference. MIR currently erases source enum and variant names, so named
    enum variants and live debugger reads for all composite values remain
    open rather than being inferred from generic ABI metadata.
- [ ] 1.3 Add an automated test that drives lldb (Linux/macOS) and cdb (Windows)
  to set a breakpoint, step, and read a local, asserting on output.
  - `tools/sgc/tests/debugger_native.rs` builds a fresh `-O 0 --debug-info`
    executable, drives LLDB in batch mode on Unix or CDB from a command file on
    Windows, and requires breakpoint/step markers plus `value = 21` and
    `doubled = 42` in the debugger transcript.
  - Command generation and transcript parsing have platform-independent unit
    coverage. Missing clang/debugger tools produce a visible `SKIP
    debugger_native::...` reason; once present, any build, breakpoint, step, or
    value-reading failure is a hard test failure. The current Windows reference
    host records the CDB-missing skip path rather than claiming a live CDB run.
  - Remaining: record one live Windows CDB transcript and one live Unix-family
    LLDB transcript on release hosts; skip-path coverage alone does not close
    this task.

## 2. Editor / DAP integration

- [x] 2.1 Provide a DAP bridge or a documented lldb/cdb launch configuration.
  - `docs/debugging-native.md` now includes VS Code `cppvsdbg` and `lldb`
    launch/task snippets that build with `sgc build -O 0 --debug-info`.
- [x] 2.2 Wire a "Debug Sengoo file" flow into the VS Code extension.
  - The existing `vscode-sengoo` extension contributes `type: "sengoo"`,
    F5/default configurations, and an inline DAP wrapper for `sgc run` /
    build-and-run. Source-level stepping remains covered by the native
    debugger launch documentation and task 1.x validation.
- [x] 2.3 Document the editor debug setup in `docs/debugging-native.md` /
  `docs/editor-setup.md`.

## 3. Test discovery and fixtures

- [x] 3.1 `#[test]` attribute (and/or `def test_*` convention) discovered by
  `sgc test`.
  - Partial: existing `sgc test` discovery covers `tests/**/*.sg` and manifest
    `[[test]]` targets. `tests/**/*.sg` files without their own `main` now
    expand top-level `def test_*` functions and `#[test]` functions into
    per-function generated harnesses, stripping the tooling-only `#[test]`
    marker before invoking the normal compiler path. Mixed `main` +
    function-test files remain open.
- [x] 3.2 Setup/teardown fixtures with documented ordering.
  - Function-test generated harnesses call top-level `setup()` before each
    bool/i64/unit test and `teardown()` after the test before returning the
    harness status. File-level `main` tests keep legacy behavior.
- [x] 3.3 Parametrized / table-driven cases.
  - `#[case("label", ARG)]` lines immediately before a function generate one
    harness per case, name cases as `path.sg::function[label]`, pass `ARG` as
    the first function argument, and emit JSON `parameters` entries for
    `case` and `arg0`.
- [x] 3.4 Tests covering discovery, fixtures, and parametrization.
  - Unit coverage validates `def test_*`, `#[test]`, UTF-8 BOM handling,
    setup/teardown fixtures, and `#[case]` harness generation. Manual smoke
    verified `sgc test --format json` on generated fixtures and parameterized
    cases.

## 4. Failure output and coverage

- [x] 4.1 Extend `std::assert` structured failures (expected/actual, file/line)
  in the existing JSON envelope.
  - Existing implementation emits schema-v1 assertion envelopes through
    `std::assert`, records callsite file/line plus expected/actual payloads,
    and is validated by `cargo test -p sgc --test assertion_transport -- --nocapture`.
- [x] 4.2 `sgc test --coverage` line-coverage report (machine-readable + summary).
  - `--coverage` now enables coverage-only HIR/MIR statement probes. The test
    binary registers every executable source probe at startup and writes only
    actual runtime hits to its per-test report; the runner unions `(source,
    line)` records across cases before emitting the v1 `covered_lines`,
    `executable_lines`, and `percent` fields in text and JSON output.
    Coverage binaries use a `.coverage` artifact/cache namespace so a later
    ordinary run cannot reuse instrumented output.
  - `cargo test -p sgc --test coverage_runtime -- --nocapture` proves an
    executed path records non-zero hits while an untaken branch and an uncalled
    function keep the report below 100%. A compiler regression proves ordinary
    compilation emits no coverage calls.
- [x] 4.3 Extend the `sgc test` JSON schema with optional coverage and
  parametrization fields, keeping existing fields.
  - The JSON report data model now emits optional report-level `coverage` when
    `--coverage` is active and per-test `parameters` for parameterized cases.
    Normal tests omit optional fields. Tests assert the future-field acceptance
    path and default omission compatibility path.

## 5. Docs and validation

- [x] 5.1 Document the test framework in `docs/` and `tools/stdlib/README.md`.
  - `docs/sgpm-quickstart.md` and `tools/stdlib/README.md` document current
    `sgc test` discovery, `#[test]` generated harness behavior,
    setup/teardown fixture ordering, `#[case]` parametrization, JSON fields,
    and `--coverage` source-line output.
    `tools/stdlib/README.md` also documents `std::assert` schema-v1 assertion
    envelopes and the `sgc test --format json` reserved-field compatibility
    rule.
- [x] 5.2 Migrate at least one realworld smoke test to fixtures + parametrization.
  - `examples/realworld/compressed-json-artifact/tests/compress_smoke.sg`
    now uses `setup()` / `teardown()` plus `#[case("array3", 3)]`, and
    `cargo run -p sgc -- test examples\realworld\compressed-json-artifact --format json`
    reports `tests/compress_smoke.sg::compress_smoke[array3]` with JSON
    parameters.
- [x] 5.3 Run `openspec validate debugger-and-test-framework --strict`.

## Verification

- The lldb/cdb stepping test (task 1.3) passes when the platform debugger is
  installed and reports an explicit skip reason when it is absent
- `sgc test` discovers `#[test]` cases, runs fixtures/parametrization, and emits
  coverage
- `cargo test -p sgc` test-command lanes remain green
