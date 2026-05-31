## Why

Sengoo can now read user arguments, stdin, files, and environment data through
standard-library helpers, but everyday programs still cannot reliably turn text
input into numbers or format numeric values back into caller-owned buffers. That
gap makes simple CLI tools awkward even after `std::args`, `std::io`, and
`std::file` landed.

This change adds a narrow conversion module before broader data-format work:
decimal `i64` parsing and formatting with the existing `Result` and managed
`Buffer` conventions.

## What Changes

- Add `std::strconv` as a source-level stdlib module.
- Provide `strconv_parse_i64(value: &str) -> Result<i64, i64>`.
- Provide `strconv_parse_i64_raw(data_ptr: i64, len: i64) -> Result<i64, i64>`
  and `strconv_parse_i64_buffer(buffer: Buffer, len: i64) -> Result<i64, i64>`
  so programs can parse bytes read into managed buffers.
- Provide `strconv_format_i64(value: i64, buffer: Buffer) -> Result<i64, i64>`
  plus a raw pointer/capacity variant for explicit interop.
- Wire the module through `sgc`, `sglsp`, docs, and runnable examples.
- Keep owned-string returns, floats, radix-specific parsing, locale behavior,
  JSON/data formats, and general byte-slice types out of scope.

## Impact

- Affected code:
  - `tools/stdlib/strconv.sg`
  - `tools/stdlib/runtime.c`
  - `tools/sgc/src/stdlib_imports.rs`
  - `tools/sgc/src/tests.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `compiler/src/tests/stdlib_surface_tests.rs`
  - `examples/stdlib/*`
  - `tools/stdlib/README.md`
- No source syntax changes.
- No third-party dependencies.
- Existing `std::string` behavior remains backward-compatible.
