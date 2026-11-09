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

## HIR schema 2, ownership ABI and concrete generic checkpoint

- [x] Preserve per-node callable signatures in HIR schema 2; reject the named same-signature callee/target mismatch and cross-function cleanup-binding corruptions.
- [x] Preserve the explicit observed-parameter contract through HIR encoding/decoding and MIR lowering, including transitive wrappers and by-value method receivers, with no callee Drop or caller-owner move and one caller cleanup.
- [x] Validate the recorded observed-parameter certificate against the resolved body without rerunning semantic/ownership analysis or replacing the supplied facts; reject the named forged-contract case.
- [x] Emit concrete generic function instances with separate locals/scopes/CFG, structural signature reuse and explicit template/substitution metadata; verify multiple types, repeated calls, recursion, nested generic calls and generic field substitution in the named fixture subset.
- [x] Reject the named malformed instance metadata and explicitly reject ownership-sensitive generic actuals and generic foreign declarations with `5009` pending their complete lowering contract.

The earlier poisoned-call discovery is now rejected by its independent
regression. The original exit-86 discovery log remains historical evidence;
it is not relabeled as a pass.

`selfhost/verify_mir.py` is the reproducible gate. It verifies seed and clang
identity/version before execution, invalidates stale passed reports, records
the complete revision/host/Python identity, bounds process time, and verifies
the complete declared frontend import closure plus stdlib/runtime, both test
programs and the runner before and after execution. Each `selfhost_` import
must belong to that declared closure. Independent native backend source files
are outside this frontend test's input graph; earlier reports which guarded
those sibling files and observed a change remain failed trial reports.

Final evidence: `target/selfhost/m3-mir-contracts/evidence.json`, SHA-256
`ddb676d01657c1555b554419a2159ad3e746c54b5b10b6c388625770af4bcecd`.
Both starting and ending revisions are
`9a62b4e2c24d9610451b18915dea5629ce356b9e`, on Windows 11 AMD64, with the
verified development seed, clang, source-development runtime and O0. All 175
input records were unchanged; `source_changes` and `tool_changes` are empty.
The forced borrow build/native run and forced MIR build/native run all exited
zero (22.60 s, 7.15 s, 31.04 s and 1.60 s). Ownership cases, lifetime/action
checks and contract checks ran fifty resource rounds. HIR schema-2 roundtrip,
poisoned-call/owner/contract rejection, observed-parameter ABI, five generic
fixtures, 21 source fixtures and five exact cleanup fixtures ran ten rounds;
raw CFG/SSA/memory/ABI/limit cases ran thirty rounds. String/Buffer/opaque
resources recovered exactly after each applicable round. Test executable
SHA-256 values are
`b782067545aebcd539e3df1bc01e007ddd22ed65bc9d99e46c5641c69ed25664`
(ownership) and
`f09452607585423ffb686af4175c94919436f4cc8766ffbcd368ba588a849294`
(HIR/MIR).

MIR schema 1 adds `instances` records with concrete function, template symbol,
template signature and a range of type substitutions. It emits no abstract
template function body. The ordinary function/local/block/instruction table
widths and runtime ABI remain unchanged. Complete ownership-aware generic
lowering, owned pattern/variant cleanup, inherited/dynamic methods, builtin
callable facts, error propagation, closures/contracts/async/concurrency and
cancellation remain open, as does full supported-corpus parity.
`m3_complete` remains `false`; the complete M3 tasks above stay unchecked.

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

## Numeric/callable/Future semantic checkpoint

- [x] Verify 124 named semantic fixtures and 100 exact resource scopes for explicit numeric widths, call signature facts, method arity/ambiguity and the implemented Future/dynamic-trait cases (`target/selfhost/m3-semantic/evidence.json`).

Report SHA-256:
`04f7661301da5a9220b12113732b14911c6e49872c59389669cf613a7c010f78`.
The report records identical start/end revision
`9a62b4e2c24d9610451b18915dea5629ce356b9e`, Windows 11 AMD64, unchanged
source/tool identities and a fresh 49-case comparison with the same five
reviewed differences. The preceding 107-fixture report is preserved as
`target/selfhost/m3-semantic/evidence-107-cbe289d.json`.

This is a partial regression checkpoint. Subsequent full-corpus discovery
identified an overly broad rejection of direct Future local bindings; those
are supported by the reference and must be corrected. The discovery counts
are not parity passes, and the rejected local bindings are not approved
soundness exceptions. Complete dynamic/operator/Future/ownership coverage and
the remaining M3 gate stay open.

The final committed-source preparation removed one trailing whitespace span in
`mir_lower.sg`; the full identical runner was rerun after that edit. Final
checkpoint evidence is `target/selfhost/m3-mir-checkpoint/evidence.json`, SHA-256
`6b88ffa5ea14244db12dab8ef032d43060959682b5476bb2670772f8c026d546`,
revision `94946f71f133e0ec28fd9bcf725e69dec45d9173`, Windows 11 AMD64.
All four build/native commands passed, with empty source/tool change lists.
This final run supersedes the earlier `ddb676...` source snapshot for the
whitespace-normalized source; earlier reports remain preserved.
