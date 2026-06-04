## Why

Sengoo can move bytes through `&str`, raw runtime helpers, and managed
`Buffer` handles, but everyday text code still lacked a first-class owned text
value. This kept stdlib APIs tied to output buffers and made mainstream string
workflows feel lower-level than Python, Go, Rust, or Java.

## Proposal (delivered in v1)

- Add owned `String` as `struct String { handle: i64 }` with runtime slot-table
  storage, explicit `drop`, move/clone/equality, and UTF-8 byte length.
- Provide `string_new`, `string_from_str`, and `string_from_buffer` plus
  `impl String` for append, clear, clone, `copy_to_buffer`, and `eq`.
- Keep `&str` literals and `Buffer` capacity ABI unchanged; conversions are
  explicit and return `Result<..., i64>` on allocation/validation failure.
- Enforce move semantics for the **canonical** stdlib `String` type in typeck.
- Document that owned text is read via `len()` / `copy_to_buffer`, not `as_str()`.
- Ship example `examples/stdlib/20_owned_string.sg`, LSP stdlib deps (`string`
  → `ffi`), and compiler integration tests.

## Impact

- New `tools/stdlib/runtime_string.c` and `tools/stdlib/string.sg`.
- Typeck borrow pass for canonical `String`; tests in `owned_string_tests.rs`.
- Stdlib README and LSP symbol graph updated.

## Non-Goals (unchanged / deferred)

- No implicit conversion from `String` to raw pointers.
- No `as_str()` or scope-end RAII in v1.
- No Unicode grapheme, normalization, locale, or regex semantics.
- No change to `Buffer.len()` capacity semantics.
- No full `trim`/`split`/`join`/`replace` owned-string surface in v1.
