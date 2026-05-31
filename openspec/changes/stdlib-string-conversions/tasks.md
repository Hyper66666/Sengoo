## 1. Specification

- [x] 1.1 Add OpenSpec proposal, design, tasks, and spec delta for `std::strconv`.
- [x] 1.2 Validate the OpenSpec change with `openspec validate --strict`.

## 2. Tests First

- [ ] 2.1 Add compiler surface tests for `std::strconv` parse/format wrappers.
- [ ] 2.2 Add `sgc` stdlib import expansion tests for `std::strconv` dependencies.
- [ ] 2.3 Add `sglsp` stdlib symbol/signature tests for `std::strconv`.
- [ ] 2.4 Add runtime smoke coverage for parsing valid, invalid, overflow, and Buffer-backed input plus formatting.

## 3. Implementation

- [ ] 3.1 Implement `tools/stdlib/strconv.sg` with safe wrappers and raw helpers.
- [ ] 3.2 Add C runtime support in `tools/stdlib/runtime.c` for decimal `i64` parse/format and last error tracking.
- [ ] 3.3 Wire `strconv` into `tools/sgc/src/stdlib_imports.rs` and `tools/sglsp/src/stdlib.rs`.
- [ ] 3.4 Add `examples/stdlib/14_strconv.sg` and document it in `examples/stdlib/README.md`.
- [ ] 3.5 Update `tools/stdlib/README.md` with the `std::strconv` contract and limitations.

## 4. Verification

- [ ] 4.1 Run focused red/green tests for compiler, `sgc`, `sglsp`, and runtime smoke coverage.
- [ ] 4.2 Run `cargo fmt --check`.
- [ ] 4.3 Run `cargo test -p sengoo-compiler --lib`.
- [ ] 4.4 Run `cargo test -p sgc`.
- [ ] 4.5 Run `cargo test -p sglsp`.
- [ ] 4.6 Run `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`.
- [ ] 4.7 Run `cmd /c openspec validate stdlib-string-conversions --strict`.
- [ ] 4.8 Run `git diff --check`.
