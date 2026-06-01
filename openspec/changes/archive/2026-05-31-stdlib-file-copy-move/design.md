## Context

The standard library now supports directory setup/listing and basic file
read/write/remove operations. Copy and move are the next missing filesystem
building blocks for project tooling, backup-style scripts, generators, and
temporary-output workflows.

## Goals / Non-Goals

**Goals:**

- Add portable file copy and move operations to `std::file`.
- Require callers to make overwrite behavior explicit.
- Preserve the existing `Result` and `_raw` wrapper conventions.
- Keep implementation dependency-free and shell-free.

**Non-Goals:**

- No recursive directory copy/move.
- No cross-filesystem move fallback.
- No metadata/permission/timestamp preservation guarantee.
- No atomic copy guarantee.
- No progress callbacks, cancellation, or async I/O.

## API Shape

`std::file` gains:

- `file_copy(source: &str, destination: &str, overwrite: bool)
  -> Result<i64, i64>`
- `file_copy_raw(source_ptr: i64, destination_ptr: i64, overwrite: bool)
  -> Result<i64, i64>`
- `file_move(source: &str, destination: &str, overwrite: bool)
  -> Result<bool, i64>`
- `file_move_raw(source_ptr: i64, destination_ptr: i64, overwrite: bool)
  -> Result<bool, i64>`

Copy streams bytes from the source to the destination and returns the byte
count. Move delegates to the host rename primitive and returns success only
when that primitive succeeds.

## Semantics

- Source and destination paths must be non-empty.
- Copy and move target regular files, not directory trees.
- When `overwrite == false`, an existing destination is rejected.
- When `overwrite == true`, copy truncates/replaces the destination file and
  move requests host replacement semantics.
- Copy rejects destinations that already refer to the source file, even when
  overwrite is enabled, so aliases cannot truncate their own input.
- Copy removes a newly-created partial destination when a read/write/close
  error occurs.
- Move does not fall back to copy-plus-remove when the host rename fails, so
  cross-filesystem moves return an error-shaped result.
- The source remains intact after copy and disappears after a successful move.

The no-overwrite move path performs a best-effort destination existence check
before the host rename. This is appropriate for small scripting helpers, but it
is not claimed as a race-free filesystem transaction.

## Risks / Trade-offs

- **Risk:** Overwrite operations can destroy destination contents.
  **Mitigation:** overwrite is an explicit required boolean argument.
- **Risk:** Cross-filesystem move behavior differs from high-level libraries
  that silently copy then delete.
  **Mitigation:** document the bounded host-rename contract and avoid hidden
  non-atomic fallback behavior.
- **Risk:** Copy does not preserve metadata.
  **Mitigation:** state that this slice copies file bytes only; metadata APIs
  need a follow-up design.

## Verification

- Compiler surface tests cover the copy/move wrappers and runtime symbols.
- `sgc` import expansion tests expose the helpers through `std::file`.
- `sglsp` symbol/signature tests expose the new functions.
- Runtime smoke tests verify byte copying, no-overwrite rejection, same-file
  copy rejection, explicit overwrite, successful move, source retention after
  copy, and source removal after move.
- The stdlib example catalog includes a runnable file copy/move example.
