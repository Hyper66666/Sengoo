## 1. Specification

- [x] 1.1 Add OpenSpec proposal, design, tasks, and spec delta for shell-free synchronous process execution.
- [x] 1.2 Validate the OpenSpec change with `openspec validate --strict`.

## 2. Tests First

- [ ] 2.1 Extend compiler surface tests for `process_run` helpers.
- [ ] 2.2 Extend `sgc` stdlib import expansion tests for the process helpers.
- [ ] 2.3 Extend `sglsp` stdlib symbol/signature tests for the process helpers.
- [ ] 2.4 Add native runtime smoke coverage for exit codes and literal argument boundaries.
- [ ] 2.5 Add runnable example smoke coverage.

## 3. Implementation

- [ ] 3.1 Extend `tools/stdlib/process.sg` with safe fixed-arity wrappers and a raw bridge.
- [ ] 3.2 Add dependency-free Windows and Unix-like process execution support in `tools/stdlib/runtime.c`.
- [ ] 3.3 Add `examples/stdlib/17_process_run.sg` and document it in `examples/stdlib/README.md`.
- [ ] 3.4 Update `tools/stdlib/README.md` with execution semantics, security boundary, and deferred capabilities.

## 4. Verification

- [ ] 4.1 Run focused red/green tests for compiler, `sgc`, `sglsp`, native runtime, and example smoke coverage.
- [ ] 4.2 Run `cargo fmt --check`.
- [ ] 4.3 Run `cargo test -p sengoo-compiler --lib`.
- [ ] 4.4 Run `cargo test -p sgc`.
- [ ] 4.5 Run `cargo test -p sglsp`.
- [ ] 4.6 Run `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`.
- [ ] 4.7 Run `cmd /c openspec validate stdlib-process-execution --strict`.
- [ ] 4.8 Run `git diff --check`.
