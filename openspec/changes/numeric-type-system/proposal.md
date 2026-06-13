## Why

The numeric story is i64-centric and underspecified:

- `std::math` and `std::strconv` are i64-only (`abs_i64`, `min_i64`,
  `strconv_parse_i64` ...); floats, radix selection, and locale formatting are
  documented as deferred.
- `f64` exists in the lexer and JSON reads numbers as `f64`, but there are no
  float math helpers, no float parse/format, and no documented float semantics.
- Integer widths (`i8/i16/i32/u8/u16/u32/u64`), signedness, conversion rules,
  and **overflow semantics** are not defined or implemented as a coherent set.

Mainstream languages define their integer widths, conversions, and overflow
behavior precisely. This change makes Sengoo's numeric layer complete and
predictable. It depends on `generics-and-trait-system` for numeric traits and
coordinates literal suffix grammar with `first-class-strings-and-formatting`.

## Proposal

- **Integer types**: `i8/i16/i32/i64`, `u8/u16/u32/u64`, and pointer-sized
  `isize/usize`, with explicit, lossless-or-checked conversions (`as` casts with
  documented truncation, plus checked `try_into`-style conversions).
- **Overflow semantics**: defined per operation — debug builds trap on
  overflow, release builds wrap, and explicit `wrapping_*` / `checked_*` /
  `saturating_*` methods are available regardless of build mode.
- **Floats**: `f32`/`f64` with IEEE-754 semantics, a `std::math` float surface
  (`sqrt`, `pow`, `floor`, `ceil`, `round`, `abs`, trig, `min`/`max`, `NaN`/`inf`
  predicates), and float parse/format via the formatting layer.
- **Numeric traits**: `Add/Sub/Mul/Div/Rem/Neg` operator traits, `From`/`Into`
  for widening conversions, and `Ord`/`Eq` (with the documented `PartialOrd`-only
  caveat for floats).
- **Literals**: typed suffixes (`42i64`, `7u8`, `1.5f32`), `0x`/`0o`/`0b` bases,
  and digit separators `_`.

## What changes

- ADDED: full integer width/signedness set with defined conversions.
- ADDED: defined overflow semantics + `wrapping_/checked_/saturating_` methods.
- ADDED: `f32`/`f64` semantics, float `std::math`, float parse/format.
- ADDED: numeric operator traits and conversion traits.
- ADDED: typed numeric literal suffixes, bases, and digit separators.

## Non-goals

- Arbitrary-precision integers / decimals / rationals (a library, proposable
  later).
- SIMD vector types.
