## Context

The current stdlib has enough pieces to write small command-line utilities:
`std::file`, `std::path`, `std::process`, and `std::args`. Directory creation is
the next low-risk filesystem gap because many programs need to prepare an
output folder before writing files.

## Goals / Non-Goals

**Goals:**

- Add a small, portable directory module.
- Keep the API shell-free and dependency-free.
- Preserve the existing `Result<bool, i64>` and `_raw` wrapper conventions.
- Make the module discoverable through `sgc`, `sglsp`, docs, and examples.

**Non-Goals:**

- No recursive deletion.
- No directory listing, current-dir mutation, or metadata structs.
- No attempt to distinguish every host error code at the source level.

## Decisions

### Decision 1: Use a separate `std::dir` module

Directory operations are related to `std::file`, but keeping them in `std::dir`
avoids turning `file.sg` into a mixed filesystem namespace and keeps examples
small. The module follows the same import/dependency pattern as `std::file`:
`Result` for fallible operations and `ffi` for `&str` pointer bridging.

### Decision 2: Support create/remove-empty, not tree deletion

Recursive deletion is high blast-radius and easy to misuse. This change only
allows removing an empty directory via `dir_remove`. Programs can create nested
directories with `dir_create_all`, but they cannot delete a populated tree in
one call.

### Decision 3: Treat existing directories as successful creation

Both `dir_create` and `dir_create_all` return success when the target directory
already exists. This makes idempotent setup code easy and mirrors common
mainstream filesystem APIs.

### Decision 4: Keep errors coarse for now

The C runtime returns `0` for success and a negative value for failure. The
Sengoo wrapper maps failures to `Result<bool, i64>` with a coarse error value,
matching the current stdlib style. Rich errno exposure can be specified later if
the language adds a broader filesystem error model.

## Risks / Trade-offs

- **Risk:** `std::dir` and `std::file` feel split compared with languages that
  have one `fs` namespace.  
  **Mitigation:** document both modules together under the stdlib filesystem
  section and keep function names explicit.
- **Risk:** Users may expect `dir_remove` to remove non-empty directories.  
  **Mitigation:** name the contract clearly in docs and examples; omit
  recursive deletion from this change.
- **Risk:** Recursive create path parsing differs by platform.  
  **Mitigation:** use simple lexical splitting over `/` and `\`, preserving
  Windows drive/UNC prefixes where possible, and cover common cases with tests.

## Migration Plan

1. Add compiler, `sgc`, and `sglsp` tests for `std::dir` import visibility.
2. Add runtime smoke coverage via a runnable stdlib example.
3. Implement `tools/stdlib/dir.sg`.
4. Add C runtime directory helpers.
5. Wire `dir` into `sgc` and `sglsp`.
6. Update stdlib docs and examples.
7. Run focused tests and the standard verification gate.
