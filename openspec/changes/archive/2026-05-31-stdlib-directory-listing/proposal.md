## Why

Sengoo now has path, file, directory setup, arguments, stdin/stdout, and scalar
conversion helpers. The next everyday scripting gap is inspecting a directory:
small tools need to discover files before filtering, counting, or processing
them.

This change adds a narrow, deterministic `std::dir` listing surface without
introducing owned strings, recursive traversal, metadata structs, or iterator
types.

## What Changes

- Extend `std::dir` with non-recursive directory listing helpers:
  - `dir_entry_count(path: &str) -> Result<i64, i64>`
  - `dir_entry_name(path: &str, index: i64, buffer: Buffer) -> Result<i64, i64>`
  - raw pointer/capacity variants for explicit interop
- Runtime listing excludes `.` and `..`.
- Entry names are sorted by unsigned byte order before index lookup so results
  are deterministic across host directory iteration order.
- Wire the expanded surface through compiler tests, `sgc`, `sglsp`, docs, and a
  runnable stdlib example.
- Keep recursive traversal, recursive deletion, file metadata, pattern/glob
  matching, owned-string returns, and iterator/list objects out of scope.

## Impact

- Affected code:
  - `tools/stdlib/dir.sg`
  - `tools/stdlib/runtime.c`
  - `compiler/src/tests/stdlib_surface_tests.rs`
  - `tools/sgc/src/tests.rs`
  - `tools/sgc/src/stdlib_imports.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `examples/stdlib/*`
  - `tools/stdlib/README.md`
- No source syntax changes.
- No third-party dependencies.
- Existing `std::dir` create/remove behavior remains backward-compatible.
