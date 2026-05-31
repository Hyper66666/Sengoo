## Why

Sengoo scripts can create directories, list entries, and read/write/remove
files, but they still cannot express two routine filesystem workflows: copying
a file and moving a file. Users currently need raw FFI or host tooling for
operations that mainstream standard libraries expose directly.

This change adds a narrow, explicit-overwrite file transfer surface.

## What Changes

- Extend `std::file` with portable copy and move helpers:
  - `file_copy(source: &str, destination: &str, overwrite: bool)
    -> Result<i64, i64>`
  - `file_move(source: &str, destination: &str, overwrite: bool)
    -> Result<bool, i64>`
  - raw pointer variants for explicit interop
- Copy returns the number of bytes copied.
- Move returns an ok-shaped boolean result when the host rename succeeds.
- Existing destinations are rejected unless `overwrite == true`.
- Wire the expanded surface through compiler tests, `sgc`, `sglsp`, docs, and a
  runnable stdlib example.
- Keep recursive directory copy/move, cross-filesystem move fallback, metadata
  preservation guarantees, progress callbacks, and atomic-copy claims out of
  scope.

## Impact

- Affected code:
  - `tools/stdlib/file.sg`
  - `tools/stdlib/runtime.c`
  - `compiler/src/tests/stdlib_surface_tests.rs`
  - `tools/sgc/src/tests.rs`
  - `tools/sgc/src/stdlib_imports.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `examples/stdlib/*`
  - `tools/stdlib/README.md`
- No source syntax changes.
- No third-party dependencies.
- Existing `std::file` behavior remains backward-compatible.
