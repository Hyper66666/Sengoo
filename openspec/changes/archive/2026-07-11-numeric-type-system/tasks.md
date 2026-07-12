## 0. Backend and target contract

- [x] 0.1 Freeze LLVM-text as the production backend for this change,
  Cranelift as experimental, and target-triple pointer width in `design.md`.
- [x] 0.2 Add a backend-capability test proving unsupported Cranelift programs
  fail explicitly rather than silently using different semantics.
  - Evidence: `cranelift_fast_jit_rejects_non_primitive_runtime_calls` and
    `tools/sgc/tests/cranelift_numeric.rs` cover rejection and acceptance.

## 1. Integer types and conversions

- [x] 1.1 Add `i8/i16/i32/i64`, `u8/u16/u32/u64`, `isize/usize` to the type
  system and production LLVM-text codegen, with target-triple pointer width.
  - Complete: source types and literal/cast lowering carry signed,
    fixed-width unsigned, float, and pointer-sized `isize`/`usize` through the
    LLVM-text path. The opt-in Cranelift fast-JIT now emits and executes bool,
    every fixed-width integer, and pointer-sized integer arithmetic/casts
    directly as Cranelift IR instead of pre-evaluating a Rust constant.
    Pointer-sized integers use the selected 32-bit or 64-bit target triple.
    Full MIR Cranelift parity remains outside this production archive gate.
- [x] 1.2 Define `as` casts (documented truncation/sign behavior) and the v1
  concrete checked conversion family
  `checked_<source>_to_<target>(value) -> Result<Target, i64>`.
  - Complete: explicit scalar `as` casts parse, type-check, lower to MIR,
    and codegen through LLVM-text and JIT paths for signed integers, fixed-width
    unsigned integers, and floats. `std::math` exposes every non-identity
    integer pair through the checked family with target-width-aware pointer
    destinations. Float-to-integer casts use saturating LLVM intrinsics, so
    NaN and out-of-range inputs are defined. Unsigned tokens carry `u64`, so
    `u64`/`usize` literals above `i64::MAX` compile; unsuffixed literals above
    `i64::MAX` are rejected with a diagnostic requiring an unsigned suffix.
- [x] 1.3 Tests for each width's arithmetic, comparisons, and conversions.
  - Complete: `cargo test -p sengoo-compiler cast_semantics --lib -- --nocapture`
    covers same-width and mixed-width signed/unsigned arithmetic, unsigned
    comparison/division/remainder/right-shift lowering, suffix/cast lowering,
    pointer-sized native-target lowering, and large suffixed `u64` literals.
    `cargo test -p sengoo-compiler math_module --lib -- --nocapture` covers
    checked conversion and overflow helper surfaces across signed, unsigned,
    and pointer-sized widths. Cranelift unit/CLI tests cover all integer widths,
    bool, signed/unsigned arithmetic, comparisons, division, remainder,
    bitwise operations, shifts, and scalar casts. `numeric_boundaries` covers
    every width, the complete checked matrix, and 32/64-bit pointer targets.

## 2. Overflow semantics

- [x] 2.1 Trap-on-overflow in debug builds, wrap in release builds, for `+ - *`.
  - Complete: LLVM-text codegen receives an integer overflow mode from
    compiler options / `sgc -O`. `O0/O1` materialize `llvm.*.with.overflow`
    checks for integer `+ - *` and route the overflow flag through the runtime
    trap helper, while `O2/O3` keep the existing plain wrapping IR. The legacy
    JIT IR path has matching debug checked emission. The Cranelift primitive
    fast-JIT uses checked overflow instructions plus traps at O0/O1 and plain
    wrapping instructions at O2/O3. All widths are covered by production IR
    tests and representative native execution. Evidence: `cargo test -p sgc native_integer_ -- --nocapture` covers
    the LLVM/native user path, while `cargo test -p sgc --test
    cranelift_numeric -- --nocapture` covers isolated Cranelift trap/wrap
    behavior.
- [x] 2.2 Provide `wrapping_*`, `checked_*` (-> `Option`), and `saturating_*`
  methods on integer types.
  - Complete: `std::math` exposes `i64` inherent methods for
    `wrapping_add/sub/mul`, `checked_add/sub/mul -> Option<i64>`, and
    `saturating_add/sub/mul`. `i32`/`i16`/`i8` now expose the same method
    family using widened i64 arithmetic plus casts, so they do not depend on
    runtime helper symbols. `u32`/`u16`/`u8` now expose the same method family
    using widened arithmetic plus casts. `u64` now exposes the same method
    family through runtime helpers. `isize` and `usize` derive bounds from the
    selected target width. Evidence: compiler math tests, `numeric_boundaries`,
    and the native `numeric_runtime` suite.
- [x] 2.3 Tests covering each mode and a documented division-by-zero behavior.
  - Complete: compiler option tests assert that `O0` emits checked LLVM
    overflow intrinsics plus the runtime trap helper for integer addition and
    that `O2` keeps plain wrapping IR; O0 unsigned `u32` addition is locked to
    `llvm.uadd.with.overflow.i32`. JIT debug tests cover the same overflow and
    zero-divisor helper calls, including unsigned `u32` overflow using
    `llvm.uadd.with.overflow` and unsigned zero-divisor checks preserving
    zero-extension. `O0/O1` integer division/remainder also call the runtime
    zero-divisor trap helper before the LLVM division; `O2/O3` keep the legacy
    plain division path. The user documentation records this contract.
    Automated reference-host smoke confirms an `O0` overflowing i64 add prints
    `Integer overflow` and exits nonzero, while the same program at `O2` wraps
    and exits successfully; it also confirms O0 integer division by zero prints
    `Division by zero` and exits nonzero. Cranelift CLI subprocess tests confirm
    debug i32 overflow and integer division by zero terminate unsuccessfully,
    while the same overflowing i32 addition at O2 prints `-2147483648`.

## 3. Floats

- [x] 3.1 `f32`/`f64` IEEE-754 arithmetic, comparisons, and `NaN`/`inf`
  predicates (`is_nan`, `is_infinite`, `is_finite`).
- [x] 3.2 Float `std::math`: `sqrt/pow/exp/ln/floor/ceil/round/abs/min/max` and
  core trig.
- [x] 3.3 Float parse (`strconv`) and format (via the formatting layer with
  precision specs).
- [x] 3.4 Tests for float math, parse round-trips, and `{:.3}` formatting.
  - Evidence: compiler surface tests cover f32/f64 math and strconv wrapper
    lowering; format tests cover f64 precision placeholders; sgc runtime smoke
    covers f64 parse/format and f32/f64 `NaN`/`inf` predicates.

## 4. Numeric traits and literals

- [x] 4.1 Operator traits `Add/Sub/Mul/Div/Rem/Neg` wired to the operators
  (coordinate with `generics-and-trait-system`).
  - `std::math` defines `Add/Sub/Mul/Div/Rem<Rhs, Output>` and `Neg<Output>`.
    The final trait parameter is the explicit source-level equivalent of an
    associated `Output` while qualified `Self::Output` syntax remains absent.
    Concrete and generic operator expressions enforce the exact Rhs/Output
    contract, lower user-defined values to uniquely selected static impl calls,
    and retain primitive numeric intrinsic lowering backed by compiler-known
    impls that satisfy exact generic bounds. Stable diagnostics cover
    missing impls, ambiguous outputs, malformed trait contracts, and method
    return/Output mismatches. Evidence: `cargo test -p sengoo-compiler --test
    numeric_operator_traits -- --nocapture`.
- [x] 4.2 `From`/`Into` for lossless widening; document why narrowing is not a
  `From`.
  - Complete: `.into()` uses the direct expected type from an annotated `let`,
    a tail or explicit return, and a concrete function parameter to select the
    exact `Into<T>` impl. Type checking records that decision for HIR/MIR, so
    ABI-equivalent `i64`/`isize` and `u64`/`usize` remain semantically distinct.
    `std::math` defines the mirrored `From<T>::from_value` surface (`from` is a
    reserved import keyword) and every lossless widening among the supported
    fixed-width integers, portable pointer-sized conversions, and `f32 -> f64`;
    no narrowing or target-dependent `u32 -> isize` impl is provided. Evidence:
    `cargo test -p sengoo-compiler --test numeric_conversions -- --nocapture`
    (8/8) and `cargo test -p sengoo-compiler math_module --lib -- --nocapture`
    (5/5).
- [x] 4.3 Lexer: typed suffixes, `0x`/`0o`/`0b` bases, and `_` digit separators
  (shared grammar with `first-class-strings-and-formatting`).
  - Complete: the lexer recognizes the full suffix/base/separator grammar and
    compiler tests cover the tokenization path. Parser semantics now preserve
    signed integer, fixed-width unsigned integer, and float suffixes through the
    explicit `as` cast pipeline; pointer-sized suffixes (`usize`/`isize`) use
    the selected target width. Suffixed `u64` values above `i64::MAX` are
    supported, and malformed/out-of-range literals have stable diagnostics.
- [x] 4.4 Tests for suffixed/based/separated literals and operator-trait dispatch.
  - Complete: signed integer, fixed-width unsigned integer, and float suffixes
    are covered through the explicit cast pipeline, including unsigned
    comparison/division/remainder/right-shift lowering. Pointer-sized suffixes
    are covered on 32-bit and 64-bit targets, and suffixed `u64` literals
    above `i64::MAX` have regression coverage. Evidence: `cargo test -p
    sengoo-compiler cast_semantics --lib -- --nocapture`. Operator dispatch for
    all six arithmetic traits, generic bounds, explicit Output, primitive
    intrinsic preservation, and stable negative diagnostics is covered by the
    `numeric_operator_traits` integration suite. Cranelift tests additionally lock primitive
    signed/unsigned operator execution without routing through trait dispatch.

## 5. Stdlib migration and docs

- [x] 5.1 Add generic numeric helpers alongside the existing `*_i64` ones; keep
  `*_i64` names source-compatible.
  - `std::math` exposes trait-bound `numeric_abs`, `numeric_min`, `numeric_max`,
    and `numeric_clamp` entry points over every supported signed, unsigned,
    pointer-sized integer and f32/f64 family while retaining every legacy
    `*_i64` helper.
- [x] 5.2 Document the numeric model (widths, overflow, floats, conversions) in
  `docs/language-features.md`.
  - `docs/language-features.md` records target-width pointer integers, cast and
    saturation rules, the complete checked family, overflow modes, float
    behavior, operator traits, backend tiers, and deliberate non-goals.
- [x] 5.3 Run `openspec validate numeric-type-system --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (numeric/codegen lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New numeric examples compile, link, and run
