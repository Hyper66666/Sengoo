## 1. Specification

- [x] 1.1 Add OpenSpec proposal, design, tasks, and spec delta for non-recursive `std::dir` listing.
- [x] 1.2 Validate the OpenSpec change with `openspec validate --strict`.

## 2. Tests First

- [ ] 2.1 Add compiler surface tests for `dir_entry_count` and `dir_entry_name`.
- [ ] 2.2 Extend `sgc` stdlib import expansion tests for the listing helpers.
- [ ] 2.3 Extend `sglsp` stdlib symbol/signature tests for the listing helpers.
- [ ] 2.4 Add runtime smoke coverage for deterministic listing order, Buffer-backed name copy, empty directories, and out-of-range errors.

## 3. Implementation

- [ ] 3.1 Extend `tools/stdlib/dir.sg` with safe wrappers and raw helpers.
- [ ] 3.2 Add C runtime support in `tools/stdlib/runtime.c` for sorted non-recursive directory entry count/name lookup.
- [ ] 3.3 Add `examples/stdlib/15_dir_listing.sg` and document it in `examples/stdlib/README.md`.
- [ ] 3.4 Update `tools/stdlib/README.md` with the listing contract and limitations.

## 4. Verification

- [ ] 4.1 Run focused red/green tests for compiler, `sgc`, `sglsp`, runtime smoke, and example smoke coverage.
- [ ] 4.2 Run `cargo fmt --check`.
- [ ] 4.3 Run `cargo test -p sengoo-compiler --lib`.
- [ ] 4.4 Run `cargo test -p sgc`.
- [ ] 4.5 Run `cargo test -p sglsp`.
- [ ] 4.6 Run `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`.
- [ ] 4.7 Run `cmd /c openspec validate stdlib-directory-listing --strict`.
- [ ] 4.8 Run `git diff --check`.
