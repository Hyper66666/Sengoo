## 1. Core string types

- [ ] 1.1 Promote `String` to a move-only, auto-`Drop`, UTF-8 owning type
  (depends on `automatic-memory-management`).
- [ ] 1.2 Make `&str` a first-class borrowed view with lifetime tracking.
- [ ] 1.3 Add `char` (Unicode scalar) type.
- [ ] 1.4 Guarantee `String` construction validates UTF-8.

## 2. Ergonomic methods and operators

- [ ] 2.1 `+`/`+=` concatenation (`String + &str`).
  - Partial: `String + &str` now type-checks, consumes the left owned `String`,
    lowers through `sengoo_string_concat_str`, links in the native runtime, and
    is covered by compiler and sgc native-run tests. `+=`, `String + String`,
    and fallible allocation/status surfacing remain open.
- [ ] 2.2 `PartialEq`/`Eq`/`PartialOrd`/`Ord` for `String`/`&str`.
  - Partial: owned `String == String` and `String != String` now borrow both
    operands, lower through `sengoo_string_eq`, remain usable after comparison,
    and run through the native runtime. Trait-backed `PartialEq`/`Eq`,
    cross `String`/`&str` comparisons, and ordering remain open.
- [ ] 2.3 Methods: `len`, `is_empty`, `contains`, `starts_with`, `ends_with`,
  `split`, `trim`, `to_ascii_upper`, `to_ascii_lower`.
- [ ] 2.4 `chars()` / `bytes()` iterators via the `Iterator` trait.
- [ ] 2.5 Byte-boundary-checked slicing: infallible `s[a..b]` plus fallible
  `s.get(a..b)`.

## 3. Formatting

- [ ] 3.1 Add `Formatter`, `Display`, and `Debug` (coordinate with
  `generics-and-trait-system` core traits).
- [ ] 3.2 Implement `format(fmt, args...)` parsing `{}`, `{:?}`, positional,
  width, precision, and `{{`/`}}`.
- [ ] 3.3 Compile-time validation of format literals (arity + spec) with a stable
  diagnostic.
- [ ] 3.4 `print`/`println`/`eprintln` accepting any `Display`; keep `print(<i64>)`
  source-compatible.
  - Partial: `println` now type-checks and lowers through the existing `print`
    runtime path. `eprintln` supports the current printable primitive/struct
    surface and writes through native stderr runtime symbols. Trait-backed
    `Display` remains open.
  - Partial: owned `String` arguments now print as UTF-8 text through
    `sengoo_string_as_str_ptr` and are borrowed rather than consumed, so later
    `String` uses remain valid. General `Display` dispatch remains open.
- [ ] 3.5 `#[derive(Debug)]` integration for structs/enums.

## 4. Literals and interpolation

- [ ] 4.1 Lexer: `f"..."` interpolation token sequence.
- [x] 4.2 Lexer: `b"..."` byte strings and `"""..."""` multiline (leading-WS strip).
  - Byte string tokenization is covered in lexer tests; multiline strings now
    scan as one `String` token and strip common leading whitespace.
- [x] 4.3 Lexer: `0o`/`0b` integer bases and typed integer suffixes (shared
  grammar with `numeric-type-system`).
  - Implemented in `compiler/src/lexer/token.rs`; covered by lexer token tests
    and a compile-to-IR print regression for based/suffixed/separated literals.
- [ ] 4.4 Lower `f"...{e}..."` to `format(...)` in the parser/HIR.
- [ ] 4.5 Tests for each literal form and interpolation lowering.

## 5. UTF-8 correctness

- [ ] 5.1 Boundary checks on slice/index; stable status on non-boundary offsets.
- [ ] 5.2 `chars()` decodes UTF-8; reject invalid sequences at construction.
- [ ] 5.3 Document ASCII-only case ops and the Unicode follow-up in
  `docs/language-features.md`.

## 6. Conformance and docs

- [ ] 6.1 Add `examples/stdlib/` programs printing a `String`, a struct via
  `Debug`, and an interpolated `f"..."`.
- [ ] 6.2 Update `examples/realworld/SUPPORT_MATRIX.md` string/formatting rows.
- [x] 6.3 Run `openspec validate first-class-strings-and-formatting --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (lexer/string/format lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- `cargo test -p sengoo-compiler owned_string_printing_borrows_and_lowers_as_text -- --nocapture`
- `cargo test -p sgc owned_string_prints_text_and_remains_usable_with_native_runtime -- --nocapture`
- New string/format examples compile, link, and run; `print(42)` still works
