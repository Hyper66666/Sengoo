## 1. Core string types

- [x] 1.1 Promote `String` to a move-only, auto-`Drop`, UTF-8 owning type
  (depends on `automatic-memory-management`).
  - Completed for the current owned handle surface: `tools/stdlib/string.sg`
    defines `impl Drop for String`, MIR drop glue calls `String_Drop_drop`, and
    owned `String` move/use-after-move tests cover the stable diagnostic.
    Verified by `examples/stdlib/20_owned_string.sg` and the automatic-memory
    management drop suites.
- [ ] 1.2 Make `&str` a first-class borrowed view with lifetime tracking.
- [x] 1.3 Add `char` (Unicode scalar) type.
  - Completed for the current scalar surface: char literals type-check as
    `char`, lower to `i32` in MIR/LLVM/FFI signatures, and `String.push_char`
    appends a scalar through the UTF-8 runtime builder. `chars()` iteration and
    boundary-aware string slicing remain tracked below.
- [x] 1.4 Guarantee `String` construction validates UTF-8.
  - Completed for stdlib construction paths backed by
    `sengoo_owned_string_from_bytes`: `string_from_str`,
    `string_from_buffer`, `string_clone_status`, `push_str`, and the new
    trim/ASCII case helpers all reject invalid UTF-8 or preserve valid input.

## 2. Ergonomic methods and operators

- [ ] 2.1 `+`/`+=` concatenation (`String + &str`).
- [~] 2.2 `PartialEq`/`Eq`/`PartialOrd`/`Ord` for `String`/`&str`.
  - Partial: `&str` already lowers `==`/`!=` through `sengoo_str_eq`.
    Owned `String` now exposes method-level `eq`/`ne` and byte-order
    `lt`/`le`/`gt`/`ge`/`compare` helpers backed by
    `sengoo_string_compare`. Operator sugar (`String ==`, `String <`) and
    trait-backed `PartialEq`/`Ord` impls remain open.
- [ ] 2.3 Methods: `len`, `is_empty`, `contains`, `starts_with`, `ends_with`,
  `split`, `trim`, `to_ascii_upper`, `to_ascii_lower`.
  - Partial: `len`, `is_empty`, `contains`, `starts_with`, `ends_with`, and
    `index_of` already existed; this slice adds `str_trim`,
    `str_to_ascii_upper`, and `str_to_ascii_lower` returning owned `String`
    values, plus owned `String.push_char(char)`. `split` remains open.
- [ ] 2.4 `chars()` / `bytes()` iterators via the `Iterator` trait.
- [~] 2.5 Byte-boundary-checked slicing: infallible `s[a..b]` plus fallible
  `s.get(a..b)`.
  - Partial: `str_get(value, start, end)` and `String.get(start, end)` copy a
    byte range into an owned `String` only when both offsets are UTF-8 scalar
    boundaries. Invalid order, out-of-range offsets, and non-boundary offsets
    return `STATUS_INVALID_ARGUMENT`. Infallible `s[a..b]` syntax remains open.

## 3. Formatting

- [ ] 3.1 Add `Formatter`, `Display`, and `Debug` (coordinate with
  `generics-and-trait-system` core traits).
- [~] 3.2 Implement `format(fmt, args...)` parsing `{}`, `{:?}`, positional,
  width, precision, and `{{`/`}}`.
  - Partial: `{}`, scalar `{:?}`, positional `{0}` / `{0:?}`, right-aligned
    width `{:>N}`, f64 fixed precision `{:.N}` / `{:>W.N}`, and `{{`/`}}`
    parse and lower through the owned-`String` builder. Struct `{:?}` renders
    fields in declaration order. Enum Debug and derive-driven Debug
    customization remain open.
- [ ] 3.3 Compile-time validation of format literals (arity + spec) with a stable
  diagnostic.
- [ ] 3.4 `print`/`println`/`eprintln` accepting any `Display`; keep `print(<i64>)`
  source-compatible.
  - Partial: `println` now type-checks and lowers through the existing `print`
    runtime path. `eprintln` supports the current printable primitive/struct
    surface and writes through native stderr runtime symbols. Trait-backed
    `Display` remains open.
- [ ] 3.5 `#[derive(Debug)]` integration for structs/enums.

## 4. Literals and interpolation

- [~] 4.1 Lexer: `f"..."` interpolation token sequence.
  - Implemented as a parser pre-lex source expansion pass rather than a
    dedicated lexer token sequence: `f"..."` rewrites to `format(...)` while
    preserving ordinary string/comment contents.
- [x] 4.2 Lexer: `b"..."` byte strings and `"""..."""` multiline (leading-WS strip).
  - Byte string tokenization is covered in lexer tests; multiline strings now
    scan as one `String` token and strip common leading whitespace.
- [x] 4.3 Lexer: `0o`/`0b` integer bases and typed integer suffixes (shared
  grammar with `numeric-type-system`).
  - Implemented in `compiler/src/lexer/token.rs`; covered by lexer token tests
    and a compile-to-IR print regression for based/suffixed/separated literals.
- [x] 4.4 Lower `f"...{e}..."` to `format(...)` in the parser/HIR.
  - Verified by `compiler/src/parser/fstring_expander.rs` unit tests and
    `cargo test -p sengoo-compiler format -- --nocapture`.
- [~] 4.5 Tests for each literal form and interpolation lowering.
  - Covered: f-string simple/multiple/compound interpolation, brace escapes,
    empty interpolation rejection, byte strings, multiline strings, based
    integer literals, and typed suffixes. Remaining: broader source-map/span
    diagnostics for expanded f-strings.

## 5. UTF-8 correctness

- [~] 5.1 Boundary checks on slice/index; stable status on non-boundary offsets.
  - Partial: fallible `str_get` / `String.get` return
    `STATUS_INVALID_ARGUMENT` on non-boundary offsets. Infallible index/slice
    syntax remains open.
- [ ] 5.2 `chars()` decodes UTF-8; reject invalid sequences at construction.
- [x] 5.3 Document ASCII-only case ops and the Unicode follow-up in
  `docs/language-features.md`.
  - Documented in the new "Text and Strings" section.

## 6. Conformance and docs

- [~] 6.1 Add `examples/stdlib/` programs printing a `String`, a struct via
  `Debug`, and an interpolated `f"..."`.
  - Added `examples/stdlib/25_formatting.sg` covering owned `String`
    formatting, positional placeholders, scalar `{:?}`, right-aligned width,
    f64 fixed precision, struct `{:?}`, and an interpolated f-string. Enum
    `Debug` and derive/custom Debug rendering remain open.
- [~] 6.2 Update `examples/realworld/SUPPORT_MATRIX.md` string/formatting rows.
  - Partial: the owned-string row now mentions trim/ASCII case transforms and
    links to the new stdlib/native tests; added a formatting/interpolation row
    for `{}`, scalar `{:?}`, positional placeholders, right-aligned width,
    f64 fixed precision, struct `{:?}`, Display-backed types, and f-string
    expansion. Enum/custom Debug remains open.
- [x] 6.3 Run `openspec validate first-class-strings-and-formatting --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (lexer/string/format lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New string/format examples compile, link, and run; `print(42)` still works
