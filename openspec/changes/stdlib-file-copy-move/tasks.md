## 1. Specification

- [x] 1.1 Add OpenSpec proposal, design, tasks, and spec delta for `std::file` copy/move helpers.
- [x] 1.2 Validate the OpenSpec change with `openspec validate --strict`.

## 2. Tests First

- [x] 2.1 Extend compiler surface tests for `file_copy` and `file_move`.
- [x] 2.2 Extend `sgc` stdlib import expansion tests for the file helpers.
- [x] 2.3 Extend `sglsp` stdlib symbol/signature tests for the file helpers.
- [x] 2.4 Add runtime smoke coverage for copy bytes, overwrite rejection, explicit overwrite, and move lifecycle.

## 3. Implementation

- [x] 3.1 Extend `tools/stdlib/file.sg` with safe wrappers and raw helpers.
- [x] 3.2 Add C runtime support in `tools/stdlib/runtime.c` for binary file copy and host-rename move.
- [x] 3.3 Add `examples/stdlib/16_file_copy_move.sg` and document it in `examples/stdlib/README.md`.
- [x] 3.4 Update `tools/stdlib/README.md` with copy/move semantics and limitations.

## 4. Verification

- [x] 4.1 Run focused red/green tests for compiler, `sgc`, `sglsp`, runtime smoke, and example smoke coverage.
- [x] 4.2 Run `cargo fmt --check`.
- [x] 4.3 Run `cargo test -p sengoo-compiler --lib`.
- [x] 4.4 Run `cargo test -p sgc`.
- [x] 4.5 Run `cargo test -p sglsp`.
- [x] 4.6 Run `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`.
- [x] 4.7 Run `cmd /c openspec validate stdlib-file-copy-move --strict`.
- [x] 4.8 Run `git diff --check`.
