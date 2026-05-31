## Why

Benchmarks show the compiler frontend dominates end-to-end build time (~87–90% at 1000k LOC), so frontend cost is the highest-leverage place to optimize. Until now there was no per-phase visibility, so optimization risked targeting the wrong stage. New phase-level profiling (`SENGOO_PHASE_TIMINGS`) revealed the real picture: HIR→MIR lowering was the #1 cost because `LoweringContext::new` deep-cloned the program-global function tables once per function (an O(n²) blowup), and the type checker is the #2 cost. The first hotspot is now fixed (frontend −62% on a 110k LOC probe); this change captures the remaining program — directions, sequencing, and the measurement/verification process — so the work proceeds evidence-first instead of guesswork-first.

## What Changes

- Establish a **profile-first frontend optimization program** with an explicit, repeatable process: measure with `SENGOO_PHASE_TIMINGS`, decide the next lever from data, implement one reviewable slice, prove it against a bench gate, and keep the four-suite verification baseline green.
- **Phase 0 (landed in this change's groundwork):** per-phase frontend timing instrumentation (`parse/typeck/hir_lower/mir_lower/mir_opt`) and the MIR-lowering O(n²) clone fix (`Cow`-backed `known_functions`/`function_sigs`).
- **Phase 1 (next):** migrate type-checker storage boundaries (`SymbolKind::{Var,Function,Type,Const,Static}` and the trait/impl registry `FunctionTy`/`MethodSig`/`ImplInfo`) from owned `Ty` to interned handles, completing the deferred Phase 2 of the interned-types baseline — now justified as the #1 remaining frontend cost.
- **Phase 2:** remove the residual `to_mut()` whole-table clone on lambda/async/generic-heavy functions via a per-context local overlay so lowering stays O(n) even on those paths.
- **Phase 3+ (backlog, data-gated):** frontend parallelization, finer incremental invalidation boundaries, MIR-optimization cost control if it grows, parser/lexer tuning, and compiler peak-RSS convergence toward the C++ baseline.
- No **BREAKING** source-language syntax or semantic change in any phase.

## Capabilities

### New Capabilities

- `frontend-compile-perf`: Defines the compiler's frontend compile-performance capability — opt-in per-phase timing observability, the per-phase performance invariants (no per-function duplication of program-global tables; lowering stays linear in function count), and the profile→implement→bench-gate→verify process that governs how frontend optimizations are sequenced and accepted.

### Modified Capabilities

- `interned-types`: Extend the existing storage-boundary requirement so that symbol storage and the trait/impl registry MUST store interned type handles rather than owned `Ty`, promoting the previously-deferred Phase 2 sweep from "preferred" to a required boundary for these long-lived stores.

## Impact

- Affected code:
  - Instrumentation (Phase 0, done): `tools/sgc/src/{pipeline.rs, main.rs, commands/build.rs, commands/run.rs, tests.rs}`
  - Lowering O(n²) fix (Phase 0, done): `compiler/src/mir/lowering/{mod.rs, context_methods.rs, lambda_expr_helpers.rs, async_methods.rs}`
  - Typeck storage (Phase 1): `compiler/src/typeck/{env.rs, trait.rs, infer.rs, check.rs, check/*}`
  - Lambda/async overlay (Phase 2): `compiler/src/mir/lowering/{mod.rs, context_methods.rs}` and the insert sites
- Public Sengoo language syntax and semantics are unchanged.
- Compiler-internal Rust APIs change at storage boundaries (owned `Ty` → interned handle) and add opt-in timing output on stderr.
- No new external dependencies.
- Verification gate per phase: `cargo test -p sengoo-compiler --lib`, `-p sgc`, `-p sengoo-runtime --lib`, `-p sgpm` must stay green, plus a bench-measured frontend delta for each performance phase.
