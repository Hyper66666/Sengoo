## 1. Core string types

- [x] 1.1 Promote `String` to a move-only, auto-`Drop`, UTF-8 owning type
  (depends on `automatic-memory-management`).
  - Completed for the current owned handle surface: `tools/stdlib/string.sg`
    defines `impl Drop for String`, MIR drop glue calls `String_Drop_drop`, and
    owned `String` move/use-after-move tests cover the stable diagnostic.
    Verified by `examples/stdlib/20_owned_string.sg` and the automatic-memory
    management drop suites.
- [ ] 1.2 Make `&str` a first-class borrowed view with lifetime tracking.
- [ ] 1.3 Add `char` (Unicode scalar) type.
- [x] 1.4 Guarantee `String` construction validates UTF-8.
  - Completed for stdlib construction paths backed by
    `sengoo_owned_string_from_bytes`: `string_from_str`,
    `string_from_buffer`, `string_clone_status`, `push_str`, and the new
    trim/ASCII case helpers all reject invalid UTF-8 or preserve valid input.

## 2. Ergonomic methods and operators

- [ ] 2.1 `+`/`+=` concatenation (`String + &str`).
- [ ] 2.2 `PartialEq`/`Eq`/`PartialOrd`/`Ord` for `String`/`&str`.
- [ ] 2.3 Methods: `len`, `is_empty`, `contains`, `starts_with`, `ends_with`,
  `split`, `trim`, `to_ascii_upper`, `to_ascii_lower`.
  - Partial: `len`, `is_empty`, `contains`, `starts_with`, `ends_with`, and
    `index_of` already existed; this slice adds `str_trim`,
    `str_to_ascii_upper`, and `str_to_ascii_lower` returning owned `String`
    values. `split` remains open.
- [ ] 2.4 `chars()` / `bytes()` iterators via the `Iterator` trait.
- [ ] 2.5 Byte-boundary-checked slicing: infallible `s[a..b]` plus fallible
  `s.get(a..b)`.

## 3. Formatting

- [ ] 3.1 Add `Formatter`, `Display`, and `Debug` (coordinate with
  `generics-and-trait-system` core traits).
- [~] 3.2 Implement `format(fmt, args...)` parsing `{}`, `{:?}`, positional,
  width, precision, and `{{`/`}}`.
  - Partial: `{}`, scalar `{:?}`, positional `{0}` / `{0:?}`, and `{{`/`}}`
    parse and lower through the owned-`String` builder. Width, precision, and
    structure-aware Debug output remain open.
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
- [x] 5.3 Document ASCII-only case ops and the Unicode follow-up in
  `docs/language-features.md`.
  - Documented in the new "Text and Strings" section.

## 6. Conformance and docs

- [ ] 6.1 Add `examples/stdlib/` programs printing a `String`, a struct via
  `Debug`, and an interpolated `f"..."`.
- [ ] 6.2 Update `examples/realworld/SUPPORT_MATRIX.md` string/formatting rows.
  - Partial: the owned-string row now mentions trim/ASCII case transforms and
    links to the new stdlib/native tests. Formatting/interpolation rows remain
    open until Display/Debug/format land.
- [x] 6.3 Run `openspec validate first-class-strings-and-formatting --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (lexer/string/format lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New string/format examples compile, link, and run; `print(42)` still works
