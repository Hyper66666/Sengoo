## 1. Specification

- [x] 1.1 Add OpenSpec proposal, design, tasks, and spec delta for `std::strconv`.
- [x] 1.2 Validate the OpenSpec change with `openspec validate --strict`.

## 2. Tests First

- [x] 2.1 Add compiler surface tests for `std::strconv` parse/format wrappers.
- [x] 2.2 Add `sgc` stdlib import expansion tests for `std::strconv` dependencies.
- [x] 2.3 Add `sglsp` stdlib symbol/signature tests for `std::strconv`.
- [x] 2.4 Add runtime smoke coverage for parsing valid, invalid, overflow, and Buffer-backed input plus formatting.

## 3. Implementation

- [x] 3.1 Implement `tools/stdlib/strconv.sg` with safe wrappers and raw helpers.
- [x] 3.2 Add C runtime support in `tools/stdlib/runtime.c` for decimal `i64` parse/format and last error tracking.
- [x] 3.3 Wire `strconv` into `tools/sgc/src/stdlib_imports.rs` and `tools/sglsp/src/stdlib.rs`.
- [x] 3.4 Add `examples/stdlib/14_strconv.sg` and document it in `examples/stdlib/README.md`.
- [x] 3.5 Update `tools/stdlib/README.md` with the `std::strconv` contract and limitations.

## 4. Verification

- [x] 4.1 Run focused red/green tests for compiler, `sgc`, `sglsp`, and runtime smoke coverage.
- [x] 4.2 Run `cargo fmt --check`.
- [x] 4.3 Run `cargo test -p sengoo-compiler --lib`.
- [x] 4.4 Run `cargo test -p sgc`.
- [x] 4.5 Run `cargo test -p sglsp`.
- [x] 4.6 Run `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`.
- [x] 4.7 Run `cmd /c openspec validate stdlib-string-conversions --strict`.
- [x] 4.8 Run `git diff --check`.
