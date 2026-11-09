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
At that foundation checkpoint the path lowered `ShChecked` typed AST directly
into MIR. A complete HIR
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

## Typed HIR and expanded MIR checkpoint

- [x] Add independently owned HIR schema 1 with source text, the initial syntax-DAG node shape, resolved type/symbol/binding/scope tables and explicit ownership facts.
- [x] Encode and decode the HIR word bridge without native handles or Rust memory layout, preserving Unicode text and rejecting the tested version, trailing-data, capacity, type and resolution corruptions.
- [x] Compare decoded-HIR lowering against direct lowering word-for-word across all MIR tables and type metadata without rerunning semantic analysis during decode or `sh_lower_hir`.
- [x] Verify the named enum construction/payload match, scalar guard/tuple match, range/array/tuple iteration, break/continue and inherent receiver-method fixture subset.
- [x] Verify complete-expression temporary cleanup, borrowed temporary/alias lifetime, owned parameter initialization and overwritten-owner cleanup in the five named cleanup-shape fixtures.

Evidence: `target/selfhost/m3-mir/extended/evidence.json`, SHA-256
`1d5dbdcf22684e3b336d5e70f847880c32cf85d62260b9ef940358035b0be8e9`.
The source revision is `8182d7931ab882963c4fe583201f16d07ac50531` on Windows
11 AMD64. The verified development seed and clang used the source-development
runtime at O0. All 174 input records, including the evidence runner itself,
remained identical; `source_changes` and `tool_changes` are empty. The build
and native test exited zero in 29.41 s and 1.27 s. Ten rounds each ran 21 source
acceptance/rejection fixtures, five exact cleanup-shape fixtures, and Unicode
HIR bridge roundtrip/corruption/direct-lowering checks. Thirty additional
rounds exercised raw MIR CFG/SSA/memory/ABI/limit negatives. String, Buffer and
opaque resource counts recovered exactly after every round. The produced test
executable SHA-256 is
`ada756393cf0d294172f7c88f2c082af462cb92f20d4c9d1c004eaf4a496ac61`.

`m3_complete` remains `false`. This HIR intentionally retains the initial AST
node structure; comprehensive HIR desugaring and malformed-input/cross-scope
ownership coverage remain part of the open M3 gate. Generic monomorphization,
owned patterns with variant-aware and conditional field cleanup, the complete
pattern/range/slice and inherited/dynamic-method subset, closures/contracts/
error propagation/async/concurrency/cancellation, and complete frozen-corpus
parity/native-consumer integration remain unproven. The native consumer has
its own evidence and is not implicitly qualified by these frontend tests.

- [ ] Reject a HIR call whose target binding disagrees with its resolved callee even when both function signatures match; add independent cross-callable ownership-fact corruption checks.

The follow-up discovery probe
`target/selfhost/m3-mir/extended/poison-call.sg` deliberately changes only a
call node's binding to another function with the same signature. The current
validator returned zero and the probe exited 86 (`poison-call-native.log`).
This is a recorded validation gap, not a passing negative case or semantic
parity result. The typed-HIR checkpoint above remains limited to its named
positive/negative fixtures, and this gap must close before the complete HIR
validation/M3 gate can be checked off.

## Additional semantic and ownership checkpoint

- [x] Verify the named 107 semantic fixtures and 100 exact resource scopes for numeric inference, associated projection normalization, callable/closure owner identities, core trait/FFI contracts and inherited class field facts (`target/selfhost/m3-semantic/evidence.json`).
- [x] Verify 42 ownership fixtures and 50 repeated lifecycle/resource scopes for temporary owners, field moves, alias retention, conditional construction, overwrite cleanup and authorized parent/child reborrows (`target/selfhost/m3-borrow/reborrow-evidence.json`).

The semantic report has SHA-256
`5dfa47ba2d41d241dab3455f5119448d142518d3f6e852d244d7983fe1d98804`;
its result is explicitly `foundation-pass-with-reference-differences`. The
fresh 49-case reference comparison retains five approved ledger differences.
The ownership report has SHA-256
`c5c7dfaf9c48e983bf662285481cada058ba25375f730800d8ad6003497f4921`.
Both have empty source-change lists and record their actual source revision,
host, tool identities and commands. Broader positive/negative corpus discovery
reports remain discovery evidence, not passes or M3 completion. In particular,
method candidate arity, unsuffixed literal contexts, dynamic/operator/Future
contracts and the other open semantic/ownership obligations still need work.
The semantic replay records identical start/end revision
`8182d7931ab882963c4fe583201f16d07ac50531` and empty tool/source changes.
The earlier 107-case report without revision/host metadata is retained as
`evidence-107-before-revision-host.json`; the replay supersedes its provenance.
