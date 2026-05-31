## Why

Benchmarks show the compiler frontend dominates end-to-end build time (~87–90% at 1000k LOC), so frontend cost is the highest-leverage place to optimize. Until now there was no per-phase visibility, so optimization risked targeting the wrong stage. New phase-level profiling (`SENGOO_PHASE_TIMINGS`) revealed the real picture: HIR→MIR lowering was the #1 cost because `LoweringContext::new` deep-cloned the program-global function tables once per function (an O(n²) blowup), and the type checker is the #2 cost. The first hotspot is now fixed (frontend −62% on a 110k LOC probe); this change captures the remaining program — directions, sequencing, and the measurement/verification process — so the work proceeds evidence-first instead of guesswork-first.

## What Changes

- Establish a **profile-first frontend optimization program** with an explicit, repeatable process: measure with `SENGOO_PHASE_TIMINGS`, decide the next lever from data, implement one reviewable slice, prove it against a bench gate, and keep the four-suite verification baseline green.
- **Phase 0 (landed in this change's groundwork):** per-phase frontend timing instrumentation (`parse/typeck/hir_lower/mir_lower/mir_opt`) and the MIR-lowering O(n²) clone fix (`Cow`-backed `known_functions`/`function_sigs`).
- **Phase 1:** reset dead type-inference substitutions at each function and method body boundary. Profiling showed substitution accumulation, rather than symbol-storage cloning, was the quadratic type-checker cost. The broader interned-handle storage sweep remains data-gated and deferred.
- **Phase 2:** remove the residual `to_mut()` whole-table clone on lambda/async/generic-heavy functions via a per-context local overlay so lowering stays O(n) even on those paths.
- **Phase 3+ (backlog, data-gated):** frontend parallelization, finer incremental invalidation boundaries, MIR-optimization cost control if it grows, parser/lexer tuning, and compiler peak-RSS convergence toward the C++ baseline.
- No **BREAKING** source-language syntax or semantic change in any phase.

## Capabilities

### New Capabilities

- `frontend-compile-perf`: Defines the compiler's frontend compile-performance capability — opt-in per-phase timing observability, the per-phase performance invariants (no per-function duplication of program-global tables; lowering stays linear in function count), and the profile→implement→bench-gate→verify process that governs how frontend optimizations are sequenced and accepted.

### Modified Capabilities

- None in this slice. The symbol-storage and trait/impl registry interned-handle sweep remains a data-gated follow-up.

## Impact

- Affected code:
  - Instrumentation (Phase 0, done): `tools/sgc/src/{pipeline.rs, main.rs, commands/build.rs, commands/run.rs, tests.rs}`
  - Lowering O(n²) fix (Phase 0, done): `compiler/src/mir/lowering/{mod.rs, context_methods.rs, lambda_expr_helpers.rs, async_methods.rs}`
  - Typeck substitution reset (Phase 1): `compiler/src/typeck/{infer.rs, check.rs, check/*}`
  - Lambda/async overlay (Phase 2): `compiler/src/mir/lowering/{mod.rs, context_methods.rs}` and the insert sites
- Public Sengoo language syntax and semantics are unchanged.
- Compiler-internal Rust behavior resets dead substitutions between bodies and adds opt-in timing output on stderr.
- No new external dependencies.
- Verification gate per phase: `cargo test -p sengoo-compiler --lib`, `-p sgc`, `-p sengoo-runtime --lib`, `-p sgpm` must stay green, plus a bench-measured frontend delta for each performance phase.
