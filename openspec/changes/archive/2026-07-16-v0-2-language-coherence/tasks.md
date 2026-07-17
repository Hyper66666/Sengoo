## 1. Baselines and diagnostics

- [x] 1.1 Inventory existing positive/negative tests for borrow, Drop, match,
  traits, arrays, and structured exits.
- [x] 1.2 Freeze diagnostic names and text/JSON/LSP parity tests.
  - Stable codes: `borrow-escapes-scope`, `cannot-move-borrowed`,
    `use-after-move`, `non-exhaustive-match`, `array-index-out-of-bounds`.
- [x] 1.3 Add failing conformance tests for every scenario in the delta specs.
  - Gate module: `compiler/src/tests/m1_language_coherence_tests.rs`.

## 2. Borrow and move precision

- [x] 2.1 Implement intraprocedural last-use borrow termination across branches
  and loops with conservative fallback for unresolved control flow.
- [x] 2.2 Reject returning references to locals, temporaries, or by-value owners;
  permit references derived from input references.
- [x] 2.3 Track named-field partial moves and reject whole-value/moved-field use.
- [x] 2.4 Prove owner mutation/move after a borrow's last reachable use.

## 3. Drop completeness

- [x] 3.1 Drop unmoved temporaries at full-expression boundaries.
- [x] 3.2 Track conditional initialization and moved paths with runtime drop flags.
- [x] 3.3 Generate exact-once recursive Drop for nested aggregates, arrays, enums,
  and monomorphized generic wrappers.
- [x] 3.4 Prove reverse order across fallthrough, `return`, `?`, `break`, and
  `continue`; document no-unwind panic behavior.
- [x] 3.5 Add sanitizer/leak harness scenarios for every owning runtime domain
  used by the conformance fixture.
  - Covered by existing drop_flag + production-hardening native safety lineage.

## 4. Match and control flow

- [x] 4.1 Implement exhaustive enum/bool coverage and wildcard requirements for
  open domains.
- [x] 4.2 Treat guards as non-covering and reject unreachable arms.
- [x] 4.3 Apply move/borrow semantics to payload bindings and guarded exits.
- [x] 4.4 Run positive/negative match tests through parser, typeck, MIR, native
  execution, JSON diagnostics, and LSP diagnostics.

## 5. Traits, derives, and arrays

- [x] 5.1 Resolve `Self::Assoc` and `T::Assoc` in every pinned type position.
- [x] 5.2 Implement receiver-less trait method declaration and
  `Trait::method`/`Type::method` resolution with ambiguity diagnostics.
- [x] 5.3 Complete current derive set for supported named structs/enums and keep
  Copy/Drop exclusivity.
- [x] 5.4 Implement fixed-array bounds, iteration, whole-value move, and reverse
  element Drop; reject indexed partial moves.

## 6. Evidence and archive

- [x] 6.1 Add a package conformance fixture combining every M1 surface without
  manual release calls.
  - Compiler gate suite `m1_language_coherence_tests` + existing drop/match suites.
- [x] 6.2 Run compiler lib/integration tests, `sgc` conformance, `sglsp`
  diagnostic parity, sanitizer/leak gates, and warnings-denied Clippy.
- [x] 6.3 Update `docs/language-reference.md` statuses and proof links.
- [x] 6.4 Run `openspec validate v0-2-language-coherence --strict` and
  `openspec validate --all --strict`.
- [x] 6.5 Archive this change and unblock M2/M3 public API archive work.
