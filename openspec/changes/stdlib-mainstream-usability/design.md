## Context

Recent work moved the stdlib forward in small, verified slices: `std::file`, `std::env`, `std::time`, and `std::random` now exist and are wired into `sgc` and `sglsp`. The next leap toward mainstream usability should stop adding isolated helpers and instead establish an explicit bar:

- a source module users can import with `import std::<module>;`
- safe wrappers over runtime functions
- `Result`/`Option` shaped errors where applicable
- managed `Buffer` output for strings until Sengoo has an owned-string return ABI
- compiler surface tests, `sgc` import tests, `sglsp` symbol tests, and runnable examples

## Goals / Non-Goals

**Goals:**

- Make path manipulation practical for file-oriented programs.
- Keep every new stdlib surface discoverable through `sglsp`.
- Make examples in `examples/stdlib` the canonical quickstart for each module.
- Preserve the current Buffer/Result convention for runtime-produced strings.
- Keep changes incremental and independently revertible.

**Non-Goals:**

- No new syntax.
- No owned-string ABI in this change.
- No fully general process management API unless it can be implemented safely and tested portably.
- No cryptographic randomness or security-sensitive primitives.
- No new dependency on a JSON/process/path crate from C or Rust for the C runtime path.

## Decisions

### Decision 1: `std::path` is Phase 1

`std::path` is the highest-leverage next module because file, env, time, and random already exist. Real utility programs need to join paths, inspect names/extensions, normalize separators, and branch on absolute paths. Implementing this first unlocks better file examples without requiring a new type system feature.

The API should prefer:

- scalar predicates returning `bool`
- scalar helpers returning `i64` for separator byte or length
- string-producing helpers copying into a managed `Buffer` and returning `Result<i64, i64>`
- `_raw` variants for explicit pointer/buffer handoff only where the existing stdlib style already does that

### Decision 2: Keep string outputs buffer-backed

The current runtime path cannot return owned Sengoo strings safely across all stdlib modules. Path helpers such as `path_join`, `path_parent`, `path_file_name`, `path_stem`, and `path_extension` should therefore copy into `Buffer` and report bytes written.

This mirrors `std::file`, `std::env`, DB/Lua/FFI diagnostics, and network receive/body output.

### Decision 3: Cross-platform behavior must be conservative

The C runtime path must work on Windows and Unix-like hosts:

- both `/` and `\` are recognized as separators
- `path_separator()` returns the platform-preferred separator byte
- `path_is_absolute` recognizes Windows drive roots, UNC-like roots, and Unix roots
- joining should avoid duplicate separators where possible
- an absolute right-hand side in `path_join` replaces the left-hand side
- normalization is lexical only; it must not touch the filesystem or resolve symlinks
- normalization trims redundant trailing separators except for roots and emits `.` for an empty relative result

### Decision 4: `std::process` metadata is Phase 2

Process and JSON-like helpers are important, but they are easier to misuse and harder to test portably. They should follow path once the module wiring and Buffer conventions are proven again.

The current compiler/runtime entry ABI does not expose stable `argc`/`argv` to Sengoo programs, and a command execution API would need careful shell-avoidance, argument-vector, environment, working-directory, timeout, and output-capture semantics. Phase 2 therefore excludes command execution and command-line argument access.

The Phase 2 API should focus on portable metadata and exit-code ergonomics:

- `process_id() -> i64`
- `process_current_dir_len() -> Result<i64, i64>`
- `process_current_dir_copy(buffer: Buffer) -> Result<i64, i64>`
- `process_exit_code(success: bool, failure_code: i64) -> i64`

This keeps the implementation useful for scripts that combine `std::file`, `std::path`, and `std::env`, while avoiding a partial process-management API that would be hard to make safe and portable.

### Decision 5: Phase 3 promotes supported collections and defers JSON

The repo has runtime-backed collection support for i64/bool vectors, maps, and iterators, plus a reflection-specific protobuf event shape. It does not yet have a general JSON value model, owned-string return ABI, byte-slice abstraction, or runtime-backed string-key/string-value collections.

Phase 3 should therefore promote the existing supported collection surface into `examples/stdlib` and document the current constraints. General JSON parsing/formatting and `Vec<&str>` / `HashMap<&str, ...>` support remain deferred until the compiler/runtime can represent the outputs and ownership model directly.

## Risks / Trade-offs

- **Risk:** Buffer-backed APIs feel less ergonomic than owned strings.  
  **Mitigation:** keep wrapper names clear and centralize examples; revisit when owned strings land.
- **Risk:** Path normalization semantics differ by platform.  
  **Mitigation:** make normalization lexical and document it; avoid filesystem-resolving behavior.
- **Risk:** Scope grows into a full stdlib rewrite.  
  **Mitigation:** phase-gate implementation and require tests/examples per module.
- **Risk:** `sgc` and `sglsp` drift when new modules are added.  
  **Mitigation:** each module task includes both wiring points and tests.
- **Risk:** A `std::process` module without argv/command execution may look incomplete.  
  **Mitigation:** document the deferred ABI work explicitly and make the available metadata helpers reliable first.
- **Risk:** Promoting collections without string-key/value containers may overstate generality.  
  **Mitigation:** examples and docs must show supported i64/bool shapes and explicitly defer string collections.

## Migration Plan

1. Add `std::path` source module and runtime functions.
2. Wire `std::path` into `sgc` stdlib import expansion and `sglsp` stdlib indexing.
3. Add compiler surface tests, `sgc` import tests, `sglsp` symbol tests, and a runnable `examples/stdlib/08_path.sg`.
4. Update `tools/stdlib/README.md` and `examples/stdlib/README.md`.
5. Run the verification baseline.
6. Add `std::process` metadata helpers without command execution.
7. Re-evaluate command-line argument and command execution scope in a separate OpenSpec before implementation.
8. Promote the supported `std::collections` surface into the stdlib example catalog.
9. Revisit JSON/string-collection scope only after the required value/string/byte-slice ABI work is specified.

## Open Questions

- What compiler/runtime entry ABI should expose command-line arguments to Sengoo source code?
- What command execution API can avoid shell injection while still being ergonomic?
- Should JSON-like helpers wait for an owned-string/byte-slice ABI?
