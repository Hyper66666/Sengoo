## Context

The previous `std::dir` change intentionally stopped at existence, creation,
recursive creation, and empty-directory removal. That was a safe setup surface,
but ordinary tools also need to inspect directory contents. Sengoo still lacks
owned strings and string collections, so listing must use the same Buffer-backed
convention as path/file/env/process/args/io helpers.

## Goals / Non-Goals

**Goals:**

- Add deterministic, non-recursive listing to `std::dir`.
- Preserve the current `Result<i64, i64>` and managed `Buffer` conventions for
  fallible string-like output.
- Make the runtime output stable enough for examples and tests.
- Avoid destructive operations and broad filesystem metadata scope.

**Non-Goals:**

- No recursive traversal.
- No recursive deletion.
- No glob/pattern matching.
- No file type/permission/timestamp metadata structs.
- No owned-string return ABI.
- No list/iterator object API in this change.

## API Shape

`std::dir` gains:

- `dir_entry_count(path: &str) -> Result<i64, i64>`
- `dir_entry_count_raw(path_ptr: i64) -> Result<i64, i64>`
- `dir_entry_name(path: &str, index: i64, buffer: Buffer) -> Result<i64, i64>`
- `dir_entry_name_raw(path_ptr: i64, index: i64, buffer_ptr: i64, capacity: i64)
  -> Result<i64, i64>`

The count is the number of immediate children excluding `.` and `..`. The name
helper copies one child name, not a joined path, into the caller-provided
Buffer. It returns the copied byte count and does not append a NUL terminator.

## Semantics

- Listing is non-recursive.
- `.` and `..` are never counted or copied.
- Runtime code sorts names by unsigned byte order before returning counts or
  indexed names. This avoids depending on host filesystem iteration order.
- Negative indices, out-of-range indices, invalid paths, non-directory paths,
  invalid output buffers, and too-small buffers return an error-shaped result.
- Empty directories return an ok-shaped count of `0`.

## Risks / Trade-offs

- **Risk:** Re-scanning and sorting for each indexed name is less efficient than
  a persistent iterator.
  **Mitigation:** this keeps the ABI simple until Sengoo has owned/list values;
  callers that need more can be covered by a follow-up iterator design.
- **Risk:** Byte-order sorting may differ from locale or platform UI sorting.
  **Mitigation:** deterministic byte sorting is portable, simple, and testable.
- **Risk:** Users may expect joined child paths.
  **Mitigation:** document that `dir_entry_name` returns the child name only;
  callers can combine it with `std::path::path_join`.

## Verification

- Compiler surface tests cover `dir_entry_count` and `dir_entry_name`.
- `sgc` import expansion tests prove `std::dir` still preloads Buffer/Result
  dependencies.
- `sglsp` symbol/signature tests expose the new helpers.
- Runtime smoke tests create a temporary directory with known files, then verify
  count, deterministic ordering, and Buffer-backed output through stdout.
- The stdlib example catalog includes a runnable directory-listing example.
