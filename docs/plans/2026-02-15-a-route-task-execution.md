# A Route Task Execution (Stability + Diagnostics + DX)

## Week 1 Diagnostics Quality
- [x] Replace frequent `expected field name` with readable, positionable structured diagnostics.
- [x] Add regression tests for invalid struct field names.
- [x] Add actionable diagnostic for `field shorthand requires identifier`.
- [x] Unify high-frequency parser diagnostics (bilingual/error-code strategy).

## Week 2 Stability and Regression
- [x] Build parser/typeck historical regression suite (at least 20 cases).
- [x] Add property tests covering conditional expressions, struct literals, and match patterns.
- [x] Add minimal cross-platform E2E smoke tests (Windows/Linux).

## Week 3 Run-Path UX
- [x] Module-level cache design and dependency invalidation rules.
- [x] Reduce unnecessary rebuilds on the `sgc run` hot path.
- [x] Add observable output for cache hit/miss reasons.

## Week 4 LSP Developer Experience
- [x] Implement core `rename/references` capability.
- [x] Align `semantic tokens` with type-aware highlighting.
- [x] Add at least 3 common quick fixes.
