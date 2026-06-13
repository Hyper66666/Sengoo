## 1. Debug info validation

- [ ] 1.1 Verify DWARF line tables map to source lines for stepping across the
  core language (scalars, structs, enums, strings, `Vec`, calls, closures).
- [ ] 1.2 Verify variable/param location + type info so a debugger shows correct
  names, types, and values.
- [ ] 1.3 Add an automated test that drives lldb (Linux/macOS) and cdb (Windows)
  to set a breakpoint, step, and read a local, asserting on output.

## 2. Editor / DAP integration

- [ ] 2.1 Provide a DAP bridge or a documented lldb/cdb launch configuration.
- [ ] 2.2 Wire a "Debug Sengoo file" flow into the VS Code extension.
- [ ] 2.3 Document the editor debug setup in `docs/debugging-native.md` /
  `docs/editor-setup.md`.

## 3. Test discovery and fixtures

- [ ] 3.1 `#[test]` attribute (and/or `def test_*` convention) discovered by
  `sgc test`.
- [ ] 3.2 Setup/teardown fixtures with documented ordering.
- [ ] 3.3 Parametrized / table-driven cases.
- [ ] 3.4 Tests covering discovery, fixtures, and parametrization.

## 4. Failure output and coverage

- [ ] 4.1 Extend `std::assert` structured failures (expected/actual, file/line)
  in the existing JSON envelope.
- [ ] 4.2 `sgc test --coverage` line-coverage report (machine-readable + summary).
- [ ] 4.3 Extend the `sgc test` JSON schema with optional coverage and
  parametrization fields, keeping existing fields.

## 5. Docs and validation

- [ ] 5.1 Document the test framework in `docs/` and `tools/stdlib/README.md`.
- [ ] 5.2 Migrate at least one realworld smoke test to fixtures + parametrization.
- [ ] 5.3 Run `openspec validate debugger-and-test-framework --strict`.

## Verification

- The lldb/cdb stepping test (task 1.3) passes on the reference host
- `sgc test` discovers `#[test]` cases, runs fixtures/parametrization, and emits
  coverage
- `cargo test -p sgc` test-command lanes remain green
