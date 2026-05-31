## Context

End-to-end benchmarks show the compiler frontend is ~87–90% of build time at 1000k LOC, while codegen and link are small. Before this work the frontend was a single opaque bucket, so there was no way to know which stage to optimize.

Phase-level instrumentation (`SENGOO_PHASE_TIMINGS`) was added and run on a 10k-function / 110k-LOC probe. Release measurements (before the first fix):

| phase | release -O0 | share |
| --- | ---: | ---: |
| parse | 105 ms | 0.4% |
| typeck | 10,138 ms | 37% |
| mir_lower | 16,767 ms | 62% |
| mir_opt | ~0 ms | ~0% |

The #1 cost was HIR→MIR lowering, because `LoweringContext::new` cloned the program-global `known_functions: HashSet<String>` and `function_sigs: HashMap<String, FunctionSig>` once per function — an O(n²) blowup (~100M clones for 10k functions). It was owned only so generic/lambda/async materialization could insert into it. Replacing the owned fields with `Cow` (borrow for reads, `to_mut()` only on insert) cut `mir_lower` from 16,767 ms to 80 ms (−99.5%) and total frontend by −62%, with all four test suites green.

After that fix, **typeck (~10 s, ~97% of the remaining frontend) is the dominant cost**, which is exactly the storage boundary the `interned-types` baseline deferred to Phase 2. This design records the process and the full remaining program so subsequent work stays evidence-first.

## Goals / Non-Goals

**Goals:**

- Make per-phase frontend timing a permanent, opt-in, behavior-neutral measurement tool.
- Keep HIR→MIR lowering linear in function count (no per-function duplication of program-global tables).
- Complete the deferred interned-types Phase 2: store symbol and trait/impl-registry types as interned handles instead of owned `Ty`.
- Define a repeatable profile → implement-one-slice → bench-gate → verify loop, and a data-gated backlog for later directions.
- Keep every phase behavior-preserving and incrementally shippable.

**Non-Goals:**

- No Sengoo source syntax or semantic change.
- No big-bang rewrite of `Ty`/`TyKind` recursive fields.
- No thread-safe global type interner; the typeck path stays single-threaded.
- No new external dependencies.
- No commitment to Phase 3+ scope before profiling justifies it.

## Decisions

### Decision 1: Profile-first via opt-in `SENGOO_PHASE_TIMINGS`

Surface the per-phase timings the pipeline already records (and were discarded at the command layer), plus a `mir` sub-split into `hir_lower` / `mir_lower` / `mir_opt`. Gate on an env flag spelled like the existing `SENGOO_*` toggles; print to stderr so stdout artifacts are untouched.

- **Chosen:** env-gated stderr breakdown at the `build`/`run` command layer.
- **Alternative considered:** a `--timings` CLI flag.
- **Rationale:** the CLI threads through a many-arg daemon-dispatch path; an env flag matches existing conventions and is non-invasive.

### Decision 2: Phase 0 — fix lowering O(n²) with `Cow` (landed)

Convert `LoweringContext`'s `known_functions` / `function_sigs` fields to `Cow<'a, _>`; borrow the session base for reads (zero clone in the common no-materialization path), and `to_mut()` only at the existing insert sites (generic/lambda/async). Reads are transparent through `Cow`'s `Deref`/deref-coercion, so only ~6 write sites change.

- **Chosen:** `Cow` borrow + copy-on-write.
- **Alternative considered:** a separate local overlay map checked before the base.
- **Rationale:** `Cow` fixes the dominant common case with the smallest, lowest-risk diff and no read-site churn; the overlay is reserved for Phase 2 where it removes the residual whole-table clone on materialization-heavy functions.

### Decision 3: Phase 1 — intern typeck storage boundaries

Migrate `SymbolKind::{Var,Function,Type,Const,Static}` to store `InternedTyId`, and the trait/impl registry (`FunctionTy`/`MethodSig`/`ImplInfo`) `Vec<Ty>`/`Ty` fields likewise. `Symbol::get_ty` returns a handle (or a materialized snapshot via the env interner). The six known cloner sites from the interned-types §6.1 catalog (`infer.rs:63`, `check.rs:625`, `check.rs:832`, `check/decl_helpers.rs:332`, `check/class_hierarchy_helpers.rs:33`, `check/expr_helpers.rs:32-37`) are updated together.

- **Chosen:** migrate symbol + trait-registry storage to interned handles, materialize on demand.
- **Alternative considered:** leave typeck storage owned (the Phase-1 baseline stance).
- **Rationale:** profiling now shows typeck is the #1 remaining frontend cost, inverting the earlier "lower ROI" assessment; the interner, helpers (`env.intern_ty`, `env.symbol_ty_id`), and tests already exist to support the sweep.

### Decision 4: Phase 2 — local overlay for materialization-heavy lowering

For functions that materialize many lambda/async/generic instances, replace the `Cow::to_mut()` whole-table clone with a small per-context overlay map consulted before the shared base, keeping lowering O(n) even on those paths.

- **Chosen:** overlay map layered over the shared base, consulted on read, written on insert.
- **Alternative considered:** keep `Cow` only.
- **Rationale:** `Cow` still clones the whole base once per materializing function; an overlay makes inserts O(1) and removes the last quadratic corner — but only worth doing if profiling shows lambda/async-heavy inputs regressing.

### Decision 5: Phase 3+ backlog is data-gated

Later directions are recorded but not scheduled until measurements justify them: (a) frontend parallelization of typeck/lowering across functions; (b) finer incremental invalidation boundaries; (c) MIR-optimization cost control if `mir_opt` grows on real workloads; (d) parser/lexer tuning (currently <1%); (e) compiler peak-RSS convergence toward the C++ baseline (1000k ≈ 3.14×). Each must re-measure before implementation.

- **Chosen:** explicit backlog with a re-profile precondition.
- **Rationale:** prevents speculative optimization of sub-1% stages and keeps effort on the measured critical path.

### Decision 6: Acceptance process

Every performance phase ships as a reviewable slice, keeps the four-suite verification baseline green (`sengoo-compiler --lib`, `sgc`, `sengoo-runtime --lib`, `sgpm`), and records a before/after `SENGOO_PHASE_TIMINGS` measurement on a representative probe.

## Risks / Trade-offs

- **Risk:** `Cow::to_mut()` still clones the whole base for each materializing function. → **Mitigation:** Phase 2 overlay; until then it is strictly better than the previous unconditional per-function clone.
- **Risk:** Phase 1 changes `Symbol::get_ty`'s return contract, touching ~6 destructure/clone sites at once. → **Mitigation:** migrate them in one coordinated slice with the suite green before/after; keep an owned-`Ty` materialization adapter for diagnostics.
- **Risk:** debug-build profiling distorts absolute numbers. → **Mitigation:** confirm rankings with a release build before committing to a target (already done for Phase 0/1).
- **Risk:** interned handles from different sessions could be mixed. → **Mitigation:** the interner is session-local and shared via `Rc<RefCell<…>>`; never expose raw IDs without the owning interner.
- **Risk:** scope creep into Phase 3+ without evidence. → **Mitigation:** Decision 5's re-profile precondition.

## Migration Plan

1. **Phase 0 (done):** add `SENGOO_PHASE_TIMINGS` + `mir` sub-split; fix lowering O(n²) via `Cow`. Verify four suites green; record −62% frontend.
2. **Phase 1:** intern `SymbolKind` storage; update the six cloner sites; intern the trait/impl registry; verify suites green; bench typeck delta on the 10k probe.
3. **Phase 2 (if warranted):** add the lowering overlay map; bench a lambda/async-heavy probe.
4. **Phase 3+ (data-gated):** re-profile, then pick from the backlog (parallelization / incremental / RSS / parser / mir_opt).

Rollback per phase is local: Phase 0 reverts the `Cow` fields and the instrumentation; Phase 1 reverts storage types to owned `Ty` (compatibility adapters keep call sites compiling); later phases are independent.

## Open Questions

- Should `Symbol::get_ty` return `InternedTyId` directly, or keep returning a materialized `Ty` snapshot (interning internally) to minimize call-site churn in Phase 1?
- Should the trait/impl registry expose interned-handle lookup APIs, or materialize at the existing lookup boundaries only?
- Is a lambda/async-heavy real workload available to justify Phase 2, or should it wait for a reported regression?
- After Phase 1, is the per-instance `Ty.id` origin tag still read by any non-debug consumer (candidate for removal)?
