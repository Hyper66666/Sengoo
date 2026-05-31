## 1. Phase 1 - `std::dir`

- [ ] 1.1 Add compiler surface tests proving `std::dir` imports expose directory predicates and fallible helpers.
- [ ] 1.2 Add `sgc` stdlib import expansion tests for `import std::dir;` including `ffi` and `Result` dependencies.
- [ ] 1.3 Add `sglsp` stdlib symbol/signature tests for `std::dir`.
- [ ] 1.4 Implement `tools/stdlib/dir.sg` with safe wrappers and `_raw` helpers.
- [ ] 1.5 Add C runtime support in `tools/stdlib/runtime.c` for directory existence, single create, recursive create, and empty remove.
- [ ] 1.6 Wire `dir` into `tools/sgc/src/stdlib_imports.rs` and `tools/sglsp/src/stdlib.rs`.
- [ ] 1.7 Add `examples/stdlib/12_dir.sg` and document it in `examples/stdlib/README.md`.
- [ ] 1.8 Update `tools/stdlib/README.md` with the `std::dir` contract and directory listing/removal deferrals.
- [ ] 1.9 Verify: focused red/green tests, `cargo fmt --check`, `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sglsp`, `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`, `cmd /c openspec validate stdlib-directory-helpers --strict`, and `git diff --check`.

## 2. Process invariants

- [ ] 2.1 `std::dir` is discoverable through `sglsp` completion/signature paths.
- [ ] 2.2 `std::dir` has at least one runnable example.
- [ ] 2.3 The change does not add recursive deletion, directory listing, new dependencies, or syntax changes.
