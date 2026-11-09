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
