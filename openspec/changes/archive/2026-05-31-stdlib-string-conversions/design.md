## Context

The stdlib now exposes enough process, argument, file, directory, and I/O
surface to write small scripts. The missing bridge is scalar conversion:
programs can receive bytes, but cannot parse common numeric input without raw
FFI, and they can compute integers but cannot format them into a Buffer for
exact stdout/stderr/file writes.

## Goals / Non-Goals

**Goals:**

- Add a small `std::strconv` module for decimal `i64` conversion.
- Follow the existing stdlib conventions: safe wrappers, `_raw` variants where
  pointer/capacity handoff is useful, `Result<i64, i64>` for fallible helpers,
  and managed `Buffer` output for runtime-produced text.
- Support parsing from normal `&str` and from a managed `Buffer` plus an
  explicit byte length.
- Keep behavior deterministic across platforms.

**Non-Goals:**

- No owned-string return ABI.
- No floats, radix-specific parsing, bool parsing, arbitrary precision, or
  locale-specific formatting.
- No JSON/data-format model.
- No new syntax or dependencies.

## API Shape

`std::strconv` adds:

- `strconv_last_error_code() -> i64`
- `strconv_parse_i64(value: &str) -> Result<i64, i64>`
- `strconv_parse_i64_raw(data_ptr: i64, len: i64) -> Result<i64, i64>`
- `strconv_parse_i64_buffer(buffer: Buffer, len: i64) -> Result<i64, i64>`
- `strconv_format_i64(value: i64, buffer: Buffer) -> Result<i64, i64>`
- `strconv_format_i64_raw(value: i64, buffer_ptr: i64, capacity: i64)
  -> Result<i64, i64>`

The runtime support uses explicit pointer+length inputs for parsing so callers
can parse buffers that are not NUL-terminated. Formatting writes decimal ASCII
bytes into the caller-provided buffer and returns the byte count.

## Semantics

Parsing:

- Accepts optional leading and trailing ASCII whitespace.
- Accepts an optional `+` or `-` sign.
- Requires at least one digit.
- Rejects non-whitespace trailing characters.
- Detects `i64` overflow and reports an error-shaped result.
- Treats negative lengths, nonzero lengths with null pointers, and invalid
  buffer handles as errors.

Formatting:

- Emits base-10 ASCII with a leading `-` for negative values.
- Does not append a NUL terminator.
- Returns the number of bytes written.
- Fails when the output capacity is too small, negative, or paired with a null
  pointer for a non-empty result.

Error values are intentionally numeric and nonzero. The stable contract is
success versus failure; individual error-code names are deferred until Sengoo
has a broader stdlib error taxonomy.

## Risks / Trade-offs

- **Risk:** Another module name can feel like surface area sprawl.
  **Mitigation:** keep `std::string` unchanged and isolate Buffer-dependent
  conversion helpers in `std::strconv`.
- **Risk:** Buffer-backed formatting is less ergonomic than returning `&str`.
  **Mitigation:** this matches the current ownership model and works with
  `std::io` raw writes today.
- **Risk:** Conversion scope grows into JSON or locale-aware formatting.
  **Mitigation:** record those as explicit non-goals and require follow-up
  OpenSpec before expanding.

## Verification

- Compiler surface tests prove the module compiles and emits runtime calls.
- `sgc` import expansion tests prove `std::strconv` preloads `ffi`,
  `Option`, and `Result` dependencies.
- `sglsp` tests prove symbols/signatures follow the import and dependencies.
- Runtime smoke tests prove valid parse, invalid parse, overflow, formatting,
  and Buffer parsing behavior.
- `examples/stdlib/14_strconv.sg` demonstrates parsing and formatting in the
  standard example catalog.
