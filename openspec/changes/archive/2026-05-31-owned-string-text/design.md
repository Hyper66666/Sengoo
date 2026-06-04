## Scope

This change is the P0 owned-text lane only. It is independently archivable when
owned `String` semantics, conversions, tests, docs, and examples are complete.

## Model

- `String` is `struct String { handle: i64 }` backed by a runtime slot table with
  generation counters (invalid/double free → stable `INVALID_HANDLE`, no raw
  double-free).
- `&str` remains a borrowed view over string literal bytes only in this lane.
- `Buffer` remains a runtime-managed byte buffer with capacity-oriented APIs
  used by FFI and C bridge helpers.
- Moving a **canonical stdlib** `String` transfers ownership; cloning allocates
  and copies bytes. There is **no** `String::as_str()` — exposing a `&str` view
  over owned heap data would be unsound without lifetime/RAII support.
- Byte inspection uses `len()`, `is_empty()`, and `copy_to_buffer`. Literal
  `&str` helpers (`str_*`) are unchanged.
- `String.len()` returns UTF-8 **byte** count, not scalar values or graphemes.
- `drop()` is explicit; scope-end destructor elision is out of scope for v1.

## API Shape (as shipped)

Free constructors and `impl String` methods (stdlib module `string`, depends on
`ffi` for `Buffer`):

```text
struct String { handle: i64 }

string_new() -> Result<String, i64>
string_from_str(value: &str) -> Result<String, i64>
string_from_buffer(buffer: Buffer, used_len: i64) -> Result<String, i64>

impl String:
  len(self: &String) -> i64
  is_empty(self: &String) -> bool
  clone(self: &String) -> Result<String, i64>
  push_str(self: &mut String, value: &str) -> Result<bool, i64>
  clear(self: &mut String) -> bool
  copy_to_buffer(self: &String, buffer: Buffer) -> Result<i64, i64>
  eq(self: &String, other: &String) -> bool
  drop(self: String) -> bool
```

Runtime C ABI (`tools/stdlib/runtime_string.c`): `sengoo_string_new`,
`from_str_copy`, `from_buffer`, `len`, `is_empty`, `clone`, `push_str`, `clear`,
`copy_to_buffer`, `eq`, `free_status`. Capacity reserves `len + 1` for NUL
safety when bridging; public `borrow_cstr` was removed.

## Compiler ownership

Move/use-after-move diagnostics apply only when typeck resolves the binding to
the **canonical** stdlib `String { handle: i64 }` (`TypeEnv.owned_string_ty`).
User-defined `struct String { ... }` types are not subject to these rules.
Borrow check runs at end of each function/block before env pop; inner-block
moves propagate to outer scope on `pop_scope`.

## Compatibility

Existing `&str` literal APIs and managed `Buffer` examples remain valid. New code
uses `Result<..., i64>` for allocation and conversion failures.

## Non-goals (deferred)

- `as_str()` / borrowed views over owned storage
- Scope-end RAII `drop`
- trim / split / join / replace on owned `String` (proposal stretch goals)
- Unicode grapheme, normalization, locale, regex

## Done Definition

A user can construct, move, clone, append, compare, and copy owned strings via
`copy_to_buffer` without raw pointer choreography; `examples/stdlib/20_owned_string.sg`
and compiler/LSP tests pass; buffer-based programs keep prior behavior.
