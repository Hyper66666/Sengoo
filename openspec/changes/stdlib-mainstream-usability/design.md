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
- normalization is lexical only; it must not touch the filesystem or resolve symlinks

### Decision 4: Process/data-format work is gated after path

Process and JSON-like helpers are important, but they are easier to misuse and harder to test portably. They should follow path once the module wiring and Buffer conventions are proven again.

Process work must decide whether the runtime can expose command execution without shell-injection footguns. Data-format work must decide whether to implement a tiny JSON-shaped utility surface or wait for a richer string/byte-slice model.

## Risks / Trade-offs

- **Risk:** Buffer-backed APIs feel less ergonomic than owned strings.  
  **Mitigation:** keep wrapper names clear and centralize examples; revisit when owned strings land.
- **Risk:** Path normalization semantics differ by platform.  
  **Mitigation:** make normalization lexical and document it; avoid filesystem-resolving behavior.
- **Risk:** Scope grows into a full stdlib rewrite.  
  **Mitigation:** phase-gate implementation and require tests/examples per module.
- **Risk:** `sgc` and `sglsp` drift when new modules are added.  
  **Mitigation:** each module task includes both wiring points and tests.

## Migration Plan

1. Add `std::path` source module and runtime functions.
2. Wire `std::path` into `sgc` stdlib import expansion and `sglsp` stdlib indexing.
3. Add compiler surface tests, `sgc` import tests, `sglsp` symbol tests, and a runnable `examples/stdlib/08_path.sg`.
4. Update `tools/stdlib/README.md` and `examples/stdlib/README.md`.
5. Run the verification baseline.
6. Re-evaluate process/data-format scope from the evidence gathered in Phase 1.

## Open Questions

- Should `path_normalize` preserve trailing separators or always trim them except roots?
- Should `path_join` treat an absolute right-hand side as a replacement or as an error-like fallback?
- Should Phase 2 expose command execution at all, or stop at process metadata/exit-code helpers until a safer API exists?
- Should JSON-like helpers wait for an owned-string/byte-slice ABI?
