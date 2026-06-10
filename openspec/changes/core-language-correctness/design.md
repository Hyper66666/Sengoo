## Scope

This change owns the correctness of Sengoo's **documented core language
surface**: native fixed-size arrays, environment-capturing closures, mutable
local bindings, and enum variants used as values. It is correctness-first: each
covered form must lower to valid LLVM IR, run with the expected result, ship a
runnable example, and be guarded by an executable test and a CI conformance
gate. It also owns the reproducibility (lockfile + pinned toolchain) and
doc/implementation reconciliation needed to make those guarantees trustworthy.

## Existing Ownership To Avoid

- `language-surface-expansion` owns the phase 4a attribute matrix, class header
  trait lists, and dynamic native i64 FFI arity `0..=8`.
- `language-default-polish` owns additive language-surface relaxations and
  diagnostic parity for adjacent rejected forms.
- Archived `try-and-match-ergonomics` owns `?`, `try {}`, match pattern
  semantics, exhaustiveness, and unreachable-arm diagnostics. This change adds
  the **value-construction** side of enums and reuses, rather than re-specifies,
  the match/pattern semantics.
- Archived `owned-string-text` owns canonical stdlib `String` and move
  semantics.
- `stdlib-breadth-mainstream` and stdlib followups own runtime-backed `Vec<T>`,
  `HashMap`, and other growable collections. This change covers native
  fixed-size `[T; N]` only.
- `async-reactor-futures`, `concurrent-async-runtime`, and
  `runtime-hardening-ffi-async` own async/runtime semantics.

## Observed Failures (Reproductions)

All reproduced from a clean source build on Linux (`rustc` 1.96, `clang` +
`lld`), running `target/release/sgc run <file>`.

| Area | Repro | Observed result |
| --- | --- | --- |
| Array index | `examples/04_array.sg` (`arr[1]`) | `error: '%u_4' defined with type '[3 x i64]*' but expected 'i64*'` at array `getelementptr`; `compile failed` |
| Array `for` | `examples/05_loop.sg`, `docs/language-features.md` §2.2 | same `[N x i64]*` vs `i64*` IR type error; `compile failed` |
| Closure capture | `examples/06_lambda.sg` (`|y| x + y`) | `error: '%u_5' defined with type '[1 x i64]*' but expected 'i64*'`; `compile failed` |
| Mutable binding | `let mut i = 0;` | `parse error: invalid pattern: expected identifier` |
| Mutability enforcement | `let s = 0; s = s + 1;` | compiles and runs (no immutability error) |
| Enum value | `code(Color::Green)` in value position | `type check error: undefined variable: Color::Green` |
| Conformance/CI | `cargo test` on Linux | 670/681 pass; 11 fail (basic-example `insta` snapshots + one Linux `#[cfg]` attribute test) |
| Reproducibility | fresh `cargo build` | no committed `Cargo.lock`; resolves newer deps that require a newer Rust than the README toolchain |

## Approach

1. **Conformance contract first.** Define the pinned core-conformance form list
   and a single harness (extend `tools/sgc` example smoke or add a dedicated
   conformance runner) that compiles each form's example and asserts its result.
   Add the currently-failing forms as expected-failing entries so the gate
   measures progress and prevents silent regressions.
2. **Array lowering fix.** Correct the fixed-size array value/address path so an
   array place decays to an element pointer before `getelementptr` indexing
   (the IR currently indexes `[N x T]*` where the element pointer `T*` is
   expected). Cover both `arr[i]` load/store and `for v in arr` iteration.
3. **Closure lowering fix.** Correct environment capture so the captured-variable
   slot is loaded/stored at the right pointer type, matching the array fix where
   the environment is itself an aggregate.
4. **Mutability.** Parse `let mut <ident>` (and the `mut` binding pattern), thread
   a mutability flag through HIR/typeck, and emit a stable diagnostic when an
   immutable binding is assigned. Keep this consistent across `sgc` JSON and
   `sglsp`. Decide and document whether existing immutable-reassignment programs
   are migrated or grandfathered (migration-gated if source-incompatible).
5. **Enum values.** Resolve `Enum::Variant` in value position (and payload
   construction for variants with fields) and lower to the discriminant/payload
   representation already consumed by `match`. Add negative tests for unknown
   variants and arity/type mismatches.
6. **Reproducibility + docs.** Commit `Cargo.lock`, add `rust-toolchain.toml`,
   wire the conformance gate and `cargo test` into CI on Linux, correct the
   backend description, drop unused `inkwell`/`llvm-sys`/`pyo3` workspace deps,
   and update `PROGRESS.md` flags to verified state.

## Pinned Decisions For This Change

### Core conformance form list (v1)

The following forms MUST compile to valid LLVM IR and run with the documented
result, each with a committed example and executable test:

- integer and `f64` arithmetic; `bool`; string literal + `print`
- `if` / `else`, `while`, `for v in 0..N` range loops, recursion
- `struct` definition, construction, field access, and `impl` methods
- native fixed-size array literal, `arr[i]` index read, and `for v in arr`
- environment-capturing closure call
- mutable local via `let mut`, with immutable-assignment rejection
- enum variant value construction and `match` over it

### Arrays

Scope is native fixed-size `[T; N]` only. Growable/dynamic arrays remain
runtime-backed stdlib `Vec<T>` and are out of scope here.

### Mutability

`let mut` becomes the only way to obtain a reassignable local. Assigning an
immutable `let` binding is an error with a stable diagnostic code. If committed
sources or examples rely on immutable reassignment, the cleanup is migration-
gated: a migration note is added before any source-incompatible enforcement
lands, and examples are updated in the same change.

### Enums

Value-position variant construction lowers to the same discriminant (and, for
payload variants, payload) representation that `match` already consumes; no new
match syntax is introduced.

### Conformance gate

The gate runs in CI on Linux and fails the build if any pinned core form fails
to compile, fails to run, or produces the wrong result, or if `cargo test`
regresses. The 11 currently-failing tests are either fixed or explicitly
re-baselined with a recorded reason before the gate is declared green.

## Risks And Mitigations

- **Array/closure IR fix scope creep into the type system.** Mitigate by keeping
  the fix at the lowering/codegen pointer-type layer and gating behavior with the
  conformance examples, not by reworking type inference.
- **Mutability enforcement breaks existing sources.** Mitigate with the migration
  gate and same-change example updates.
- **Snapshot churn.** Re-baseline `insta` snapshots deliberately and review the
  diff so a green gate reflects correct output, not accepted-bad output.
