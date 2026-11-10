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

## Physical module source mapping

- [x] Validate complete expansion and physical-file maps and stage original UTF-8 source spans before updating MIR metadata (`target/selfhost/m3-module-mir-expanded-final/evidence.json`).
- [x] Prove exact function and generated/specialized provenance, all local/instruction/terminator/block spans, and byte-identical native objects with real macro and attribute transformations (same report).
- [x] Reject malformed maps, cross-file intervals and source-count limits without changing any MIR state; verify first-call and repeated resource restoration with independent execution (same report).

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

## Class prefix layout and callable-fact checkpoint

- [x] Preserve class identity as MIR nominal data 2 and explicit direct-parent type records, with parent-first flattened field types and inherited-field ordinals.
- [x] Lower the named inherited shared/mutable receiver methods through class-prefix reference upcasts; validate parent identity, acyclic/unique parent records, field-type/offset prefixes and compatible size/alignment.
- [x] Reject the named unrelated class reference, shared-to-mutable upgrade, mismatched parent field layout/offset and cyclic-parent MIR mutations.
- [x] Give internal ADT/impl/trait methods distinct symbols within the current resolved module and verify the named two-ADT same-method-name fixture.
- [x] Include inherited owned fields and enum payload fields in needs-Drop analysis; verify repeated use after moving either aggregate is rejected, and inherited-field partial cleanup emits the two expected drops with one field projection.
- [x] Require complete builtin callable signatures in schema-2 HIR and roundtrip the named print/sleep builtin facts while rejecting a removed callable signature.

Evidence: `target/selfhost/m3-class/checkpoint/evidence.json`, SHA-256
`04b28fd57accda9583cab4dba45820df90aa349ecdad3468cad4a54463cbc819`.
Both starting and ending revisions are
`def09df340fffae2b0e32c8841b7d199f032e4da`, Windows 11 AMD64. All 175
declared frontend/stdlib/runtime/test/runner input records and verified tool
identities were unchanged. The forced ownership build/native and MIR
build/native commands exited zero (23.57 s, 7.39 s, 32.35 s, 7.05 s).
The ownership regressions ran fifty resource rounds; class, builtin-HIR,
generic, poisoned-image, observed-ABI and typed-source/cleanup regressions ran
ten rounds, with thirty raw MIR CFG/SSA/ABI rounds. String/Buffer/opaque counts
recovered exactly. Artifact SHA-256 values are
`951e27c73e9de613429a88203b4d380bfc9056d23e1f05da32f93b5d12d1bb17`
(ownership) and
`dcb3e21704e963e42fe9f02fea2255b46f90e51e6c20c8da5f525a0f452bc3af`
(HIR/MIR).

These upcasts implement the documented internal receiver adjustment only.
The independent qualified-reference oracle rejects passing `&Leaf` directly
to a free function expecting `&Base`; that source rejection remains
`5104`. The qualified reference also rejects generic arguments in the class
inheritance path (`Parent<T>`, expected LBrace/found Lt). A named parser
`3001` regression retains that boundary; the positive generic class fixture
uses a legal generic class without that inheritance syntax.

Builtin signatures are HIR facts in this checkpoint; their existence does not
qualify builtin native lowering. Method symbol identity is unique only within
the current resolved module; a module-qualified source identity and cross-module
collision/ABI gates remain open under M7. Complete owned patterns and
variant-aware/conditional field cleanup, error propagation, closures/async and
concurrency/cancellation remain open. `m3_complete` remains `false`.

## Propagation, loop exits and explicit Drop glue checkpoint

- [x] Stop emitting counter-value uses in a for-loop advance block with no incoming edge; preserve strict SSA validation and verify unconditional break/return plus break/continue-branch regressions.
- [x] Lower the named Result/Option function propagation and try-block success/failure cases with explicit branches, container rebuilding and typed result values.
- [x] Record failure-only propagation cleanup as ownership action kind 7 at the question-mark AST node; preserve the success-path state and scope try-block failure cleanup to its own locals.
- [x] Roundtrip propagation cleanup through HIR schema 2 without rerunning analysis and reject a forged cleanup site.
- [x] Record nominal Drop glue as trivial, resolved user function, structural, tag-selected enum or pending; validate member-ownership proof, exact user Drop ABI/layout agreement, pending-state consistency and named metadata mutations.
- [x] Preserve the narrowly approved implicit-unit effect tail for class/impl methods by discarding the body's checked tail value and emitting a unit return.

Evidence: `target/selfhost/m3-try/checkpoint/evidence.json`, SHA-256
`2057465dc209cf303e09b9df54c2324ddfba72c9e9882f48a39d73fa99af5b3f`.
Both starting and ending revisions are
`07082b7b406229de6f76862247fcb7b2997d9210`, Windows 11 AMD64. All 175
frontend import-closure, stdlib/runtime, test and runner records remained
identical, as did the verified seed/clang identities. Forced ownership
build/native and MIR build/native commands all exited zero (25.90 s, 8.01 s,
39.59 s, 3.38 s). Ownership cases and the independent propagation-fact
assertions ran fifty rounds. Typed-source/cleanup, propagation HIR
roundtrip/corruption, Drop glue mutation, generic, builtin-HIR and prior
checkpoint tests ran ten rounds, with thirty raw MIR CFG/SSA/ABI rounds.
String/Buffer/opaque counts recovered exactly. Test executable hashes are
`d3170aaf752acf84dff8bd0c03cf92e46ae422a38d3bce193fccc4bf819ccb4f`
(ownership) and
`79dd3c5d21c1d247a000ebdc92c7150662a9836d1dd7495a45237c932d4e8295`
(HIR/MIR).

`ShMir.drop_glues` records are `type, kind, targetFunction`; kind 0 is
proven trivial, 1 is a resolved user Drop, 2 structural fields, 3 tag-selected
enum, and 4 pending. Layout `dropFunction=-2` records required unresolved
glue. Pending metadata never authorizes a free; native cleanup must reject
it while physical layout can still be computed. A recognized runtime-owner
name without a resolved source Drop implementation remains pending even if
it also contains owned fields. This checkpoint does not invent a runtime
deallocator from the type name. Resolved runtime ownership/Drop ABI and
complete generic Drop specialization remain open.

The inactive payload builder currently covers scalar and recursively
supported aggregate defaults. Reference/dyn/Future and other unsupported
inactive payloads reject rather than fabricate a value. A fresh qualified
reference check also rejects function propagation from `Option<bool>` into
`Option<i64>`; that source case retains `5104`, alongside a same-payload
Option failure positive. Result failure reconstruction with a different
success payload and unchanged error type is covered.

The original for-exit discovery (`for-exit-probe-native.log`) returned
`5005` for unconditional break and return; the fixed independent probe
returned zero for all three named sources. Neither those discovery logs nor
earlier partial reports are relabeled as final evidence. Complete owned
patterns, conditional field flags, resolved runtime resources,
closures/contracts/async/concurrency/cancellation and full supported-corpus
parity remain open. `m3_complete` remains `false`.

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

## Future local and dynamic association semantic checkpoint

- [x] Correct the supported direct Future local binding rule and verify the named Pending wakeup, Poll/AsyncContext layout/receiver and opaque-value contract cases.
- [x] Preserve fixed dynamic associated type bindings in explicit type roles; reject the tested unknown/missing/duplicate bindings and object-unsafe methods, and validate structural arena flags and arguments.

The 147-fixture semantic report is `target/selfhost/m3-semantic/evidence.json`,
SHA-256 `d39e95430808ffd35ff61ae7ac406d59465c60993e5df14a7726f483d1cdf1ec`.
It records identical start/end revision
`cb3277bcfdaf55ce10c04adcb053517f011fba70`, Windows 11 AMD64, unchanged
sources/tools, 100 semantic resource scopes, 100 structural-type resource
scopes, formatting/build/CLI checks and the fresh 49-case comparison retaining
the same five reviewed differences. The earlier overbroad Future-local
rejection is corrected in this snapshot; it is not an approved exception.

Full-corpus discovery still finds seven supported positive gaps and remaining
negative gaps; the typed-only discovery CLI does not yet apply borrowing.
Path function calls, remaining Hash/operator/contract/supertrait and pattern
rules and the complete M3 gate remain open. These discovery results are not
treated as a complete corpus pass.


## Qualified calls, operator and CLI ownership checkpoint

- [x] Verify 178 named semantic fixtures and 100 exact resource scopes, including qualified callable lookup, operator Rhs/Output and generic bounds, transitive supertraits, or-pattern bindings, contracts, the scoped implicit-unit method tail rule and format template argument/specifier validation.
- [x] Run the semantic CLI through the real ownership analyzer after successful type checking, preserving located original-source diagnostics and phase identity.
- [x] Replay structural types and native semantic tests, formatting and native CLI build, plus 49 fresh standalone Rust type-check requests on the unchanged mapped source closure.

Report: `target/selfhost/m3-semantic/evidence.json`, SHA-256
`f0b7c4be515ce334f94c4781ec4a9cd2350ee36b9ab60bab06494ac396eb2614`.
It records identical start/end revision
`07082b7b406229de6f76862247fcb7b2997d9210`, Windows 11 AMD64, empty
source/tool change lists, 178 fixture outcomes and the same five reviewed
49-case differences. The previous 147-case report is preserved as
`target/selfhost/m3-semantic/evidence-147-e5a02ba.json`; the first 178-case run
whose HEAD and ownership source changed remains failed development evidence.

The complete fresh discovery still is not a parity pass: 770 standalone
Rust-accepted fixtures have 19 CLI rejections, including eight newly exposed
ownership compatibility gaps and the Hasher typed/lowering inconsistency.
718 Rust-rejected fixtures have four remaining ownership acceptances; the
oracle completed every request without faults. The six genuine positive gaps
from the previous checkpoint now pass through the ownership CLI. Fixture 625
remains a typed positive differential despite qualified reference MIR rejection;
its independent evidence is `hasher-625-inconsistency.json` beside the report.

The initial actual-source single-file discovery accepts only five of 96
selfhost/stdlib files and records all diagnostics. Its imports do not load
modules; this is an explicit missing capability, not a completed module graph.
Full shared-module resolution, standard-library/intrinsic coverage and all
remaining semantic, ownership and lowering obligations remain unchecked.

## Unsafe byte movement, nested destructors and reborrow checkpoint

- [x] Define opcode 22 as unsafe overlap-safe byte movement with a u64/usize byte count, equal-pointee raw pointers, mutable destination, unit result and fixed reserved metadata; reject signed counts, reference operands, immutable destinations, mismatched pointees and missing unsafe flags.
- [x] Mark a source destructor that explicitly or transitively invokes another destructor as pending glue kind four until field-inactivation facts exist; reject forged resolved glue and legacy layout metadata that would permit nested or projected destruction followed by recursive cleanup.
- [x] Preserve exclusive loans in returned aggregates containing mutable references and reject moved-field reads in unreachable tails without manufacturing runtime cleanup actions.
- [x] Authorize method receiver and explicit receiver reborrows through the existing loan parent, preserving same-field conflict diagnostics and disjoint-field acceptance; retain borrowed scalar handle projections as reads.
- [x] Replay the seven named canonical positive fixtures 300/741/802/810/812/815/1200, the three negative reproductions 507/768/769 and independent conflict/reinitialization cases inside the native ownership resource gate.

The final report is
`target/selfhost/m3-memory-drop/final-checkpoint/evidence.json`, SHA-256
`7d52495abec4a2ac77fee371e6f3bb72aafed075c918588714689a4f36dbf28d`.
It records identical start/end revision
`94c7a24fc99053c220fb6c4d36849de6451c9eea`, Windows 11 AMD64,
Python 3.12.14, 175 mapped input hashes and empty source/tool change lists.
All four build/native commands passed: 50 ownership rounds, 10 typed HIR/MIR
roundtrip/lowering/cleanup rounds and 30 raw CFG/SSA/ABI/byte-movement rounds
with exact String, Buffer and opaque handle counts. The earlier failed
`checkpoint` and `receiver-checkpoint` reports are preserved; they exposed the
field receiver rejection and the incorrect class implicit-unit test fixture.
The corrected positive fixture uses struct/impl, matching the frozen reference;
class effect tails retain the semantic rejection rule.

This is a partial correctness checkpoint, with `m3_complete: false`.
Opcode 22's runtime byte extents remain an unsafe producer obligation and its
native implementation is a separate backend gate. Pending destructor glue does
not authorize any actual free. Complete closure/Send, owned-pattern and
conditional field cleanup, projected-only wrapper ABI specialization, runtime
drop ABI, async/cancellation and full supported-corpus parity remain open.

## Arithmetic calls and deferred typed HIR checkpoint

- [x] Carry owned deferred associated-projection equalities in HIR schema three, including complete bridge encoding/decoding, caps and callable/type/origin/outer-shape validation; reject schema two and malformed or poisoned records without repeating semantic analysis.
- [x] Close every concrete callable's deferred equalities after generic substitution and projection normalization, rejecting unresolved or unequal instances with 5009; verify Source/Iter/Take through semantic checking, HIR roundtrip and concrete MIR lowering.
- [x] Lower checked arithmetic to actual user operator methods, including concrete aggregate output, generic ADT and primitive dispatch, Neg and borrowed receivers; validate operator signatures and implementation identities in saved HIR.
- [x] Select concrete generic trait method implementations from typed constraints instead of executing abstract trait bodies; align ownership analysis with semantic analysis by omitting execution facts for those abstract bodies.
- [x] Use a visited worklist for source destructor call traversal, preserving ordinary mutually recursive helpers without authorizing nested field destruction.
- [x] Preserve initialized ancestor state when a field path first appears in a branch or loop, refresh descendants on complete reinitialization and retain rejection of conditional moves; discard typed unit branch tails while preserving their effects.

Report: `target/selfhost/m3-operator-deferred/final-checkpoint/evidence.json`,
SHA-256 `d8044cb6de4f411c4a5605c90a4bebb8f2921d929715c9c46cfbb90a407b2645`.
It records Windows 11 AMD64, Python 3.12.14, 175 exact input hashes and empty
source/tool change lists. The start revision is
`66043a401c96350bfdd5fed2a4b1326477a52f82`; the end revision is
`c32037ff9941ae3f10094c5b17dd8909fd63a698`, after an independent native-backend
commit whose files are outside this gate's mapped frontend closure. All four
build/native commands passed with 50 ownership rounds, 10 typed HIR/MIR
roundtrip/lowering/cleanup rounds and 30 raw CFG/SSA/ABI rounds, with exact
String, Buffer and opaque handle counts. Earlier failed trial/checkpoint
reports remain preserved, including the initialized-field false positive and
the source compiler's readonly Vec getter API mismatch in the first fix.

This is partial correctness evidence, with `m3_complete: false`. Fresh full
corpus discovery still exposes a borrowed temporary escaping through an
anonymous call result (negative fixture 803), which this named checkpoint
does not cover. Complete borrowed-result lifetime tracking, projected-only
wrapper ABI specialization, runtime contracts, closure environments, owned
patterns, runtime drop ABI, conditional field cleanup, async/cancellation and
the full supported corpus remain open. No compiler self-build or performance
qualification is claimed by this gate.

## Anonymous result lifetime checkpoint

- [x] Reject borrowed anonymous call results that escape local or temporary storage, including negative corpus fixture 803, explicit returns, branch returns and local aliases of temporary storage.
- [x] Track addressable temporary storage independently of Drop obligations, preserving full-expression uses, parameter-derived returns and local alias lifetime extension; retain accurate MIR local Drop flags for storage-only temporaries.
- [x] Bind the corrected synthetic String.as_str shared receiver signature, including owned/shared/mutable source receiver forms, to the real loan path rather than consuming an owned synthetic parameter.
- [x] Reject requires/ensures-bearing callables with 5008 until runtime contract lowering exists; preserve generic where-clause lowering.

Report: `target/selfhost/m3-borrow-escape/checkpoint/evidence.json`, SHA-256
`9fdd33c4b83977b3107b5de7c0203643b6ae4a91bdc8ac1a7a6ba24fed3b408a`.
It records identical start/end revision
`f1967f2f3167d3ef802f6dff17b2fcaa8d558d9f`, Windows 11 AMD64,
Python 3.12.14, 175 input hashes and empty source/tool change lists. All four
build/native commands passed with the eight new temporary-lifetime cases,
50 ownership rounds, 10 HIR schema-three roundtrip/lowering/cleanup rounds and
30 raw MIR rounds, with exact String, Buffer and opaque handle counts.
The development failures remain preserved, including the synthetic receiver
fact error and the initial overly broad contract-node check that also matched
where clauses. The final check selects requires/ensures payloads explicitly.

This closes the named 803 lifetime reproduction, not the full borrowed-result
or standard-library corpus. Projected-only wrapper specialization, generic
ownership summaries, explicit handle transfer/inactivation, executable runtime
contracts, closure environments, owned patterns, runtime drop ABI, conditional
field cleanup and async/cancellation remain open, with `m3_complete: false`.


## Physical module graph and class-tail correction checkpoint

- [x] Verify the named physical-identity alias, dependency-first diamond graph, relative/mapped Unicode paths, cycle and missing import, malformed module map, module/byte limits, invalid UTF-8, standard import and cross-file diagnostic cases through SG native execution.
- [x] Retain exact String/Buffer/opaque/FileSnapshot handle and snapshot-byte resource recovery across ten complete graph/check lifecycles.
- [x] Preserve node/binding/scope ModuleIds, explicit module scope/export rows and original SourceId byte maps in the owning module result; route the semantic CLI through real local/stdlib dependency loading.
- [x] Correct the class boolean-tail acceptance: only impl methods use the implicit-unit discard rule; class methods remain strict.

Source-bound report: `target/selfhost/m3-semantic/evidence.json`, SHA-256
`af1458175c50bf61f7495fc2faec9ae468cc35cf61a303afeecc23a7bea23bec`.
The full start/end revision is
`94c7a24fc99053c220fb6c4d36849de6451c9eea`; source/tool changes are empty.
The Windows 11 AMD64 replay includes 179 semantic fixtures, 100 semantic and
100 structural type resource scopes, ten native module lifecycle/resource
rounds, all mapped module source/fixture hashes, native CLI build and a fresh
49-case Rust comparison with the same five reviewed differences. Earlier
178-case evidence is preserved separately.

The module-aware actual entry discovery remains partial: std::assert,
std::option and std::result entry wrappers pass; std::string has ownership gaps,
and core/lexer/semantic still hit the located collections typing obligations.
The module provenance wrapper is not yet serialized through HIR schema 2.
Complete module-aware typing, stdlib intrinsic/specialization coverage,
ownership compatibility and self-compilation gates remain open.


Independent qualified reference check: all ten module/import cases agree on
acceptance and rejection with the frozen source-qualified release sgc
`c70400a9cae30edc666dcccfdfc867155875853b` (binary SHA-256
`c07e02be19ebb82a33927851dab6224f444477f0b8dc0aab93f1c83adbdde279`).
Report `target/selfhost/m3-semantic/module-reference-evidence.json` has SHA-256
`8f16d0edf0418a9a67dd5d22e91b260d69ff04e40bd0d216e09be26b93e14e5e` and identical actual start/end revision
`94c7a24fc99053c220fb6c4d36849de6451c9eea`. It includes source/module-map inputs and complete
qualified-reference/candidate outputs for each case. This evidence proves only
the named physical import cases, not full namespace or stdlib compatibility.


## Generic association and impl effect-context checkpoint

- [x] Retain bounded per-callable deferred associated-type equalities with original AST provenance; verify concrete correctness/rejection and forward/generic-wrapper calls instead of treating projections as wildcards.
- [x] Verify the reference-compatible impl discarded-expression context while preserving ordinary/class/value-context and explicit-return checks.
- [x] Apply the existing reserved blocking-spawn Send capture constraint to resolved lambda captures and reject the named non-Send wrapper case.
- [x] Verify 188 named semantic cases plus deferred-table ownership/getter bounds, 100 semantic and structural-type resource scopes, native module lifecycle resources, native CLI and a fresh 49-case comparison.

The source-bound report `target/selfhost/m3-semantic/evidence.json` has SHA-256
`0cfec7e77bc1af8f408bdac686ef4a3bc9704e544983d449f70205b1b771bf16`.
It records identical start/end revision
`66043a401c96350bfdd5fed2a4b1326477a52f82`, Windows 11 AMD64 and empty
source/tool changes, retaining the same five reviewed fresh-49 differences.
The earlier 179-case report is preserved as
`target/selfhost/m3-semantic/evidence-179-392f122.json`.

Actual module-entry development trials for std::collections, core and lexer
now pass typing and reach ownership diagnostics. They do not pass ownership,
MIR lowering or self-compilation; unspecialized generic equalities remain
explicit obligations for concrete jobs. HIR schema 3 preservation and MIR
strict concrete-job checks have a separate coordinated gate. The first
large-corpus discovery rerun exceeded its old 20-second per-case deadline
because imports are now genuinely analyzed. Those aborted runs remain
incomplete development evidence; the replacement scanner saves every case,
uses a bounded 120-second deadline and records faults separately from rejection.


## Borrowed receiver facts and physical source BOM checkpoint

- [x] Give synthetic String.as_str a shared &String receiver signature for owned, shared and mutable receiver inputs, and verify the three callable facts with exact resources.
- [x] Preserve leading BOM bytes in each physical snapshot while masking only the three leading bytes during module merge; verify original byte mapping and reject an interior BOM.
- [x] Rebuild and execute the unchanged final semantic/type/module/CLI snapshot after the independent commit window, retaining historical revision-changing and artifact-replay evidence rather than rewriting it.

Final source-bound report: `target/selfhost/m3-semantic/evidence.json`, SHA-256
`619956dc189c081df082d2af288b5674ab62ef5b76c338e020b11bd945754973`.
It records identical start/end revision
`59f1c9c601456b083e4b36cb05b3a6e8ad014e9c`, Windows 11 AMD64, unchanged
source/tool identities, all 188 semantic fixtures plus receiver-fact assertions,
100 semantic/type resource scopes, ten module resource cycles, native CLI build
and the fresh 49-case comparison retaining the five reviewed differences.
The coordinated ownership/MIR gate separately verifies anonymous borrowed-view
escape rejection, no-Drop storage lifetimes and valid full-expression observation.

The eleven physical module cases, including a BOM-bearing imported source,
match qualified reference acceptance/rejection. Independent report
`target/selfhost/m3-semantic/module-reference-11case-evidence.json` has SHA-256
`eb444b105bf5886d6e7bae8a943c77f1b4ef1a444cb983bd9c1e994a4c59675e`.
The earlier ten-case report remains preserved separately. Large module-aware
corpus discovery is still classified by phase and missing dependency context;
its development results do not close M3 or establish complete stdlib/async support.


## Intrinsic authorization and ordinary Future annotation checkpoint

- [x] Add explicit six-word intrinsic call facts separating the source call, kind, receiver nominal type, real runtime declaration, language signature and C ABI signature.
- [x] Verify ABI-one String.as_str declaration/layout requirements, missing-declaration non-authorization, wrong parameter/return/layout/non-C rejection, safe-reference cast rejection and six-field corruption checks with an unchanged arena during validation.
- [x] Preserve ordinary Future<T> structural identity separately from Future trait-bound/impl roles; verify unused parameters, annotated local await and arity rejection without claiming runtime-wrapper implementation.
- [x] Verify the native String-view ABI bridge against the real runtime and independently checked MIR authorization; keep this consumer gate separate from native execution of the checker tests.

Source-bound report `target/selfhost/m3-semantic/evidence.json` has SHA-256
`76b068ba127a0fae7aa7194aeae2f14350c0f8d672b0262c49e6168e5b92778b`.
It records 193 literal semantic fixtures plus intrinsic field-corruption and
arena-stability checks, 100 exact semantic/type resource scopes, ten module
cycles, native CLI build and a fresh 49-case comparison retaining the same five
reviewed differences. Actual start/end revisions are
`190ef7ca9fb26656f0a2be4e9adf1c60b6b787c2` and
`35b163db0bb259fe74e4f786d74f084f9cdc51e1`. The report explicitly records
that revision interval; all mapped frontend, compiler/stdlib source, design/spec
and tool hashes were unchanged. Independent commits outside that complete input
closure do not invalidate an otherwise identical build snapshot. The earlier
changing-ownership-source run remains failed development evidence.

The ordinary annotated-local Future regression corrects the reference's missing
Future/Future structural-unification arm, rather than asserting zero differential
mismatches on that spelling. Actual runtime start handles, language Future
signatures, polling/result/cancellation/drop lifecycle origins and ABI bridges
remain distinct open obligations. Current selfhost semantic-module discovery
passes typing and still reaches the ownership diagnostic cap; this checkpoint
is not complete M3, native async support or self-compilation.

## Projected parameter and readonly runtime pointer checkpoint

- [x] Separate observed and projected-only parameter summaries, retain conservative default transfers and authorize a projected result only inside a proved noescape complete-expression context.
- [x] Serialize projected-parameter certificate kind nine, suppress the callee's owning parameter Drop on both sides of the call contract and independently infer certificates from an empty state to reject forged recursive authorization.
- [x] Preserve intrinsic six-word facts in HIR schema four with exact language/C signatures, pure receiver/layout/declaration validation, unique records and corruption/old-version rejection.
- [x] Lower authorized String.as_str through the unchanged declared C i64-to-i64 function and a dedicated MIR opcode-26 borrowed pointer bridge; reject wrong kinds, flags, C targets, call provenance, receivers and ordinary integer-to-safe-reference casts.
- [x] Permit only same-pointee mutable-to-shared reference qualification through the existing validated reference-coercion operation, preserving rejection of mutability upgrades.
- [x] Verify source-wrapper projection, native pointer projection and readonly UTF-8 pointer bridging through independent SG-object and frozen Rust/LLVM paths, each with 100 owner scopes, two observations per scope, exactly one release and a zero live count.

The frontend report is
`target/selfhost/m3-projection-contract/frontend-checkpoint/evidence.json`,
SHA-256 `82a201538c208e8050f4f486a27ae34e5fe2de111e42b60079c6e9af0966aefb`.
It records 175 unchanged input hashes, empty tool changes, 50 ownership rounds,
10 HIR-four/lowering/cleanup rounds and 30 raw MIR rounds with exact String,
Buffer and opaque handle counts. Actual revisions are
`190ef7ca9fb26656f0a2be4e9adf1c60b6b787c2` through
`35b163db0bb259fe74e4f786d74f084f9cdc51e1`; independent native commits outside
this frontend closure are recorded without altering the input identities.

The independent executable report is
`target/selfhost/m3-projection-contract/final-checkpoint/evidence.json`,
SHA-256 `eff28a24103587239f2365b6ca4ef64378d0afd0b775d3f83ef036411c2fe3a3`.
It records identical start/end revision
`35b163db0bb259fe74e4f786d74f084f9cdc51e1`, Windows 11 AMD64,
Python 3.12.14, 193 input hashes and empty source/tool changes. All 22 commands
passed: one SG compiler build and seven steps for each of three exact source
programs. Both object emissions are byte-identical for each source. All six
candidate/reference executables returned 42; C independently checks live,
created, released and observation counts before each release, and validates
UTF-8 view bytes. The C probe implements the declared reserved ABI contracts;
integration with actual registered runtime String allocations remains a
separate runtime gate and does not follow from these probe results.

The earlier unstrengthened C-probe report remains preserved. The development
resource test also exposed a genuine narrow-SSA load bug: a one-byte false
block parameter was read as an uninitialized qword. Its independent backend
fix and dirty-stack negative control are recorded by the native backend gates;
the resource OR assertion was retained unchanged for this final pass.

This checkpoint closes the named projected-wrapper reproduction 807 and the
three declared probe programs, with `m3_complete: false`. Full standard-library
ownership parity, pointer aliases outside noescape contexts, generic ownership,
real runtime allocation/drop integration, executable contracts, closure
environments, owned patterns, conditional field cleanup and async/cancellation
remain open. It does not qualify performance or native self-compilation.

## Registered runtime String bridge integration

- [x] Link the exact successful pointer-bridge producer object and frozen-reference LLVM IR against freshly compiled runtime.c and runtime_string.c, preserving their artifact/source/tool identities.
- [x] Allocate registered Strings through the real from_bytes_copy entry, observe the real as_str_ptr UTF-8 storage twice and release through real free_status exactly once per scope; check actual String, Buffer and opaque live counts return to zero over 100 scopes in both executions.

Report: `target/selfhost/m3-string-runtime/checkpoint/evidence.json`, SHA-256
`8fd3e9def047fe1fcb122ac399bb39b9138162cf9e52098d2a5b5b26070a3d7e`.
It records identical start/end revision
`3ad0189ecf083f0a08bea58d08b4ed0fd5193b83`, 119 unchanged input hashes,
empty tool changes and six successful compile/link/execute commands. Both
candidate and reference programs returned 42. The dependency is the exact
`eff28a...` producer report, with matching source, COFF object, LLVM IR and
qualified tool hashes; this gate does not claim to rebuild that frontend.

This narrow real-runtime result supersedes the pending registered-String
integration item in the earlier probe checkpoint. It does not close the full
runtime SDK, standard-library body/lowering or generic ownership obligations.
Actual profile-nine discovery still reports nine String-entry ownership
diagnostics and 245 core-entry diagnostics, preserved separately by phase.
Full M3, native self-compilation and performance/cutover gates remain open.

## Future declaration and polling regression checkpoint

- [x] Reject shared, owning and absent receivers on Future trait poll declarations before an impl exists, preserving located contract diagnostics.
- [x] Verify wakes on both branches, and reject optional-loop or uncalled-lambda wake evidence for a later pending Poll; retain the canonical ordered Poll await layout.
- [x] Run the source-bound 200-case native semantic suite, exact resource scopes, structural type/module/CLI gates and fresh 49-case comparison with all five reviewed differences preserved.
- [x] Independently compare eighteen polling cases against the current Rust type checker and frozen qualified compiler, preserving the unused-layout and shadow-context phase/contract differences rather than reporting zero mismatches.

The source-bound semantic report `target/selfhost/m3-semantic/evidence.json`
has SHA-256
`06ac4d6e93b5f180d5160b6a1133a68ade34fe543aad32b0e3af066b431524ce`.
It records identical start/end revision
`ac077024a358e5f4b2fb1a12a643e96353f56b67`, Windows 11 AMD64, 200 literal
fixtures, 100 semantic/type resource scopes, ten module scopes and empty
source/tool changes. The preceding 193-case report remains separately preserved
as `target/selfhost/m3-semantic/evidence-193-3ad0189.json`.

The bound polling report `target/selfhost/m3-semantic/poll-contract-evidence.json`
has SHA-256
`763a5816c8a01c32f107b0450af698ac6357c0db2155bb021b7e7c7913987863`.
Its eighteen cases use the exact newly built CLI artifact. The three malformed
trait receiver declarations are rejected by both reference paths and the SG
checker. A reordered unused Poll impl and a same-spelled replacement context
retain documented typed differences; a real await of the reordered Poll is
independently rejected by the frozen compiler in both check and LLVM-emission
commands. No native async lifecycle or complete SDK capability follows from
these static protocol regressions. Full M3 remains open.

## Native Buffer input observations and real runtime cleanup

- [x] Separate non-retaining native getter inputs from projected source-wrapper outputs: reading an already-owned Buffer handle through the exact C buffer_ptr declaration does not move that Buffer, while an ordinary source projector still transfers ownership outside a proved noescape context.
- [x] Audit the exact fixed-arity, signed-i64 C prototypes and destination position of unicode_version_copy, unicode_provenance_copy and ffi_last_error_copy; reject source-name shadows, wrong arity and variadic signature facts.
- [x] Re-run the full typed HIR/MIR/ownership gate including fifty native signature poison/restore resource rounds, and independently execute four actual runtime Buffer programs over one hundred scopes each.
- [x] Rebuild a module-aware CLI from the same frozen source closure and require the actual std::string import to reach the ownership phase with zero diagnostics.
- [x] Reject zero or negative runtime pointer status before constructing a safe borrowed view; the production native guard and actual-runtime old/new consumer-reachability controls are verified in the later status checkpoint below.

The final frontend report
target/selfhost/m3-native-input-borrow/final-checkpoint/evidence.json
has SHA-256
7bc81a9059190f81b3b1bf1af95801f6a769eab74d0340d429cde3a588fdbccb.
It records full start/end revision
ac077024a358e5f4b2fb1a12a643e96353f56b67, Windows 11 AMD64,
Python 3.12.14, four successful build/native commands and empty source/tool
changes. All previous fifty ownership resource rounds, ten HIR roundtrip and
poison rounds, and thirty raw MIR rounds remain enabled.

The independent initial compiler-build/runtime report
target/selfhost/m3-buffer-observers/final-checkpoint/evidence.json
has SHA-256
b6bbf73562750a4078e9561a03c7577f93370dc99cc96bda9c9eeb8e511f2bf8.
It records 33 commands, exact immutable compiler snapshots, identical repeated
COFF emissions and the same full start/end revision. The final harness replay
target/selfhost/m3-buffer-observers/final-confirmed/evidence.json
has SHA-256
62c204d1ac8756717d8d067f586ef487095274dbab828ebefe6b8fc2f8bec33d.
Its 31 commands explicitly reuse only the prior hash-bound compiler executables
after checking their complete source/dependency/tool identities; runtime C is
freshly compiled and every execution is repeated. Source, snapshot, checkout
and tool change lists are empty. Early seed identity failure and a replaced
compiler-reuse artifact both fail before executing the supplied unverified
compiler and preserve failed reports in their named negative-result directories.

Candidate programs use actual runtime.c and runtime_string.c Buffer allocations,
two-byte reads, bounded Unicode version/provenance copies and a nonempty
last-error copy. C independently verifies stored bytes and exactly one release,
with String/Buffer/opaque live counts returning to zero on every scope.
All four candidate executions return 42. All four frozen reference LLVM programs
compile and link but return 2 on the first scope with the exact independent
diagnostic "reference-resource-gap live=1 created=1 freed=0 verified=1".
These are preserved Buffer cleanup differences: the result is
passed-with-reference-resource-gaps, with parity_pass: false, never a
reference-equivalence claim. The last-error fixture restores the error after
evaluating ptr/len arguments; it does not assert that the current production
ffi_last_error_copy wrapper preserves an earlier error, because those getters
clear that state.

The actual std::string import now has zero typed and ownership diagnostics
(previously nine ownership diagnostics). This is a checking result for its
five-module graph, not full standard-library native lowering or self-compilation.
Complete core/generic/closure/async ownership, SDK admission and M3 remain open.

The same qualified CLI development scan of selfhost/src/core.sg records 236
remaining ownership diagnostics (228 code 6101 and eight code 6112), down from
245 at the preceding profile-nine snapshot. The unchanged-input discovery is
preserved as target/selfhost/m3-buffer-observers/actual-core-final.json, SHA-256
62ff1a1ae93a6e8f7fce3f2ae55d487bb1ac34aa2e9df6e0c2d4ff8e366a8857.
It is not a passing core/native compilation gate. Remaining cases concentrate
in collections scalar/reference reads, iterator/handle state and two core
borrowed String-field method calls; they stay open.


## Borrowed receiver and scalar dereference checkpoint

- [x] Preserve shared/mutable method receiver borrowing from the instantiated function signature even when no by-value observation summary exists.
- [x] Classify a dereference transfer using the typed expression result rather than the potentially different type of its loan provenance.
- [x] Copy non-borrow-carrying dereference results without transferring the source mutable-reference loan; retain owned-value move-out-of-borrow rejection.
- [x] Run six additional positive/negative cases within the fifty-round ownership resource suite, together with the complete HIR/MIR poison, roundtrip and raw CFG/SSA gates.
- [x] Rebuild the current module-aware CLI and independently record actual std::string and core checking results without calling a still-failing core entry complete.

Formal frontend report:
target/selfhost/m3-borrow-reference-access/final-checkpoint/evidence.json,
SHA-256 912b4972617ed7481050fdbb6bb92f69e0d3d827dd18b28bcc35fb6b969276eb.
It records actual start revision 49e47f80af573f3186e853c6abb41909412bbfac
and end revision 2b64f9bcb6768494a13c0d255880ada85387d668, Windows 11 AMD64,
Python 3.12.14, all four native build/execution commands successful and empty
source/tool changes. The intervening independent commits did not alter the
declared frontend import/resource closure.

The current-source CLI discovery report
target/selfhost/m3-borrow-reference-access/current-discovery/evidence.json
has SHA-256 607be485473a7744dab7d86323438b54552f69187ca2da8c11d0b04b907032e9.
It records identical full revision
3e70fd54feb5ff0e6c3f4144889b6a7845000ec3 and empty source/tool changes.
The actual std::string graph still has zero ownership diagnostics. The actual
core entry has 230 remaining ownership diagnostics, down from 236: six scalar
borrow-provenance false positives are closed, while 228 move diagnostics and
two borrowed String-field by-value method cases remain open. This discovery
is not a successful core/native self-compilation gate. The six small fixtures
also have separately saved frozen-reference check/LLVM phase observations;
their conservative SG owned-move rejection is not presented as a zero-difference
frozen-reference result.

## Production runtime pointer status rejection

- [x] Build a native compiler from a complete immutable production SG import snapshot and verify real registered String storage over one hundred valid scopes.
- [x] Use a hash-bound pre-guard compiler as the negative control for zero, invalid-handle status, out-of-memory status, minimum signed status and a real invalid registered handle.
- [x] Require every old control to enter the C consumer with exit 55 and every guarded execution to trap before that consumer, with no marker output.

The durable gate is selfhost/verify_runtime_pointer_status.py. Its final report
target/selfhost/m4-runtime-pointer-status/final-production-checkpoint/evidence.json
has SHA-256 b789d1501d7186ca1bca92e5394e70382d4ec364035de1faa70b2acfba80cee9.
It records 36 passing commands, identical full revision
3e70fd54feb5ff0e6c3f4144889b6a7845000ec3, complete C/Rust runtime and Cargo
dependency input inventory, and empty dependency/snapshot/tool changes.
A concurrent checkout edit to native.sg is explicitly listed under
checkout_changes; compilation consumes the preserved snapshot, so that edit
does not become part of this binary. The original 36-command report remains
preserved at production-checkpoint/evidence-36cmd-initial.json.

Five old controls independently print the consumer-reached marker and exit 55.
Five new controls terminate with Windows status 0xC000001D and no consumer
output. The valid registered-storage program returns 42 after one hundred
exact resource scopes. This protects the non-positive status boundary; it
does not prove that arbitrary positive pointer bits identify valid storage.
Full SDK admission, complete M3 and native self-compilation remain open.

## Audited slice and bounded status-copy inputs

- [x] Prove the exact fixed signed-i64 C slice input and two bounded status-copy destination inputs without authorizing borrowed raw outputs.
- [x] Verify source shadows, wrong arity, variadic signature flags and unaudited positions, with the complete frontend ownership/HIR/MIR resource regressions.
- [x] Independently execute actual C providers for one hundred scopes, proving independent String storage, UTF-8 bounds, exact status bytes, retained capacity-failure storage and full resource recovery.

Frontend report:
`target/selfhost/m3-audited-copy-inputs/passed-checkpoint/evidence.json`, SHA-256
`36764ea55fceeaa27eadbddde680094a22c2e1f8c73259777554dce71b08677f`.
Provider report:
`target/selfhost/m3-audited-copy-inputs/provider-final/evidence.json`, SHA-256
`da2c8f1515b00eb4819d9be5ddc37cde0e72fa6157c658826623da8be9ed37e5`.
Both record identical full start/end revision
`bb21b3b96aad259215561233a40a6a6c2aee4d02`, Windows 11 AMD64, four passing
commands each and unchanged input/tool hashes. The frontend retains fifty
ownership/resource rounds, ten HIR roundtrip/poison rounds and thirty raw MIR
rounds. The actual provider gate compares complete UTF-8 slice bytes, observes
copies after the source is changed and released, rejects invalid UTF-8 boundaries
without allocation, checks exact status text and untouched tails, and restores
all String/Buffer/opaque resource counts. A wrong-clang identity control fails
before compiler execution.

The actual current core diagnostic ledger is
`target/selfhost/m3-audited-copy-inputs/remaining-core-ledger.json`, SHA-256
`59324506ac172fae3d5d23b5cc4efab5f1108bd843b6c08dd12cc7f65ffea6b8`.
Core ownership diagnostics decrease from 230 to 226; std::string imports retain
zero. The remaining reports are all 6101 in collections.sg at 99 distinct
positions, with repeated CFG passes accounting for duplicates. Iterator done
observations, borrowed next/copy results and manual handle detachment remain
separate mechanisms. Native core compilation and complete M3 remain open.

## Exact iterator exhaustion observations

- [x] Preserve ownership through the exact fixed C vector/map-key iterator done input prototypes, retaining source-shadow, wrong-arity/type and variadic rejection and ordinary creation/free transfer behavior.
- [x] Verify real container/iterator providers with descriptor move/drop callbacks, unchanged positions/live handles and exact cleanup over one hundred scopes.

Frontend report:
`target/selfhost/m3-iterator-done-inputs/frontend-checkpoint/evidence.json`, SHA-256
`ae8bc92ef880d317e9020bc78c5644ea779dcfaa68c7798d3371ae85391fc9cf`.
Provider report:
`target/selfhost/m3-iterator-done-inputs/provider-checkpoint/evidence.json`, SHA-256
`7872f3eaab743937fe1bc86947b6a330b04f6e8430ab3d6775ee057fedffbb11`.
Current source discovery:
`target/selfhost/m3-iterator-done-inputs/current-discovery/evidence.json`, SHA-256
`b6d861db736f670dca33ed969d0b1c47c7cb996eb9fca5b35eb2fce2ecebc97b`.
All three record identical full start/end revision
`a5b45bfe5fe269ab58184648314a30dadc0db5ba`, Windows 11 AMD64 and unchanged
input/tool hashes. Four complete frontend build/execution commands retain fifty
ownership/resource rounds, HIR poison/roundtrip and raw MIR regressions. The
five-command C gate queries real Vec and HashMap iterators repeatedly, observes
unchanged indices and exact Drop callbacks, and checks exhausted, backing-freed,
closed and zero-handle cases. It does not authorize arbitrary wrong-kind handles
or change next/output lifetime rules.

Actual core diagnostics decrease from 226 to 119; std::string imports remain
at zero. The source-bound ledger SHA-256 is
`dc8fbc4ac4c1ff0f2c1cf72d21477ebd39cdbfb2f46bbc0dc7eb156ae345cef7`.
Repeated CFG paths explain duplicate reports; these counts are not counts of
independent defects. Borrowed next/copy results, higher-level iterator wrappers
and seven handle detachment sites remain open. Target-only scalar-copy provider
experiments do not qualify a production typed bridge or reduce this formal
119-diagnostic checkpoint.

## Physical module MIR evidence

`target/selfhost/m3-module-mir-expanded-final/evidence.json`, SHA-256
`4a5d4a855f12f4a11fdd0f7a82fe77938e8fc8e18efaf9ee27f983385e66cf41`,
records identical full start/end revision
`a5b45bfe5fe269ab58184648314a30dadc0db5ba`, Windows 11 AMD64 and UTC
2026-11-09 21:28:20 through 21:29:35. All fourteen commands passed with
190 unchanged source/snapshot inputs, seven unchanged tools, unchanged Cargo
configuration presence/content and no producer checkout changes.

The actual three-file program contains a physical BOM and Unicode comment,
a macro declared in one file and invoked in another, one removed cfg function,
an enabled cfg attribute mask and a concrete generic instance. The native test
observes all four transformations, validates two local spans, twenty-one
instruction spans, four terminator spans and four block spans against their
owning functions, and emits the exact macro invocation interval. Independent
Python byte-literal expectations check all four complete function intervals,
the specialization name and macro origin. Whole-file spans, wrong instance
names, shifted macro positions and extra output are deliberate oracle controls.

Nine rejected metadata states preserve every MIR vector, scalar, type arena,
symbol UTF-8 bytes and original storage handle: invalid source length,
cross-file span, absent/holey maps, maximal origin, wrong frontend source,
ambiguous replay and source-count overflow. The latter starts with a valid
empty MIR limited to two entries and a three-file graph. No warmup precedes the
ten complete resource scopes. Two deliberate assertion failures exit one and
name the fixture in stderr. Before/after mapping COFF bytes are identical; the
mapped object and frozen reference both execute 42 with equal output. Full HIR
module persistence, public tool integration and multiple-debug-module support
remain open. This adapter alone does not establish M3 or compiler self-build.

Specialization-oracle hardening:
`target/selfhost/m3-module-mir-instance-name-qualified/evidence.json`, SHA-256
`838d6b0e1e68c657faa3534017cd1ea079471fe0109a55864dd40134a98fa61c`,
full start/end revision `4ca67c911907e8caa354554b1944f1190d88b871`, Windows 11
AMD64. Fourteen commands and all existing mapping/resource/negative controls
pass with unchanged inputs/tools/configurations/checkout. The independent
oracle now explicitly rejects the bare generic name `identity`, as well as
other malformed specialization names. The observed generated name remains
`identity__sg_instance_3`; the previous report's actual code was specialized,
but its name check also accepted the bare spelling. The stronger negative
control closes that test-oracle gap without changing the adapter implementation.


## HIR6 complete owning snapshot fidelity

- [x] Preserve all ShChecked limits/scalars/arenas/symbol bytes and every ShOwnership field through independent clone/codec ownership.
- [x] Migrate HIR schema to six while rejecting old five and preserving builtin6/deferred4/actions8/9/10 semantics.
- [x] Bind actual module/ownership inputs through read-only full-state comparison, without using a decoded payload as its own external source authority.
- [x] Verify ordinary legacy behavior, complete per-field corruption and codec/budget refusals, first-call resources and required HIR/MIR/source-native regressions.
- [x] Freeze full revision/host/tool/source/artifact evidence and strict specs with complete compiler milestones still open.

Candidate-specific codec and authority-boundary design/qualification is owned
by `add-selfhost-collection-intrinsics/hir-codec-checkpoint.md` and its
`selfhost-hir-origin-codec` capability. This section owns generic HIR fidelity.

Qualification: `target/selfhost/m3-hir6-owning-budget-qualified3/evidence.json`
SHA-256 `87a96fcb6e9a91eafa7ccab82d287710573a95eaa1e11512516fa316f07f08f4`,
Windows-11-10.0.26300-SP0; full revision
`0b797a88391fe8e28a7bb35d2255ab79b3da02ca` through
`b7867614ce59d4e404c702784df52db6acb9db8a`. Concurrent commits did not alter
consumed inputs; all source/tool/configuration/checkout/artifact guards are
empty. Ten ordinary plus seven module first-call scopes, 120 field poisons,
thirteen mapping poisons, actual physical-text replacement refusal, exact
aggregate budget boundaries and old-HIR5 rejection all execute. Complete
legacy regressions pass expected exits: HIR/MIR four, disarm nineteen with its
explicit reference zero-drop policy difference, projection twenty-two and
source-native 346 commands (49 positive/five negative programs). Independent
audit SHA-256 `beb39143b3ad1944416ebc63dd46b955b3539881435160a5167eed45abdfffba`
rehashes 3,709 records and 820 receipt files. Full source pins, original limits,
ownership diagnostics and HIR error/start/end are retained. Actual standard
ownership still rejects 6101 with 112 diagnostics; no M3/core112 closure,
constructor/Ref authority, compiler self-build capacity or M8 RSS result is
claimed. The collection checkpoint records exact commands, child report
hashes, failed trials and the C/Rust development/legacy-linker boundary.
