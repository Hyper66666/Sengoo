# Proposed staged tasks — none implemented

All names beginning `verify_conditional_` below are proposed verifier names.
They do not refer to existing successful gates. No checkbox is completed by the
cause report or by this design draft.

## A. Contract and captured baseline

- [ ] Register/review one child OpenSpec change with semantics and native deltas;
  preserve edition-2026 and v0.2 compatibility boundaries.
- [ ] Pin cause report `9586ad...`, current implementation, canonical specs and
  exact ordinary gate baseline. Retain the original 5008 failure.
- [ ] Freeze initial named-struct domain, supported leaf glue, diagnostics for
  excluded paths, staged loop/reinitialization admission and opcode inventory.
- [ ] Decide schema-2 producer/consumer compatibility explicitly. Inventory
  constructors, direct MIR producers, validators, debug/source adapters, raw
  fixtures, callbacks and reported schema fields. Do not assume a MIR wire codec.
- [ ] Confirm whether existing complete HIR7 facts suffice without new meanings;
  version the HIR bridge separately if they do not.

## B. Typed path representation and branch cleanup

- [ ] Implement deterministic bounded drop-path tree construction, binding/local
  and generic field/ordinal translation, complete terminal partition and budgets.
- [ ] Implement path operations with exact reserved fields and pure validation;
  reject duplicate/orphan/cyclic/cross-function/wrong-type/missing-field records,
  pending glue and conflicting old/new flag state.
- [ ] Account for every relevant checked action, including negative temporary
  roots, rather than retaining the existing skip of projected flag updates.
- [ ] Add native private flag slots, prologue zeroing, guarded path address/drop,
  exact reverse order and drop-scratch reservation.
- [ ] Run proposed `verify_conditional_field_paths.py`: direct HIR, independent
  clone and wire lowering; true/false/nested diamonds; only-left/only-right/both/
  neither moves; parent-subtree moves; owned parameters/temporaries; normal,
  early return and both propagation exits.
- [ ] Run structural poison controls for every new record/operation word and
  separate source-mapping controls for compatible sibling swaps. Check first-call
  and repeated compiler-resource recovery and exhausted budgets.
- [ ] Until subsequent gates pass, reject unsupported combined replacement or
  loop cases explicitly, while preserving all old non-path behavior.

## C. Replacement, aggregate reinitialization and loops

- [ ] Establish RHS-before-drop/store-before-flag ordering and temporary ownership
  for initialized/uninitialized/conditionally moved field replacement.
- [ ] Support whole aggregate replacement after a partial move, field-only
  reinitialization, nested replacement, repeated replacement and independent
  sibling state without stale root/descendant flags.
- [ ] Preserve action-10 zero-disarm behavior; nonzero/stale/wrong-field/disarm
  after conditional move controls retain rejection. Keep unknown observer and
  consuming-call contracts conservative.
- [ ] Review the current loop replay's path and action coverage. Prove the
  admitted domain or implement a bounded fixed point with its own fact tests.
- [ ] Run proposed `verify_conditional_field_reinit.py`: zero/one/many iterations,
  alternating moves, move+reinitialize, nested loops, break/continue, early
  return/propagation and parent replacement. Every path has an exact release log.
- [ ] Retain use-after-partial-move, moved-whole-read, borrowed-place move,
  custom-destructor ancestor and tagged/indexed partial-move negative cases.

## D. Independent native ABI and resource gates

- [ ] Run proposed `verify_conditional_field_native.py` with a real C allocation
  registry and SG destructors; record per-identity release count/order, writes,
  result validity and exact allocation/live-byte baseline at every scope.
- [ ] Test nested structs, concrete generics, scalar padding and aggregate sizes
  crossing the existing direct/indirect parameter and return thresholds; mix
  integer/float arguments and sret. Compare independently compiled C layout.
- [ ] Test large flag sets crossing one-page stack probing; inspect actual code,
  relocations, frame bounds and `RtlVirtualUnwind` behavior; verify skipped Drop
  makes no invalid field load/call. Do not claim exception cleanup.
- [ ] Exercise existing typed Drop nesting/pending-glue poison controls, callback
  ABI and collection-owned elements. A custom destructor plus automatic fields
  must neither run twice nor bypass a pending field-inactivation obligation.
- [ ] Run proposed `verify_conditional_string_unwrap.py` on real canonical source
  and actual runtime implementation, with delegating counters, true/false cases,
  live/zero placeholders, Unicode, independent clones, clone failure and repeated
  exact resource scopes. Freeze any remaining leak as a failed gate.
- [ ] If `unwrap_or` fails resources, open a separately reviewed source/observer
  decision with actual provenance and runtime evidence. Do not change the legacy
  move rule or mark library correctness on object/main42 success.

## E. Joint integration and honest checkpoint

- [ ] Run the existing MIR/borrow/projection/handle-disarm and native ordinary
  gates, plus callbacks, collection effects and relevant module/sourceflow gates.
- [ ] Preserve validation of every required body; rerun `import std::file` and
  actual callable use, while keeping compilation and resource claims separate.
- [ ] Verify physical source spans, previous-output preservation, repeated object
  bytes and first-call/repeated String/Buffer/opaque/snapshot/OS-resource recovery
  where the corresponding gate owns those resources.
- [ ] Audit all original/retained/tool/header/config/log/artifact hashes and
  nested receipt pins; record full revision, dirty-source snapshot and host.
- [ ] Validate the registered child and all OpenSpec changes strictly, then
  update only the capabilities established by named terminal reports.
- [ ] Keep full M3/M4, unqualified path forms, final SDK and whole compiler
  self-build open until their independent requirements are met.
