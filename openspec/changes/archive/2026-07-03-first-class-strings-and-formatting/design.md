## Context

Owned `String` already exists as a runtime handle (`runtime_string.c`,
`tools/stdlib/string.sg`) with manual `.drop()`. `&str` helpers exist
(`str_len`, equality, search). JSON already reads numbers as `f64`. This change
promotes text to first-class status on top of the memory model and trait system.

## Goals / Non-goals

- Goal: idiomatic text code — concatenation, formatting, interpolation, printing
  any `Display`, UTF-8-safe slicing — with no manual frees.
- Non-goal: full Unicode (normalization/collation/case folding) and a maximal
  format language.

## Decisions

### Decision 1 — Types

- `String`: owning, growable, UTF-8, move-only, auto-`Drop` (depends on
  `automatic-memory-management`). Backed by the existing runtime string buffer.
- `&str`: borrowed `{ ptr, len }` view with a lifetime tied to its owner; never
  owns, never drops.
- `char`: a Unicode scalar value (u32 range, excluding surrogates).

### Decision 2 — Operators and methods

`+`/`+=` concatenate (`String + &str -> String`). `==`/`<` use byte-wise
compare via `PartialEq`/`Ord`. Methods: `len`, `is_empty`, `contains`,
`starts_with`, `ends_with`, `split(&str) -> iterator`, `trim`, `to_ascii_upper`,
`to_ascii_lower`, `chars() -> impl Iterator<Item = char>`, `bytes()`. Slicing
`s[a..b]` returns `&str` and validates that `a` and `b` are char boundaries.

### Decision 3 — Formatting

`Display::fmt(&self, f: &mut Formatter) -> Result` writes into the formatter.
`format(fmt_literal, args...)` parses a fixed mini-language:

- `{}` → `Display`, `{:?}` → `Debug`
- positional `{0}` and width/precision `{:>8}`, `{:.3}` for the common cases
- `{{` / `}}` escape braces

`print`/`println`/`eprintln` accept any single `Display` (and a `format`-style
variadic form). This replaces the integer-only `print`.

### Decision 4 — Interpolation lowering

`f"a {x} b {y:?}"` lexes as an interpolation token sequence and lowers to
`format("a {} b {:?}", x, y)` during parsing/HIR, so it reuses the formatting
machinery with no separate runtime.

### Decision 5 — Literal grammar

Add lexer rules: `b"..."` (byte string → `&[u8]`/Buffer), `"""..."""`
(multiline, common-leading-whitespace stripped), integer bases `0o[0-7]+` and
`0b[01]+`, and typed integer suffixes (`i8/i16/i32/i64/u8/...`, grammar shared
with `numeric-type-system`). `f"..."` is a distinct token kind. Raw `r"..."`
stays as-is.

### Decision 6 — UTF-8 correctness

Slicing/indexing on non-char-boundary byte offsets returns a stable
`STATUS_PARSE`-class error (or panics in the infallible operator form, with a
fallible `get(a..b)` variant). `chars()` decodes UTF-8; invalid sequences are
rejected at construction (a `String` is always valid UTF-8).

## Risks / Trade-offs

- **`print` signature change.** Old `print(<i64>)` must keep working: integers
  implement `Display`, so `print(42)` still compiles. Verified by the existing
  examples in conformance.
- **Format parsing errors.** A malformed format literal is a compile error (the
  literal is known at compile time), not a runtime failure.
- **ASCII-only case ops.** Documented; Unicode case folding is a follow-up.

## Migration

Additive. Existing `string.sg` handle helpers remain. New code uses operators,
`format`, and `println`. Owned `String` no longer needs `.drop()` once
`automatic-memory-management` lands; the explicit `.drop()` stays valid.
