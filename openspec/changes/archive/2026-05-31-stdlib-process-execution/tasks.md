## 1. Specification

- [x] 1.1 Add OpenSpec proposal, design, tasks, and spec delta for shell-free synchronous process execution.
- [x] 1.2 Validate the OpenSpec change with `openspec validate --strict`.

## 2. Tests First

- [x] 2.1 Extend compiler surface tests for `process_run` helpers.
- [x] 2.2 Extend `sgc` stdlib import expansion tests for the process helpers.
- [x] 2.3 Extend `sglsp` stdlib symbol/signature tests for the process helpers.
- [x] 2.4 Add native runtime smoke coverage for exit codes and literal argument boundaries.
- [x] 2.5 Add runnable example smoke coverage.

## 3. Implementation

- [x] 3.1 Extend `tools/stdlib/process.sg` with safe fixed-arity wrappers and a raw bridge.
- [x] 3.2 Add dependency-free Windows and Unix-like process execution support in `tools/stdlib/runtime.c`.
- [x] 3.3 Add `examples/stdlib/17_process_run.sg` and document it in `examples/stdlib/README.md`.
- [x] 3.4 Update `tools/stdlib/README.md` with execution semantics, security boundary, and deferred capabilities.

## 4. Verification

- [x] 4.1 Run focused red/green tests for compiler, `sgc`, `sglsp`, native runtime, and example smoke coverage.
- [x] 4.2 Run `cargo fmt --check`.
- [x] 4.3 Run `cargo test -p sengoo-compiler --lib`.
- [x] 4.4 Run `cargo test -p sgc`.
- [x] 4.5 Run `cargo test -p sglsp`.
- [x] 4.6 Run `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`.
- [x] 4.7 Run `cmd /c openspec validate stdlib-process-execution --strict`.
- [x] 4.8 Run `git diff --check`.
