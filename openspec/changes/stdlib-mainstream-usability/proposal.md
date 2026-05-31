## Why

Sengoo's toolchain has crossed the "toy" threshold: `sgc`, `sgpm`, `sgfmt`, and `sglsp` all have meaningful test coverage, and the standard library now has core data structures plus string, file, env, time, random, and reflection wrappers. The remaining gap is everyday language ergonomics. A user can compile and run programs, but writing ordinary utility programs still requires raw pointer-ish Buffer flows, manual path string handling, and ad hoc process/data-format glue.

This change defines the next standard-library usability program so future edits are spec-first instead of a pile of disconnected helpers.

## What Changes

- Add a `stdlib-mainstream-usability` capability that defines the minimum bar for "mainstream-language usable" standard-library modules.
- Phase 1 focuses on `std::path`: path separator discovery, absolute-path checks, file-name/stem/extension/parent extraction, joining, and normalization into managed `Buffer` outputs.
- Phase 2 focuses on `std::process`: portable process metadata and conventional exit-code helpers. Command execution and command-line argument access are explicitly deferred until the compiler/runtime entry ABI can support them safely.
- Phase 3 focuses on data-format and collection ergonomics: JSON-like helpers are deferred until Sengoo has a better value/string/byte-slice model, while currently supported `std::collections` shapes become first-class stdlib examples.
- Every new stdlib source module must be wired through `sgc` source import expansion, `sglsp` stdlib symbol/signature indexing, stdlib docs, and runnable examples.
- No source-language syntax change is required by this proposal.

## Capabilities

### New Capabilities

- `stdlib-mainstream-usability`: Defines the behavior, wiring, examples, and verification bar for standard-library modules that close day-to-day usability gaps.

## Impact

- Affected code:
  - `tools/stdlib/*.sg`
  - `tools/stdlib/runtime.c`
  - `tools/sgc/src/stdlib_imports.rs`
  - `tools/sgc/src/tests.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `compiler/src/tests/stdlib_surface_tests.rs`
  - `examples/stdlib/*`
  - `tools/stdlib/README.md`
- Public syntax and existing stdlib behavior must remain backward-compatible.
- No new third-party dependencies unless a later OpenSpec update explicitly justifies one.
- Verification per phase: focused red/green tests first, then `cargo fmt --check`, `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sglsp`, and `git diff --check`.
