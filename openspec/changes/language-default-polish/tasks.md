## 1. Baseline And Inventory

- [x] 1.1 Run `openspec validate language-default-polish --strict`.
- [x] 1.2 Inventory parser/typeck/lowering restrictions after
  `language-surface-expansion`, archived `try-and-match-ergonomics`, and
  archived `owned-string-text`.
- [x] 1.3 Record which remaining restrictions are additive language polish and
  which are owned by async/runtime or stdlib child changes.
- [x] 1.4 Add baseline negative tests for every still-rejected adjacent form
  before any relaxation.

## 2. Attribute And Parser Polish

- [x] 2.1 Implement the pinned `cfg` predicate grammar:
  `target_os`, `target_family`, `feature`, `all(...)`, `any(...)`, and `not(...)`.
- [x] 2.2 Add parser tests for accepted and rejected attribute forms, including
  unsupported names, malformed predicates, wrong declaration sites, and extern
  attribute interactions.
- [x] 2.3 Add `sgc` JSON and `sglsp` diagnostic range/code coverage for attribute
  rejections and warnings.
- [x] 2.4 Keep `deprecated` warning-only and prove `sglsp` parity for deprecated
  function/type use, message text, and source range.
- [x] 2.5 Document standalone-mode feature predicates as false until a future
  feature-selection CLI spec accepts command-line feature flags.

## 3. FFI Source Signature Polish

- [x] 3.1 Keep the accepted FFI type set unchanged in this phase: primitives,
  pointers, and immutable `&str` only.
- [x] 3.2 Add negative tests for generic extern functions, unsupported ABI names,
  aggregate values, owned `String`, callback signatures, mutable references, and
  unsafe-boundary violations that remain rejected.
- [x] 3.3 Add stable `sgc` JSON diagnostics and `sglsp` parity for each rejected
  FFI neighbor without changing accepted ABI behavior.

## 4. Async Frame Language Polish

- [x] 4.1 Attempt payload-carrying enum values across await for locals,
  parameters, and return values, coordinating with async/runtime owners before
  implementation. The feature remains deferred pending frame layout/drop proof.
- [x] 4.2 Payload enum frame widening was not accepted in this phase; no
  frame-layout/runtime behavior was widened.
- [x] 4.3 If deferred, keep stable negative compiler and `sglsp` diagnostics for
  payload enum crossing awaits and prevent lowering from reaching LLVM.

## 5. Match/Try Diagnostic Parity

- [x] 5.1 Inventory remaining generic or phase-specific match/try diagnostics
  after the archived ergonomics change.
- [x] 5.2 Add negative tests before any new pattern, guard, propagation, or
  conversion relaxation.
- [x] 5.3 Require `sglsp` quick fixes only where the insertion or rewrite is
  unambiguous and omit them otherwise with a test.
- [x] 5.4 Do not add match guards, new pattern syntax, or implicit error
  conversion in this phase; record them as deferred if discovered.

## 6. Migration Gate For Breaking Cleanup

- [x] 6.1 Do not implement source-incompatible cleanup inside this change unless
  a migration document is added first.
- [x] 6.2 No source-incompatible cleanup was implemented, so no migration doc was
  required in this phase.
- [x] 6.3 Parent umbrella integration has no migration doc to accept for this
  additive/diagnostic-only change.

## 7. Verification

- [x] 7.1 `cargo test -p sengoo-compiler attribute`
- [x] 7.2 `cargo test -p sengoo-compiler ffi`
- [x] 7.3 `cargo test -p sengoo-compiler async_frame`
- [x] 7.4 `cargo test -p sengoo-compiler try_match`
- [x] 7.5 `cargo test -p sglsp diagnostics`
- [x] 7.6 `sgc check` / JSON diagnostic snapshots for representative accepted
  and rejected language forms.

## Archive Gate

- [x] `openspec validate language-default-polish --strict` passes.
- [x] All accepted relaxations have parser/typeck/lowering or runtime-shape
  coverage as applicable.
- [x] All still-rejected adjacent forms have negative tests.
- [x] `sglsp` diagnostic and quick-fix parity is proven for every new diagnostic
  code or warning.
- [x] Any source-incompatible cleanup has migration documentation accepted by
  the parent umbrella; no cleanup was implemented, so this is not applicable.
