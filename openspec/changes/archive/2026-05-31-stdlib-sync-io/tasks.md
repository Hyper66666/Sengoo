## 1. Phase 1 - `std::io`

- [x] 1.1 Add compiler surface tests proving `std::io` imports expose stdin, stdout, stderr, and flush helpers.
- [x] 1.2 Add `sgc` stdlib import expansion tests for `import std::io;` including `ffi` and `Result` dependencies.
- [x] 1.3 Add `sglsp` stdlib symbol/signature tests for `std::io`.
- [x] 1.4 Add runtime smoke coverage proving stdin input can be read into a `Buffer` and stdout/stderr writes are observable.
- [x] 1.5 Implement `tools/stdlib/io.sg` with safe wrappers and `_raw` helpers.
- [x] 1.6 Add C runtime support in `tools/stdlib/runtime.c` for stdin reads, stdin line reads, stdout/stderr writes, and stdout/stderr flush.
- [x] 1.7 Wire `io` into `tools/sgc/src/stdlib_imports.rs` and `tools/sglsp/src/stdlib.rs`.
- [x] 1.8 Add `examples/stdlib/13_io.sg` and document it in `examples/stdlib/README.md`.
- [x] 1.9 Update `tools/stdlib/README.md` with the `std::io` contract and async/TTY/owned-string deferrals.
- [x] 1.10 Verify: focused red/green tests, `cargo fmt --check`, `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sglsp`, `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`, `cmd /c openspec validate stdlib-sync-io --strict`, and `git diff --check`.

## 2. Process invariants

- [x] 2.1 `std::io` is discoverable through `sglsp` completion/signature paths.
- [x] 2.2 `std::io` has at least one runnable example.
- [x] 2.3 The change does not add async I/O, terminal control, file descriptor APIs, new dependencies, or syntax changes.
