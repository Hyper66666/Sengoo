## 1. Phase 1 - `std::path`

- [x] 1.1 Add compiler surface tests for `std::path` path predicates and Buffer-backed output helpers.
- [x] 1.2 Add `sgc` stdlib import expansion tests for `import std::path;` including `ffi`/`Result` dependencies.
- [x] 1.3 Add `sglsp` stdlib symbol/signature tests for `std::path`.
- [x] 1.4 Implement `tools/stdlib/path.sg` with safe wrappers and `_raw` helpers where needed.
- [x] 1.5 Add C runtime support in `tools/stdlib/runtime.c` for separator, absolute checks, join, parent, file name, stem, extension, and lexical normalization.
- [x] 1.6 Wire `path` into `tools/sgc/src/stdlib_imports.rs` and `tools/sglsp/src/stdlib.rs`.
- [x] 1.7 Add `examples/stdlib/08_path.sg` and document it in `examples/stdlib/README.md`.
- [x] 1.8 Update `tools/stdlib/README.md` with the `std::path` contract and limitations.
- [x] 1.9 Verify: focused red/green tests, `cargo fmt --check`, `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sglsp`, and `git diff --check`.

## 2. Phase 2 - process usability gate

- [x] 2.1 Audit the runtime ABI and existing stdlib conventions for a safe process module boundary.
- [x] 2.2 Decide whether command execution is in scope or whether this phase should only cover process metadata/exit helpers.
- [x] 2.3 Record command execution and command-line argument access as deferred unless a follow-up OpenSpec specifies ABI/security details.
- [x] 2.4 Add compiler surface tests for `std::process` metadata and Buffer-backed current-dir helpers.
- [x] 2.5 Add `sgc` stdlib import expansion tests for `import std::process;` including `ffi`/`Result` dependencies.
- [x] 2.6 Add `sglsp` stdlib symbol/signature tests for `std::process`.
- [x] 2.7 Implement `tools/stdlib/process.sg` with safe wrappers and `_raw` helpers where needed.
- [x] 2.8 Add C runtime support in `tools/stdlib/runtime.c` for process ID and current working directory length/copy.
- [x] 2.9 Wire `process` into `tools/sgc/src/stdlib_imports.rs` and `tools/sglsp/src/stdlib.rs`.
- [x] 2.10 Add `examples/stdlib/09_process.sg` and document it in `examples/stdlib/README.md`.
- [x] 2.11 Update `tools/stdlib/README.md` with the `std::process` contract and deferred command/argv limitations.
- [x] 2.12 Verify: focused red/green tests, `cargo fmt --check`, `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sglsp`, `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`, and `git diff --check`.

## 3. Phase 3 - data-format and collection ergonomics gate

- [x] 3.1 Audit whether JSON-like helpers are viable before an owned-string/byte-slice ABI lands.
- [x] 3.2 Promote `collections` into `examples/stdlib` as a first-class stdlib example entry.
- [x] 3.3 Gate `Vec<&str>` / `HashMap<&str, ...>` support on compiler/runtime evidence rather than assuming generic containers are fully general.
- [x] 3.4 Add an `examples/stdlib/10_collections.sg` runtime smoke example covering supported `Vec<i64>` / `HashMap<i64, i64>` / iterator flows.
- [x] 3.5 Add `sgc` example smoke coverage and update `examples/stdlib/README.md`.
- [x] 3.6 Update `tools/stdlib/README.md` with the current collection shape limits and JSON/string-collection deferrals.
- [x] 3.7 Verify: focused red/green test, `cargo fmt --check`, `cargo test -p sgc`, `cmd /c openspec validate stdlib-mainstream-usability --strict`, and `git diff --check`.

## 4. Phase 4 - command-line argument ABI

- [x] 4.1 Add compiler surface tests proving `std::args` imports compile and args users get an opt-in `main(argc, argv)` wrapper.
- [x] 4.2 Add a compiler regression test proving programs that do not call args helpers still emit the existing zero-argument `main` shape.
- [x] 4.3 Add `sgc` stdlib import expansion tests for `import std::args;` including `ffi`, `Buffer`, and `Result` dependencies.
- [x] 4.4 Add `sglsp` stdlib symbol/signature tests for `std::args`.
- [x] 4.5 Implement `tools/stdlib/args.sg` with safe wrappers for argument count, existence, length, and Buffer-backed copy.
- [x] 4.6 Add C runtime support in `tools/stdlib/runtime.c` for initializing and reading `argc`/`argv` user arguments.
- [x] 4.7 Implement opt-in compiler codegen for the args main wrapper while preserving mapped calls to the source-level user `main`.
- [x] 4.8 Forward trailing `sgc run` args through native and `lli` execution without including args in compile-cache keys.
- [x] 4.9 Wire `args` into `tools/sgc/src/stdlib_imports.rs` and `tools/sglsp/src/stdlib.rs`.
- [x] 4.10 Add `examples/stdlib/11_args.sg` and document it in `examples/stdlib/README.md`.
- [x] 4.11 Update `tools/stdlib/README.md` with the `std::args` contract and the command-execution deferral.
- [x] 4.12 Verify: focused red/green tests, `cargo fmt --check`, `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sglsp`, `cargo clippy -p sengoo-compiler -p sgc -p sglsp --all-targets -- -D warnings`, `cmd /c openspec validate stdlib-mainstream-usability --strict`, and `git diff --check`.

## 5. Process invariants

- [x] 5.1 Every new stdlib module is discoverable through `sglsp` completion/definition/signature paths.
- [x] 5.2 Every new stdlib module has at least one runnable example.
- [x] 5.3 Every runtime-produced string output uses the managed `Buffer` convention until an owned-string ABI is specified.
- [x] 5.4 No new external dependency is introduced without an explicit OpenSpec update.
