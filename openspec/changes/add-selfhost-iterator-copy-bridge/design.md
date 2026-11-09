# Authority, types and scalar copies

## Current evidence and rejected shortcuts

`raw_vec_iter_next_i64` and `raw_vec_iter_next_bool_i64` have source-level i64
handle parameters. Their `Option<&T>` temporary depends on the Rust intrinsic
that calls `sengoo_raw_vec_iter_next` and constructs the option/reference. The
existing observed/projected source summaries do not prove that intrinsic's
physical pointer, storage provenance or lifetime. A same-named ordinary function
or external declaration is not its implementation.

The target-only C proof uses explicit scalar copy helpers, with producer-kind
tracking for actual vector/iterator handles and descriptor width/alignment
checks. i64 is copied with memcpy of eight bytes; bool reads one byte and accepts
only zero/one. Invalid, stale, wrong-family, exhausted and mismatched physical
inputs use the supplied fallback without advancing the iterator. An independent
copy remains valid after mutation/release of the backing storage. The helper
neither retains nor frees the inputs. These physical checks cannot distinguish
an i64 from a same-sized owner or other non-Copy value. They are necessary
runtime checks, not semantic type authority.

## Preferred typed source contract

Keep the legacy public raw-handle wrappers intact. Introduce separate internal
wrappers that accept shared references to the actual admitted
`VecIter<i64>`/`VecIter<bool>` nominal type. Specialized iterator methods pass
their receiver to those wrappers, which load its genuine field-zero signed-i64
handle and call the corresponding admitted C helper. Bool's physical scalar
result is normalized by the existing language conversion. The method's declared
language result and physical C result remain separate facts.

Authorization requires all of the following relationships, not just names:

- A caller pin admits the target, ABI, manifest and actual provider artifact.
  The helper export has a real defined symbol in that selected object and the
  source provider prototype matches the reviewed fixed C signature.
- The imported declaration is the exact physical SDK/prelude module and byte
  declaration admitted by that catalog. Its actual checked binding is external,
  bodyless and fixed C `(i64, i64) -> i64`. SDK type/AST IDs are structurally
  mapped into program IDs; numeric coincidence cannot authorize a binding.
- The internal source wrapper's definition comes from its pinned collections
  module identity and byte declaration, not an application alias or shadow.
- The real call receiver is a shared reference to that module's actual nominal
  `VecIter` with exactly one concrete i64 or bool argument. Its layout uses the
  declared handle field; casts, arbitrary integers, unrelated records, generic
  non-Copy arguments and same-shaped application types do not satisfy it.
- The authorized C input expression is precisely the checked handle-field
  projection from that receiver. The fallback is the wrapper's checked i64
  parameter. The runtime callee resolution and callable facts equal the admitted
  physical declaration. No `binding = -1` synthesized authorization is allowed.

An immutable per-source-call side table can prove these relationships before
ownership checking. The table references the actual call AST ID, concrete
receiver type and declaration, handle-field binding, runtime binding/type,
source wrapper binding and provider-certificate index. It grants input
observation only for that call and position zero. It does not make arbitrary C
results observers, turn integers into safe references or mark the public raw
wrapper as a privileged name.

The target prototype uses twelve words per call: call AST, scalar kind,
receiver AST, concrete receiver ADT type, handle-field binding, actual runtime
binding, physical C function type, source-wrapper binding, wrapper function
type, physical source-module ID, wrapper declaration AST and nominal iterator
binding. Provider admission is a separately retained context containing the
caller-pinned manifest, source snapshots, actual COFF provider and exact mapped
declarations. The table cannot authorize itself. Consumption rechecks the
retained authority, every call relationship and primitive result; poisoning
either the side table or the actual checked declaration must fail.

Only canonical VecIter specializations participate in the initial source
rewrite. HashMapIter owns additional values storage, and other same-shaped
iterator types have different lifetime/cleanup contracts. Their legacy calls
remain unchanged and their diagnostics remain visible. An Enumerate wrapper
may eventually use its actual VecIter field through that field's provenance;
its outer nominal type is not automatically an authorized receiver.

## Source summaries and owning cleanup

The typed wrapper's shared reference uses ordinary borrow checking. The target
prototype additionally offers a validated call-local observation branch, but
that branch must not be credited with a diagnostic reduction merely because
the typed source also changes. The existing checker already permits reading a
scalar handle field through an explicit shared reference, including when the
scalar is passed to an otherwise unknown C declaration. This is existing scalar
field-copy behavior; it neither authenticates that provider nor constructs a
borrowed result. A clear-side-table control must distinguish this ordinary
reference behavior from new authority-driven checking. If ownership results
are unchanged after clearing the table, contract-driven borrow integration
remains unimplemented despite successful source-binding validation.

Production source summary inference must derive a surrounding method's
contract from its real body and reviewed native effects. Action eight remains
a derived no-retain input certificate; revalidation starts from an empty
summary state and rechecks any native call fact on which it actually depends.
No supplied action eight may justify itself.
The fallback is an ordinary scalar; the result is a copied scalar, carrying no
borrow. A caller owner stays live across the call and is dropped exactly once
at its original scope. The internal C helper never executes that owner's Drop.

When a by-value iterator method returns its receiver, transfers storage, creates
an iterator or explicitly frees it, existing transfer/cleanup rules remain in
force. A scalar copy is not authority to suppress destructor cleanup or allow
iteration after backing storage is released. Runtime producer tracking handles
invalid physical input; source lifetimes still forbid invalidating a live
borrowing iterator.

An owned-input negative control passes an iterator by value to an unknown
wrapper twice and must still report use-after-move 6101. The corresponding
explicit-reference input may read the scalar field without that diagnostic;
the latter is recorded as the existing behavior, not as evidence that all
unknown C functions have acquired observation authority. The originally
incorrect expectation that this reference-input case must move is retained in
the failed target report rather than rewritten into a pass.

## HIR/provider migration and lowering

The production integration requires a provider binding map in addition to the
current six-word String `builtin_calls` table. It records admitted catalog
identity/digest, real prelude declaration identity and program runtime binding,
with independent exact ABI validation. The scalar call table refers to this
map; it must not overload String intrinsic kind one or alias SDK/program type
IDs. A new HIR schema must own, clone, serialize and bound both tables and
their identity strings. Old versions fail explicitly. Pure validation checks
every referenced AST, binding, source identity, concrete type and relationship,
rejects duplicate call records and leaves type arenas unchanged.

MIR concrete jobs substitute generic types and require the exact scalar result
and receiver contract again. Unresolved generic arguments reject instead of
being treated as Copy. Lowering emits the unchanged admitted physical C call;
its handle load keeps the typed receiver as the lifetime origin. MIR preserves
the no-retain contract and owned cleanup, with independent verifier checks. No
new arithmetic opcode or arbitrary i64-to-reference cast is needed. Until this
provider map and lowering certificate exist, native production support remains
an explicit boundary rather than borrowing authority from a target prototype.

## Qualification and runtime boundary

The target proof must actually map admitted physical declarations to real
program bindings, exercise actual core, and reject source shadows, conflicting
declarations, wrong C conventions/arity/result types, wrong primitive widths,
generic/non-Copy arguments, same-shaped user nominal types, wrong provider pins
and corrupted call/provider facts. HIR poisoning and roundtrips are separate
obligations if a production schema is implemented. A source test that merely
passes a fake integer or checks a helper name is insufficient.

The independent provider test covers actual i64 and one-byte bool slots,
fallback preservation, non-advancement on rejection, producer-kind/generation
checks, copied-value independence, no input release and exactly-once external
cleanup. Real SG calls must execute against that target provider with resources
checked independently in C. This is a development C runtime bridge, not a port
of collection algorithms to SG or final SDK qualification. The linked-list tag
prototype's allocation, concurrency and performance behavior are not qualified.

All target evidence records actual revisions, host and complete source/tool
hashes, including the copied provider and authority inputs. Remaining core
diagnostics are classified and retained. The earlier target 34 result is a
discovery baseline and must not be promoted to a completed milestone.

## Canonical reconstruction counterexample and remaining origin proof

The first strict target is intentionally not production-ready. It reduces the
done-contract core baseline from 119 diagnostics to 75 while authorizing only
two concrete scalar helper calls in the pinned typed wrappers. Its actual
shared i64/bool and scalar-return cases pass, and unrelated types, unresolved
generics, non-Copy arguments and source/extern shadows are rejected. However,
the following counterexample is accepted by that partial prototype:

```text
def forged(iterator: RawVecIter<Owner>) -> i64 {
    let fake: VecIter<i64> = VecIter { handle: iterator.handle, marker: 0 };
    sh_iter_typed_copy_i64(&fake, 0)
}
```

Its root has zero ownership diagnostics. Moving the raw identity into a new
canonical nominal value changes the apparent element type without proving the
backing vector's semantic element type. This is a failed required rejection,
not an approved soundness exception or a reason to relabel the case positive.
The target report must retain it as an open producer-origin gap. The bridge
must not be enabled in production on the strength of the 75-count experiment.

ABI-one `SengooTypeDescriptor.flags` does not presently supply this proof:
`raw_vec_new_parts` always sets flags to zero, the descriptor validator does
not validate them, and both scalar and owning elements receive non-null
generated move/drop callbacks. Neither callback presence nor eight-byte layout
distinguishes Copy. A proposed typed producer entry must establish semantic
kind through actual typed lowering/provider authority, not caller-controlled
flags, and leave existing `new_parts` results unknown. Its compatibility with
the old stage-zero producer and any required named seed qualification must be
specified before implementation.

An alternative is explicit origin facts flowing from the actual canonical
`Vec<T>.iter` producer into receiver parameters and through moves/reborrows,
with a checked rejection of arbitrary cross-element handle reconstruction.
If sealed construction is required, preserve the published two-field layout
and ordinary FFI rules and identify the compatibility restriction separately.
The current target does not implement either complete origin route. No further
provider registry or full-tree experiment is needed to establish that missing
obligation; the next design should resolve it before expanding runtime scope.
