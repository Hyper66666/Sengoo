## Why

Sengoo can now read arguments, inspect paths, and read/write files, but ordinary
utility programs still cannot create or remove directories without leaving the
language. Mainstream scripting workflows need at least a safe portable directory
subset for preparing output folders, test scratch roots, and cache directories.

## What Changes

- Add a `std::dir` source module for portable directory predicates and mutation
  helpers.
- Provide:
  - `dir_exists(path: &str) -> bool`
  - `dir_create(path: &str) -> Result<bool, i64>`
  - `dir_create_all(path: &str) -> Result<bool, i64>`
  - `dir_remove(path: &str) -> Result<bool, i64>`
- Add C runtime support for directory existence, single-directory creation,
  recursive creation, and empty-directory removal.
- Wire `std::dir` through `sgc`, `sglsp`, docs, and a runnable stdlib example.

## Non-Goals

- No directory listing or iteration in this change.
- No recursive deletion. Removing trees is too destructive for this small slice.
- No filesystem metadata model beyond a directory-exists predicate.
- No new source-language syntax and no third-party dependencies.

## Impact

- Affected code:
  - `tools/stdlib/dir.sg`
  - `tools/stdlib/runtime.c`
  - `tools/sgc/src/stdlib_imports.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `compiler/src/tests/stdlib_surface_tests.rs`
  - `tools/sgc/src/tests.rs`
  - `examples/stdlib/*`
  - `tools/stdlib/README.md`
- Existing `std::file`, `std::path`, and `std::process` behavior must remain
  backward-compatible.
- Verification follows the stdlib pattern: focused red/green tests,
  `cargo fmt --check`, compiler/sgc/sglsp tests, OpenSpec validation, and
  `git diff --check`.
