# Independent origin facts, stage one

ShOriginFacts owns bounded vectors and identity/prototype strings. It consumes
actual ShModules and ShModuleChecked (which contains Front and ShChecked), plus
caller-supplied collection physical identity, SHA256 and exact raw-iterator
prototype. Admission verifies immutable bytes against the graph's source text,
real declaration/module bindings, canonical layouts, actual Vec constructor and
all three same-T iter signatures and source bodies. SDK/program IDs are never
interchanged and no source name alone grants authority.

Node records use eight words: state, role, elementType, backingBinding,
producerAST, parentAST, functionBinding, conditionalParameterBinding. State zero
is unknown, one is concrete producer origin, two is a conditional parameter,
three is inert/null. Roles are Vec, RawVecIter, VecIter. Raw handle expressions
and ordinary nonzero reconstructed records remain unknown. Whole moves and
reborrows preserve same-T origin; a moved binding stops supplying authority.
Zero placeholders retain inert state and never count as a concrete iterator.

Factory rows bind actual callable/declaration/signature/module. Return rows
record state, role, elementType, backingParameterOrdinal and source AST.
Conditional parameter uses produce explicit five-word function/parameter/role/
element/site obligations. A caller's unknown argument cannot satisfy them.
Simple return forwarding substitutes the actual checked call signature and
argument origin, never an assumed trusted parameter. Returning an iterator of
a dead local backing vector loses authority and records a diagnostic.

The first implementation is intentionally partial: only straight-line functions
in the root physical module plus the admitted library factories are analyzed.
General CFG joins, recursive summaries, mutation invalidation/loan liveness,
independent module exports and actual raw-next pointer-to-Option lowering remain
open. Facts are source-origin eligibility, not permission to emit safe refs or
suppress owning Drop. Unknown controls fail closed. Sixteen summary passes are
bounded; non-convergence explicitly fails instead of granting facts.

Validation re-derives only these bounded origin rules from the existing typed
input and caller authority, then compares every owned proof table. It never
re-runs parsing/Sh_check or calls normalizers/interners. Owned copies bind all
type, AST, resolution/call and physical module inputs. Arena byte/count and five
runtime resource counters are checked independently. Later HIR6 integration
must make these relations mandatory and preserve the existing six-word builtin,
four-word deferred and ownership eight/nine/ten contracts.

All getters, including sh_origin_concrete_iterator, are queries on facts that a
consumer has already validated. They are not independent native/HIR authority
entry points. A production consumer must first call validate with independent
caller pins and the actual graph/checked input. Schema one belongs only to this
independent analysis; it does not claim that HIR6 or any native bridge exists.

Before borrowed-next can go live, backing storage effects must be enforced,
including legacy Vec push/set/clear/free methods with shared receivers and raw
handle escape into unknown C. A copied scalar field passed to unknown C is not
a proof of no retain/free effect. This stage conservatively invalidates tracked
origins across unsupported controls and unknown canonical-argument calls; it
does not claim complete alias/loan or native-effect inference. A saved origin
getter does not prove that storage remains live after arbitrary native calls.

All callee origin obligations are discharged regardless of result type. Scalar
returns and two-parameter/one-result functions retain every requirement. Only a
conditional actual may transfer its requirement to the caller. Unknown actuals
produce unproved-use diagnostics; unresolved multiple conditional dependencies
with a concrete result stay unknown and retain a partial flag. The internal
binding workspace is cleared before facts are exposed and validators reject
any nonempty workspace or nonreset internal traversal state.

The executable fixtures include branch and loop invalidation, raw handle escape
to an unknown C call, and clearing the entire node-origin table. These all lack
new concrete eligibility or fail validation. Call, obligation and diagnostic
record tables are capped by the explicit origin limit. A tiny source limit fails
before admission/analysis.


## Stage two: independent backing leases and source effects

`semantic_loans.sg` owns a separate ShBackingLoans. Caller supplies real origins,
module/checked input, canonical collections identity/SHA/raw-iter-new prototype,
and a separate Option physical identity/SHA. Validation rechecks both external
pins. Option membership is actual ADT binding/module plus struct bool-tag and
value-T field shape, not nominal spelling. Raw-next and iterator free/Drop
candidates validate actual pinned declaration/body, function ABI/flags/result,
receiver mutability, field owner/type and real receiver parameter relationship.

Node records retain state/role/T/backing/producer/lease/conditional parameter.
Leases retain callable, holder, backing, T, producer, scope, creation/end and
kind/parent. Returned-parameter and source-call effect records substitute actual
arguments; arbitrary i64 parameters do not become physical handle origins.
Scratch bindings and active bits are cleared before persisted facts are exposed.

Scope-end retention is deliberately conservative. Explicit iterator release
requires an exact candidate release body and no dependent element/reborrow
lease. A tail element candidate stored outside its producing block retains its
own lease when the inner iterator scope ends. A dead local or unsaved temporary
Vec backing is rejected. Whole backing owner moves during a live iterator are
conflicts; iterator whole moves transfer lease holders and reborrows preserve
backing relationships. Raw scalar handles retain taint, and nonzero record
reconstruction does not re-seal a lease.

Every actual source parameter has an independently inferred bitmask summary:
bit1 may invalidate/escape, bit2 requires a verified canonical origin or iterator
lease; release is3. Zero means a source body has no recognized effect or origin
obligation. Every required-origin bit is discharged even for scalar returns and
for parameters other than the returned one; conditional inputs remain conditional. Calls propagate these effects through all actual arguments,
not merely a returned parameter. Unknown C or bodyless calls are conservative.
A source body returning a constant and making no effectful call can prove a
no-effect parameter; this does not bless arbitrary shared references. A mode
zero causal control ignores summaries and rejects the otherwise allowed pure
observer call while a lease is live. Mutating saved summaries fails validation.

This phase authenticates logical source safety candidates only. All payloads
retain unlowerable=1. Actual Vecnew source is still a zero placeholder whose
allocation semantics live in S0's constructor intrinsic. Raw-next source is
still the admitted recursive intrinsic placeholder. Neither body establishes
an actual runtime allocation or safe returned pointer. No String26 cast, raw
integer, slot width or nonnull callback constructs a reference in this phase.

Before HIR6/MIR optional borrowing, the old i64/bool public raw wrappers require
conditional physical-handle obligations carrying T/backing/lease from a real
canonical receiver.handle projection at every caller, or a separate admitted
internal typed wrapper while retaining old public entries as ordinary unsafe/
unlowerable boundaries. The early typed-reference rewrite's scalar-copy effect
is not this provider-return-loan proof. One coordinated HIR6 integration will
preserve builtin6/deferred4/actions8/9/10 and reject old HIR5 with5502; MIR must
carry independently certified constructor, pointer and Some/null facts.

The scope-end lease model deliberately over-rejects `last_use_overreject`: a
mutation after the iterator's last reachable use produces5803 in this candidate
phase. Canonical memory-management requires last-reachable-use NLL; this case
must become accepted before final language compatibility is claimed. General
CFG candidate source rejects5806. By-value iterator calls remain conservative
transfer effects; no reduction of the production actualcore112 ledger is claimed.

Existing generic explicit `.drop()` method resolution can reject5123 before
loan analysis; a separate typed fixture retains this boundary. Canonical
`.free()` and source owner transfer independently prove live-lease conflicts.
Unsupported ordinary C reference-parameter syntax has its own5122 fixture;
neither error is suppressed to reach loan analysis. A direct non-Copy owned
pointee move rejects5807; Owner candidate borrowing emits no copy or Drop.

The current pinned S0 has a separately reproduced lexical-shadow defect:
a branch-local `element` can incorrectly replace an outer local after a false
branch, reading uninitialized storage. This module uses a distinct
`borrowed_element`; the exact minimal source, LLVM, real outputs and unique-name
control are preserved in target/selfhost/m3-iterator-loans/stage0-shadow. This
is a narrow seed compatibility ledger, not a redefinition of SG lexical scopes
or a claim about every scope pattern.
