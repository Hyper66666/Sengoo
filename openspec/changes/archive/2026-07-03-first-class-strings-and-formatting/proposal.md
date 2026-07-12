## Why

Strings are not first-class, which makes everyday code verbose and un-idiomatic:

- `print(...)` only prints integers; all examples either `print(<i64>)` or return
  an exit code.
- Owned `String` requires manual `.drop()` and result-unwrapping ceremony
  (`examples/stdlib/20_owned_string.sg`); there is no `+` concatenation, no
  indexing/slicing ergonomics, and no formatting.
- The lexer (`compiler/src/lexer/token.rs`) only recognizes `"..."` and `r"..."`.
  The spec's `f"{x}"` interpolation, byte strings `b"..."`, multiline `"""..."""`,
  and numeric suffixes / `0o` / `0b` are unimplemented.
- Unicode handling is byte-order only (`std::collections` documents "no
  normalization, locale collation, or case folding").

No mainstream language ships without ergonomic strings, formatting, and
printing. This change depends on `automatic-memory-management` (owned `String`
becomes auto-drop) and `generics-and-trait-system` (`Display`/`Debug`).

## Proposal

Make text first-class and ergonomic.

- **`String` (owned, growable, UTF-8) and `&str` (borrowed view)** as core types.
  `String` is move-only and auto-dropped (no manual `.drop()`). `&str` supports
  length, comparison, search, slicing by byte index with UTF-8 boundary checks,
  and char iteration.
- **Operators/ergonomics**: `+` / `+=` concatenation for `String`, equality and
  ordering, `len()`, `is_empty()`, `contains`/`starts_with`/`ends_with`, `split`,
  `trim`, `to_upper`/`to_lower` (ASCII now; Unicode-aware behind a documented
  follow-up), and `chars()` / `bytes()` iterators (using `Iterator`).
- **Formatting**: a `Display`/`Debug`-based `format(...)` with a `{}` /
  `{:?}` / width / precision mini-language, plus `print`/`println`/`eprintln`
  that accept any `Display` value (not just `i64`).
- **String interpolation**: `f"...{expr}..."` lexed and lowered to `format`
  calls.
- **Literal forms**: byte strings `b"..."`, multiline `"""..."""`, and integer
  literal bases `0o`/`0b` and typed suffixes (e.g. `42i64`) — coordinated with
  `numeric-type-system` for the numeric suffix grammar.
- **UTF-8 correctness**: indexing/slicing validates char boundaries; invalid
  boundaries are a stable runtime status, not a silent split.

## What changes

- ADDED: first-class `String`/`&str` with ergonomic methods and operators.
- ADDED: `Display`/`Debug` formatting, `format`, and variadic-friendly
  `print`/`println`/`eprintln`.
- ADDED: `f"..."` interpolation, `b"..."`, `"""..."""` literals; `0o`/`0b` and
  typed integer suffixes in the lexer.
- ADDED: UTF-8 boundary-checked slicing/iteration.

## Non-goals

- Full Unicode normalization (NFC/NFD), locale collation, and Unicode-aware case
  folding (a documented follow-up; ASCII case ops ship now).
- Regex/format-string Turing completeness; the format mini-language is a fixed,
  documented subset.
