# Optional borrowing and push integration checkpoint

This is an implementation plan and development ledger within the approved
collection child. It does not mark full source integration or M3/M4 complete.
Physical MIR27 and typed layout-cache behavior are uniquely owned by the
native child `selfhost-optional-borrow-abi` capability. This child owns actual
source/provider/backing integration, push consumption and the independent
effect-refinement model. HIR6 candidate persistence remains separately owned.

The public compiler entry will freshly validate actual physical modules,
typed input and explicit reachable roots, independent provider manifest/object
pins, original origin/loan/NLL candidates and fresh ownership. It will own HIR6
and lower on its private checked-type arena before emitting immediately.
An external candidate MIR or decoded context cannot select its own authority.
Ordinary MIR/native entrypoints continue to reject opcode27 without this path.

The physical operation has two inputs: the actual fixed-C next result and the
exact handle SSA value passed to that call. It returns the complete concrete
bool/reference Option. Same-initializer Some guards are checked through direct
SSA and one unique local initializer. False paths, uninitialized or aliased
carriers and reference escapes reject. Non-Copy owner field access retains a
borrowed address and reads only the requested scalar; it must not load or Drop
the complete owner as an intermediate representation.

Initial source integration expands the complete, independently checked scalar
helper at an admitted concrete caller, avoiding recursive placeholders and
unproved whole-Option return propagation. Direct canonical borrowed-next may
similarly be expanded at its actual callsite when return-loan/backing facts
and a Some-guarded local consumer are proved. Remaining conditional caller
obligations cannot become native permission by merely recording status two.

The new effect model preserves original loan/NLL tables and diagnostics. It
binds actual source calls, provider bindings, concrete callable/T, roots,
tokens and members, recomputes the actual CFG equations into separate output
tables, and records each superseded row and its proof. A proved push preserves
the vector-owner token but invalidates every existing iterator/element token
for that root. A renewed iterator proof identifies the actual canonical
factory and provider, parent owner, same-T token and valid CFG input; a missing
old lease ID is never relabeled as a valid old lease. Primitive raw-handle
observations require an exact complete-body helper receipt. Callpoint token
uses check backing after all argument evaluation, not only at an earlier field
read. Other unresolved diagnostics remain errors. Saved refinements retain
`unlowerable=1` until the fresh complete compiler path discharges them.

The actual selected C push contract consumes on status zero by move and on
out-of-memory status fourteen by callee Drop. Invalid-argument status two
leaves the input unconsumed. The MIR emitter spills T into typed storage,
calls the actual provider through an explicit raw pointer, disarms on zero or
fourteen, and performs exactly one typed caller Drop on two. Other statuses
trap as provider violations. The source hook must also transfer responsibility
from any already-created source temporary or moved path; the sink's flag alone
cannot suppress an independent caller cleanup. Required-provider mask32 is an
obligation, never evidence that these conditions were met.

The physical MIR27/push ABI phase was committed as `ed3aaa4ea7f778603712cf21a49aa8661ac1634b`.
Its native-child checkpoint owns physical execution and typed cleanup evidence.
It does not qualify an external candidate or authorize source references.

## Fresh source effect and ownership qualification

The independent source checkpoint is
`target/selfhost/m3-collection-source-effects-qualified5/evidence.json`, SHA256
`826942510a446b966656e3e7b11b4574232620fa5244b7ffeb65958a37196504`.
It passed ten commands on `Windows-11-10.0.26300-SP0`, from
`2026-11-10T03:13:46.073099+00:00` to
`2026-11-10T03:20:24.275354+00:00`. Both full revisions are
`3324cd613cc44fe3255e1b83257f26b73e6ddb2a`. The report binds 662 actual retained
inputs, 191 checkout originals and seven tools; all source, checkout, tool,
configuration and artifact change lists are empty. Its immutable source snapshot
includes the deliberate new scalar-escape language fix in `borrow.sg`, SHA256
`827539b4f828a0575c8945517be79cb63da03cdb6638466e5c7ec2fcd5d90dd2`, which was
committed in that revision; all producer inputs retain their exact recorded bytes. Other concurrent changes
outside the import closure do not become inputs or authority.

The source implementation hashes are:

- `semantic_collection_effects.sg`: `41f431d6fd264e4048d2f0d579ef1bfb3018004c16fe682c540d10ee04f38761`.
- `semantic_collection_effect_ownership.sg`: `97bc563e3c308e4c1c2128a0cc8263d6f349a2cc41fbe2156ab38760d8dff690`.

The program is a retained 4,919-byte physical UTF-8 file under a directory with
Unicode, spaces and `&`; its SHA256 is
`c4093fa7bfcb0ee2e58d8262c837cdb7855f3fb5b8cac5184a496b56b704ee30`.
It is checked from actual physical modules and the historical namespace-two SDK.
Typed field finalization runs before any facts. A second finalization checks
idempotence; it does not warm resource accounting. Every subsequent analysis and
validation preserves the frozen checked vectors, scalars, symbols and handles,
AST/source-map and actual physical source bytes.

Six concrete root functions pass fresh scoped ownership with the unchanged
22-field state and explicit mode-three coverage. They cover i64/bool complete
scalar helpers, push followed by a new iterator, a non-Copy owner with an empty
actual Drop body, and a named owner read before its later consuming push.
There are 30 actual operation rows, 37 exact old-NLL supersessions, 22 exact
old-loan supersessions, and nine candidate lowering-hook rows. Original loan
and NLL diagnostics remain present. Original validator entrypoints do not accept
the adjusted NLL/handle tables as original facts.

Fifty-three effect controls replay every vector, including correctly empty
vectors, all compared nested scalar state, independent caller pins, actual
physical text and type-arena changes. Twenty-eight outer controls replay all
22 ownership fields plus functions/coverage, duplicate selection and independent
pins/development mode. A named owner used after push rejects with 6101. Unguarded,
false-branch and stored-before-guard payload observations reject with 6124.
Continued use of an older iterator or element after push and a fallback argument
which clears the backing remain rejected. A mutable Option reassigned after its
tag test fails both source admission and its direct payload-guard query.

Because the C provider may call the element destructor on allocation failure,
a custom destructor currently requires a unique exact concrete target, actual
SG signature `(&mut T)->unit`, and an empty actual body. Nested elements are
checked too. An actual destructor with an opaque external call produces no
admitted push operation and leaves a nonzero function error. This deliberately
excludes source resource destructors with unproved effects from borrow-preserving
push admission; constructor-only callback generation is a separate scope.

Actual ADT source admission is restricted to struct declarations (binding kind
three and AST declaration kind 22), with no recorded parent relation. A real
Base/Derived fixture whose base owns a field with an opaque-calling destructor
is typed successfully, but both class-type getters reject; Derived push emits
no admitted operation and leaves a nonzero source-function error. Enum AST24 is
also explicitly unsupported at this checkpoint. Nested fields recurse through
the same shape and destructor restrictions without type interning.

Three complete first-call scopes compare String, Buffer, opaque, file-snapshot
handle and file-snapshot byte counts against a baseline taken before the first
scope. Both intentional assertion-failure controls execute and return one with
the test filename in stderr. The final complete stdout is independently parsed
for exact markers, row counts and no trailing text. The SG test executable is
`5ceb52817f721da727e4cbe89aa43ab7c73474aa557c58a8e502d8012c2f4bd3`.

The flat audit is
`target/selfhost/m3-collection-source-effects-qualified5/flat_audit.json`, SHA256
`dc525478a7c05e52801047870af70767ea4a625c4f278465b2d61a5d3bfdaaeb`.
All 1,308 input/tool/configuration/artifact records and 20 command receipts match;
seven additional retained object/IR/executable artifacts are independently listed.
The SDK branch is the genuinely consumed historical provider report
`a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3` with its
immutable source/tool/object chain. Its historical checkout records are not
replaced with hashes of later mutable algorithms or documentation.

Fresh directories `qualified1`, `qualified2`, and `qualified3` retain real
failed test-build trials: source-map ledger type mismatch, a generated invalid
pin argument list, and a missing unit-valued assertion branch respectively.
They are not qualifications. The earlier `qualified4` report remains an
unchanged historical passing report with SHA256
`cd85351e47150298ae93e476ba598ea4684ca12102d4d99c9c140edcc333dde9`;
it preceded the independent inherited-class review, so it does not qualify the
class/enum boundary now tested by `qualified5`. The final report recompiles a
fresh snapshot and does not reuse any failed-trial executable.

This checkpoint qualifies bounded source candidates and the independent
ownership outer layer. `unlowerable=1` remains mandatory. No public native source
entry consumes these facts yet; no complete source Some/push execution, arbitrary
Drop effect, generic helper job, full compiler closure, M6 cutover or M8 capacity
result follows from it. The development compiler still uses its recorded C/Rust
runtime. Requested lld selection does not qualify all nested SDK child-linker
inputs. Full compiler self-building remains open.
