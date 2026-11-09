## Foundations

- [ ] Implement checked deterministic structural type identity and qualifier/argument preservation.
- [ ] Verify type arena limits, invalid child IDs, duplicate reuse and exact resource recovery.
- [ ] Implement shared module/scope/symbol resolution and located structured diagnostics.

## Supported semantics

- [ ] Implement complete supported declaration/expression/type checking and numeric rules.
- [ ] Implement generic constraints, trait/impl and method resolution, substitutions and specialization identity.
- [ ] Implement documented borrowing, moves, temporary ownership, Drop and control-flow joins.
- [ ] Verify closures, contracts, FFI, async/futures, concurrency and cancellation boundaries.
- [ ] Add a general temporary owning-result field-projection lifetime regression.

## Typed lowering and integration

- [ ] Implement SG HIR and typed MIR with checked places/blocks/calls/aggregates and cleanup.
- [ ] Differentially verify supported semantic and rejection corpus using exact inputs and diagnostics.
- [ ] Verify LLVM-reference/native consumers against the same typed semantics when available.
- [ ] Record full revision/host/inputs/commands/results, validate this change and all specs strictly.
- [ ] Close M3 only after every supported semantic/lowering gate is proven; retain native self-build gates separately.

## Partial MIR foundation evidence

- [x] Implement MIR schema 1 with explicit function/local/scope/block/value/terminator identities, byte spans, type/layout references and SG/C call ABI, targeting Windows x86-64 and runtime ABI 1.
- [x] Verify the fixture subset through SG parsing, semantic analysis, ownership analysis and typed MIR construction, including explicit root and field cleanup and owned ABI parameter initialization.
- [x] Verify named malformed-MIR cases for invalid IDs/types, open CFG, edge argument mismatch, invalid SSA dominance/order/cross-function use, duplicate ABI parameters, wrong call/drop types, wrong root-drop local provenance, orphan field rows and capacity/resource recovery.

This is a foundation checkpoint; `m3_complete` remains `false`. The complete
typed-lowering task and the supported-semantics tasks above remain unchecked.
The current path lowers `ShChecked` typed AST directly into MIR. A complete HIR
layer/version bridge, generic specialization, methods, enum/pattern/match/for
lowering, closures/contracts/error propagation/async/concurrency/cancellation,
temporary-owner facts, conditional partial-field cleanup, physical aggregate
size/alignment/member offsets, and a native MIR consumer remain unproven.

Evidence: `target/selfhost/m3-mir/evidence.json`, SHA-256
`2e2694b8d147760fd435c263769d72a706f56299d74f632684e02024becaf627`.
The report records revision
`ca3a163e361619b17e5829f26ac2575ac031e89d`, Windows 11 AMD64, verified
development seed/clang identities, source-development runtime and O0. Its 172
input files and tool identities were identical before and after both commands;
`source_changes` and `tool_changes` are empty. Native verification passed ten
rounds of typed source fixtures and three exact cleanup-shape cases, plus thirty
rounds of raw CFG/SSA/memory/ABI/limit cases, with exact String/Buffer/opaque
resource counts. Owned parameter cleanup covers a branch that moves the owner
and a join that conditionally drops it. The raw negative cases explicitly reject
a root drop whose pointer belongs to another local of the same type and reject
orphan field rows with both invalid and valid type IDs.

The compiler build and native test exited zero (36.75 s and 0.55 s). Raw logs
are `foundation-build.log` and `foundation-native.log` beside the report. The
earlier parser-input-change trials remain beside it as failed evidence, rather
than being relabeled as a stable source run. This evidence does not qualify a
release compiler, compare performance or demonstrate native self-compilation.
