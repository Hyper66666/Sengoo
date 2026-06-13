## 1. Integer types and conversions

- [ ] 1.1 Add `i8/i16/i32/i64`, `u8/u16/u32/u64`, `isize/usize` to the type
  system and codegen (LLVM-text + Cranelift).
- [ ] 1.2 Define `as` casts (documented truncation/sign behavior) and checked
  conversions returning `Result`.
- [ ] 1.3 Tests for each width's arithmetic, comparisons, and conversions.

## 2. Overflow semantics

- [ ] 2.1 Trap-on-overflow in debug builds, wrap in release builds, for `+ - *`.
- [ ] 2.2 Provide `wrapping_*`, `checked_*` (-> `Option`), and `saturating_*`
  methods on integer types.
- [ ] 2.3 Tests covering each mode and a documented division-by-zero behavior.

## 3. Floats

- [ ] 3.1 `f32`/`f64` IEEE-754 arithmetic, comparisons, and `NaN`/`inf`
  predicates (`is_nan`, `is_infinite`, `is_finite`).
- [ ] 3.2 Float `std::math`: `sqrt/pow/exp/ln/floor/ceil/round/abs/min/max` and
  core trig.
- [ ] 3.3 Float parse (`strconv`) and format (via the formatting layer with
  precision specs).
- [ ] 3.4 Tests for float math, parse round-trips, and `{:.3}` formatting.

## 4. Numeric traits and literals

- [ ] 4.1 Operator traits `Add/Sub/Mul/Div/Rem/Neg` wired to the operators
  (coordinate with `generics-and-trait-system`).
- [ ] 4.2 `From`/`Into` for lossless widening; document why narrowing is not a
  `From`.
- [x] 4.3 Lexer: typed suffixes, `0x`/`0o`/`0b` bases, and `_` digit separators
  (shared grammar with `first-class-strings-and-formatting`).
  - Integer and float suffixes plus based/separated integer literals are lexed
    by `compiler/src/lexer/token.rs` and covered by compiler tests.
- [ ] 4.4 Tests for suffixed/based/separated literals and operator-trait dispatch.

## 5. Stdlib migration and docs

- [ ] 5.1 Add generic numeric helpers alongside the existing `*_i64` ones; keep
  `*_i64` names source-compatible.
- [ ] 5.2 Document the numeric model (widths, overflow, floats, conversions) in
  `docs/language-features.md`.
- [x] 5.3 Run `openspec validate numeric-type-system --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (numeric/codegen lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New numeric examples compile, link, and run
