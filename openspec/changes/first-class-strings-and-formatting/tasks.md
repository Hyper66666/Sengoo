## 1. Core string types

- [x] 1.1 Promote `String` to a move-only, auto-`Drop`, UTF-8 owning type
  (depends on `automatic-memory-management`).
  - Completed for the current owned handle surface: `tools/stdlib/string.sg`
    defines `impl Drop for String`, MIR drop glue calls `String_Drop_drop`, and
    owned `String` move/use-after-move tests cover the stable diagnostic.
    Verified by `examples/stdlib/20_owned_string.sg` and the automatic-memory
    management drop suites.
- [~] 1.2 Make `&str` a first-class borrowed view with lifetime tracking.
  - Partial: string literals type-check as `&str`, stdlib APIs accept borrowed
    `&str`, and owned `String.as_str() -> &str` now lowers through
    `sengoo_string_as_str_ptr`. The borrow checker treats `as_str()` as an
    immutable borrow of the owner, so moving the `String` while the view is live
    reports `cannot-move-borrowed`. Returning an `as_str()`-derived view through
    explicit `return view` or a function/method tail expression now fails with
    stable `borrow-escapes-scope`. Full lifetime escape analysis for arbitrary
    stored borrowed views and aggregate-contained references remains open.
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

- [x] 2.1 `+`/`+=` concatenation (`String + &str`).
  - Completed for the current owned/borrowed surface: `String + &str`
    type-checks, lowers through
    `sengoo_string_concat_str_status`, and returns a new owned `String`.
    `String += &str` now type-checks for mutable bindings and lowers to the
    in-place `sengoo_string_push_str_status` runtime helper. `&str + String`
    now builds a fresh owned `String` from the borrowed prefix and appends the
    right-hand owned text. Negative runtime allocation/status results now route
    through the existing result-unwrap panic helper instead of being wrapped as
    invalid handles. Covered by compiler IR and native runtime tests.
- [x] 2.2 `PartialEq`/`Eq`/`PartialOrd`/`Ord` for `String`/`&str`.
  - Completed for the current byte-order surface: `&str` lowers `==`/`!=`
    through `sengoo_str_eq` and satisfies the comparison trait bounds through
    compiler-known impls. Owned `String` exposes method-level `eq`/`ne` and
    byte-order `lt`/`le`/`gt`/`ge`/`compare` helpers backed by
    `sengoo_string_compare`; owned `String` comparison operators
    `==`/`!=`/`<`/`<=`/`>`/`>=` lower through `sengoo_string_eq` /
    `sengoo_string_compare`; and `String` now registers
    `PartialEq`/`Eq`/`PartialOrd`/`Ord` marker impls in `std::string`.
    Verified by
    `cargo test -p sengoo-compiler string_and_str_satisfy_comparison_trait_bounds -- --nocapture`
    plus
    `cargo test -p sgc stdlib_str_comparison_operators_order_borrowed_strings -- --nocapture`
    and the existing native runtime owned-string comparison tests.
- [x] 2.3 Methods: `len`, `is_empty`, `contains`, `starts_with`, `ends_with`,
  `split`, `trim`, `to_ascii_upper`, `to_ascii_lower`.
  - Completed for the current stdlib surface: `&str` exposes the borrowed
    query helpers plus owned-result trim/ASCII-case transforms, owned `String`
    exposes `len`/`is_empty`, and `String.split(delimiter)` returns a copied
    snapshot iterator of owned `String` segments. Empty delimiters conservatively
    produce an empty iterator until char-splitting semantics are specified.
- [~] 2.4 `chars()` / `bytes()` iterators via the `Iterator` trait.
  - Partial: owned `String.bytes()` and `String.chars()` now create copied
    snapshot iterators with inherent `next() -> Option<i64>` and explicit
    `free()`. `bytes()` yields byte values and `chars()` yields Unicode scalar
    codepoints; both also satisfy the current generic `Iterator<Item = i64>`
    bound surface. `String.split()` uses the same inherent iterator pattern for
    owned segments. Source-level `Iterator<Item = char>` and
    `StringSplitIter<Item = String>` integration remain open.
- [x] 2.5 Byte-boundary-checked slicing: infallible `s[a..b]` plus fallible
  `s.get(a..b)`.
  - Completed for the current exclusive range surface: `str_get(value, start,
    end)` and `String.get(start, end)` copy a byte range into an owned `String`
    only when both offsets are UTF-8 scalar boundaries. Invalid order,
    out-of-range offsets, and non-boundary offsets return
    `STATUS_INVALID_ARGUMENT`. Infallible `s[a..b]` syntax now lowers for
    `&str` and owned `String`, returns an owned `String`, and panics through the
    existing result-unwrap panic helper on invalid ranges.

## 3. Formatting

- [x] 3.1 Add `Formatter`, `Display`, and `Debug` (coordinate with
  `generics-and-trait-system` core traits).
  - Completed through the shared core-trait surface: `Formatter`, `Display`,
    and `Debug` resolve as compiler-known names, `Display::to_string` is the
    contract used by `print`/`println`/`eprintln` and `{}` formatting, and
    `Debug` is available to builtin derive plus `{:?}` formatting.
- [~] 3.2 Implement `format(fmt, args...)` parsing `{}`, `{:?}`, positional,
  width, precision, and `{{`/`}}`.
  - Partial: `{}`, scalar `{:?}`, positional `{0}` / `{0:?}`, right-aligned
    width `{:>N}`, f64 fixed precision `{:.N}` / `{:>W.N}`, and `{{`/`}}`
    parse and lower through the owned-`String` builder. Struct `{:?}` renders
    fields in declaration order unless a user `Debug.to_string()` is present,
    in which case that custom Debug body is called. Enum `{:?}` likewise calls
    a user `Debug.to_string()` when present, otherwise derived enum Debug
    renders unit and tuple-payload variants. The general `Formatter` object
    protocol remains open.
- [x] 3.3 Compile-time validation of format literals (arity + spec) with a stable
  diagnostic.
  - Completed for the current format mini-language: invalid templates and
    non-literal templates report `invalid-format-template`; arity mismatches
    report `format-argument-count`. Covered by `compiler/src/tests/format_tests.rs`.
- [x] 3.4 `print`/`println`/`eprintln` accepting any `Display`; keep `print(<i64>)`
  source-compatible.
  - Completed for the current Display surface: builtin `print`/`println` and
    `eprintln` type-check user `Display` impls, lower through `to_string() ->
    String`, and keep primitive printing source-compatible. Compiler IR and
    native stdout/stderr tests cover the routing; `println` still shares the
    existing print runtime path.
- [x] 3.5 `#[derive(Debug)]` integration for structs/enums.
  - Completed for the current formatter surface: builtin derive registers the
    `Debug` impl, struct `{:?}` renders fields in declaration order, and enum
    `{:?}` now renders unit variants plus tuple payload variants through
    discriminant-based MIR lowering. Covered by `format_tests` unit/tuple enum
    regressions and `examples/stdlib/25_formatting.sg`.

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

- [x] 5.1 Boundary checks on slice/index; stable status on non-boundary offsets.
  - Completed for the current exclusive range surface: fallible `str_get` /
    `String.get` return `STATUS_INVALID_ARGUMENT` on non-boundary offsets, and
    infallible `s[a..b]` / `String[a..b]` reuse the same runtime checks before
    either producing an owned `String` or panicking.
- [x] 5.2 `chars()` decodes UTF-8; reject invalid sequences at construction.
  - Completed for the current scalar-codepoint surface: `String.chars()`
    decodes validated UTF-8 into scalar codepoints, construction paths reject
    invalid UTF-8, and the iterator participates in generic bounds as
    `Iterator<Item = i64>` codepoints. Returning source-level `char` through a
    generic iterator remains tracked by 2.4.
- [x] 5.3 Document ASCII-only case ops and the Unicode follow-up in
  `docs/language-features.md`.
  - Documented in the new "Text and Strings" section.

## 6. Conformance and docs

- [x] 6.1 Add `examples/stdlib/` programs printing a `String`, a struct via
  `Debug`, and an interpolated `f"..."`.
  - Completed by `examples/stdlib/25_formatting.sg`, covering owned `String`
    formatting, positional placeholders, scalar `{:?}`, right-aligned width,
    f64 fixed precision, structural struct `{:?}`, custom struct `Debug`,
    derived enum payload `{:?}`, and an interpolated f-string. Custom enum
    Debug is covered by compiler regression tests; the example remains focused
    on the common derive path.
- [x] 6.2 Update `examples/realworld/SUPPORT_MATRIX.md` string/formatting rows.
  - Completed for the current supported subset: the owned-string row now
    mentions trim/ASCII case transforms, comparison trait bounds, and links to
    the new stdlib/native tests; the formatting/interpolation row covers
    for `{}`, scalar `{:?}`, positional placeholders, right-aligned width,
    f64 fixed precision, struct `{:?}`, struct custom Debug, derived enum
    Debug, custom enum Debug, Display-backed types, and f-string expansion.
- [x] 6.3 Run `openspec validate first-class-strings-and-formatting --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (lexer/string/format lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New string/format examples compile, link, and run; `print(42)` still works
