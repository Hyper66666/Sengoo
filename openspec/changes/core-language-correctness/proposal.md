## Why

Sengoo's tooling track (`sgpm`, `sglsp`, `sgfmt`, `sgc doc`, incremental
pipeline, reflection, async subset, runtime-backed stdlib) is ahead of the
language core. Building the toolchain from source and running the committed
surface shows that several **documented core forms do not compile or run**, even
though `PROGRESS.md` marks them done:

- Native array indexing and array `for` iteration emit invalid LLVM IR and fail
  `clang` verification. `examples/04_array.sg`, `examples/05_loop.sg`, and the
  `docs/language-features.md` §2.2 `for v in arr` snippet all fail with
  `'[N x i64]*' ... but expected 'i64*'` at the array `getelementptr`.
- Closures that capture their environment emit invalid LLVM IR.
  `examples/06_lambda.sg` (`|y| x + y`) fails with the same class of error.
- `let mut x = ...` does not parse: `parse error: invalid pattern: expected
  identifier`. Separately, reassigning an immutable `let` binding compiles
  without error, so mutability is neither expressible nor enforced.
- Enum variants are not usable as values: `Color::Green` in value position fails
  type checking with `undefined variable: Color::Green`, even though the `match`
  pattern side parses. The committed `examples/ergonomics/03_enum_match.sg`
  passes only because its `main` never constructs an enum value.

These are the structural blockers between Sengoo and a mainstream-usable
language: a user writing ordinary code (arrays, loops, closures, mutable locals,
enums-as-values) hits compiler/codegen failures on the first try. This change
owns making the **already documented core surface actually compile, run, and
stay green in CI**, and aligns those semantics with mainstream expectations.

## What Changes

- Pin a **Core Language Conformance** surface: the minimal set of core forms
  that MUST compile to valid LLVM IR and run with correct results, each backed
  by a runnable example and an executable test.
- Fix native fixed-size array (`[T; N]`) value/address lowering so indexing
  (`arr[i]`) and array `for v in arr` iteration produce valid IR and correct
  results.
- Fix environment-capturing closure lowering so `|x| ...` over captured locals
  produces valid IR and correct results.
- Make mutability **expressible and enforced**: accept `let mut`, and reject
  assignment to an immutable binding with a stable diagnostic, consistent across
  parser, type checker, `sgc` JSON diagnostics, and `sglsp`.
- Make enum variants **first-class values**: allow `Enum::Variant` (and
  payload-carrying construction where the variant has fields) in value position
  and lower it to a correct discriminant/payload representation aligned with the
  existing `match` semantics.
- Add a **conformance gate**: a CI-runnable check that builds every committed
  `.sg` example and asserts the expected result, plus the `cargo test` suite, on
  Linux. Commit a `Cargo.lock` and pin the Rust toolchain so the build is
  reproducible.
- Reconcile documentation with the implementation: correct the "LLVM backend"
  description versus the textual-IR-plus-`clang` reality, remove the unused
  `inkwell` / `llvm-sys` / `pyo3` workspace dependencies, and update
  `PROGRESS.md` status flags to match verified behavior.

## Capabilities

### New Capabilities

- `core-language-correctness`: a pinned core-conformance contract plus the
  array, closure, mutability, and enum-value correctness requirements and an
  example/test/CI gate that keeps the documented core surface green.

### Modified Capabilities

- None in canonical `openspec/specs/` today. This change cites, and does not
  re-specify, the semantics owned by `language-surface-expansion`,
  `language-default-polish`, archived `try-and-match-ergonomics`, and archived
  `owned-string-text`.

## Impact

- OpenSpec planning artifact only in this change-creation task; no compiler or
  runtime code is modified here.
- Future implementation will touch `compiler/src/mir/lowering.rs`,
  `compiler/src/codegen/` (LLVM IR emission), `compiler/src/parser/`,
  `compiler/src/typeck/`, `compiler/src/hir/`, `tools/sgc/` (example/conformance
  harness), `tools/sglsp/` (diagnostic parity), `.github/` (CI), `Cargo.lock`,
  and a `rust-toolchain.toml`.
- Parent umbrella: `mainstream-default-readiness` (core-language readiness arm).
- Docs touched by the reconciliation task: `README.md`, `README.zh-CN.md`,
  `docs/language-features.md`, `PROGRESS.md`, and root `Cargo.toml`.

## Non-Goals

- No new generic/trait system rework: this change does not redesign generics,
  add a GC, or change the ownership model beyond making `let mut` and
  immutable-reassignment diagnostics consistent.
- No re-specification of delivered work owned by other changes: `?`, `try {}`,
  match pattern semantics and exhaustiveness, owned `String`, attribute matrix,
  dynamic native i64 FFI arity, async runtime/reactor semantics.
- No dynamic/growable arrays in the language core here; runtime-backed `Vec<T>`
  via stdlib stays owned by the stdlib changes. This change covers native
  fixed-size arrays only.
- No new pattern syntax, match guards, or implicit error conversion.
- No performance targets; correctness and reproducibility only.
