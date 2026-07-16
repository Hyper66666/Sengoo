## 1. Baselines and diagnostics

- [ ] 1.1 Inventory existing positive/negative tests for borrow, Drop, match,
  traits, arrays, and structured exits.
- [ ] 1.2 Freeze diagnostic names and text/JSON/LSP parity tests.
- [ ] 1.3 Add failing conformance tests for every scenario in the delta specs.

## 2. Borrow and move precision

- [ ] 2.1 Implement intraprocedural last-use borrow termination across branches
  and loops with conservative fallback for unresolved control flow.
- [ ] 2.2 Reject returning references to locals, temporaries, or by-value owners;
  permit references derived from input references.
- [ ] 2.3 Track named-field partial moves and reject whole-value/moved-field use.
- [ ] 2.4 Prove owner mutation/move after a borrow's last reachable use.

## 3. Drop completeness

- [ ] 3.1 Drop unmoved temporaries at full-expression boundaries.
- [ ] 3.2 Track conditional initialization and moved paths with runtime drop flags.
- [ ] 3.3 Generate exact-once recursive Drop for nested aggregates, arrays, enums,
  and monomorphized generic wrappers.
- [ ] 3.4 Prove reverse order across fallthrough, `return`, `?`, `break`, and
  `continue`; document no-unwind panic behavior.
- [ ] 3.5 Add sanitizer/leak harness scenarios for every owning runtime domain
  used by the conformance fixture.

## 4. Match and control flow

- [ ] 4.1 Implement exhaustive enum/bool coverage and wildcard requirements for
  open domains.
- [ ] 4.2 Treat guards as non-covering and reject unreachable arms.
- [ ] 4.3 Apply move/borrow semantics to payload bindings and guarded exits.
- [ ] 4.4 Run positive/negative match tests through parser, typeck, MIR, native
  execution, JSON diagnostics, and LSP diagnostics.

## 5. Traits, derives, and arrays

- [ ] 5.1 Resolve `Self::Assoc` and `T::Assoc` in every pinned type position.
- [ ] 5.2 Implement receiver-less trait method declaration and
  `Trait::method`/`Type::method` resolution with ambiguity diagnostics.
- [ ] 5.3 Complete current derive set for supported named structs/enums and keep
  Copy/Drop exclusivity.
- [ ] 5.4 Implement fixed-array bounds, iteration, whole-value move, and reverse
  element Drop; reject indexed partial moves.

## 6. Evidence and archive

- [ ] 6.1 Add a package conformance fixture combining every M1 surface without
  manual release calls.
- [ ] 6.2 Run compiler lib/integration tests, `sgc` conformance, `sglsp`
  diagnostic parity, sanitizer/leak gates, and warnings-denied Clippy.
- [ ] 6.3 Update `docs/language-reference.md` statuses and proof links.
- [ ] 6.4 Run `openspec validate v0-2-language-coherence --strict` and
  `openspec validate --all --strict`.
- [ ] 6.5 Archive this change and unblock M2/M3 public API archive work.
