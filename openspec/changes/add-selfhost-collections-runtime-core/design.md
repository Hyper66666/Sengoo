# Design

## Context and handle contract

The unsafe C ABI accepts caller-owned, eight-byte-aligned, readable/writable
12,352-byte storage: eight control words and 128 twelve-word slots. Initialization
writes every byte before reading it and allocates no heap. Each public entry takes
the actual extent; a wrong extent or null state returns before memory access.
Context metadata and payload pointers are opaque and must not be forged or edited
while live. Read-only validation checks initialized metadata and bounded fields;
it cannot make arbitrary forged addresses, callbacks, or unsafe extents safe.

The caller supplies a positive 31-bit identity, unique across every live and past
context lifetime whose handles may still exist. A positive signed handle encodes
that identity in bits 32..62, a nonzero 24-bit slot generation in bits 8..31, and
slot+1 in bits 0..7. Lookups check identity, generation, and kind. An exhausted
generation retires the slot; it never wraps. Deliberately initializing two contexts
with the same caller identity violates this unsafe contract: their equal encoded
handles are not distinguishable, and no cross-context safety is claimed then.
Reset requires an empty valid context and a different caller identity. Dispose
drops all vector payloads, closes iterators, clears storage, and allocates nothing.
Double dispose reports invalid context. Caller releases the state itself.

Slots contain generation, kind, reserved, payload pointer, length, capacity,
element size/alignment, C move/drop callback addresses, iterator owner, and index.
The SG registry chooses and retires slots, checks metadata, and owns lifecycle
decisions. A private slot API is unsafe implementation plumbing, not SDK admission.

## RawVec ownership and memory

One SG algorithm implements new/reserve/len/get/push/set/insert/pop/remove/clear/
free and iterator new/next/done/reset/index/free. Elements are arbitrary bytes with
explicit size, power-of-two alignment, and exact C ABI move/drop callbacks. A move
initializes its destination and consumes/zeros the source according to the actual
callback contract; removed values transfer to the caller. Descriptor callback
addresses and storage provenance are caller obligations, never Copy evidence.

The initial bounded profile accepts size 1..1MiB, alignment 1..4096, and at most
64MiB payload storage per vector. Capacity begins at four and doubles. Growth
moves each live element in order through its callback; clear drops in reverse
order. All failure effects match the declared reference status model: invalid
handle/NULL input does not consume input; valid-vector set/insert invalid index
does consume/drop input; reserve failure consumes/drops input exactly once and
preserves previous vector elements; pop/remove return not-found without writing
the destination. Existing C reference behavior is compared inside this bounded
domain rather than claiming SIZE_MAX equivalence.

Borrowed raw pointers are invalidated by growth/mutation/free according to the
caller contract. Iterators retain a generation-checked owner handle; freeing the
backing vector makes them done without reading freed payloads. Slot exhaustion
when constructing an iterator allocates no iterator object and leaks nothing.
The old C iterator implementation can leak its calloc object when subsequent
opaque-handle allocation fails; this is an explicitly observed reference resource
defect, not parity to preserve or silently omit.

Context, live payloads, input element, and output element are disjoint except the
documented exact self-set case. The caller ensures accessible extents, authentic
callbacks, no overlapping mutation/borrow, no concurrency, and no callback reentry.
Concurrent/reentrant/global production behavior is not implemented by this profile.

## Platform and deployment

C supplies only untyped word/pointer/byte load/store, pointer offset/identity,
VirtualAlloc/VirtualFree, and exact C move/drop calling-convention trampolines.
Allocation is 64KiB aligned on the qualified Windows host, satisfying all admitted
alignments; allocation sizes and callback choices are made in SG. C contains no
slot search, generation policy, container capacity, iteration, or type/Copy policy.

`collections_registry.sg` and `collections_vec.sg` are separate SG objects linked
together. Private unsafe C declarations connect them without duplicate imported
definitions. The runtime cannot be deployed by linking just one object. Existing
runtime and namespace2 catalogs remain immutable. The focused verifier uses real
C runtime reference objects; the product proof instead uses the qualified SG
linker, two SG algorithm objects, minimal C memory/fixture objects and pinned OS
import libraries. Any C metadata adaptation is explicit and byte-verified; real
unwind metadata is not stripped from a nonleaf function.

## Evidence

The runner snapshots consumed source bytes, captures C headers, records actual
tool/provider/report relationships, UTC/host/revision and dirty state, and guards
only that closure. Historical target discovery reports remain immutable. Current
checkout drift outside the consumed snapshot is reported separately. Tasks are
excluded from the algorithm closure to avoid a report/ledger cycle. Formal tests
include actual host owning pointers, three-byte and 16-byte aligned owning values,
OOM, generation retirement, lifecycle, state bounds, and deterministic SG objects
and SG-linked PE bytes. Peak memory measurements are observations, not M8 approval.
