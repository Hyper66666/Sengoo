# Compiler authority and runtime operations

The existing generic-collections and memory-management specifications remain
authoritative: arbitrary owning elements, correct layout, exact Drop, moves,
borrows and last-reachable-use lifetimes. Conservative standalone candidate
analysis is an intermediate boundary, not permission to change those contracts.

## Independent authority

Keep base SDK namespace one with its fixed sixteen entries. Introduce a separately
versioned collections provider namespace, with caller-pinned target, ABI,
manifest, physical source/header prototypes and selected object identity.
Authenticate actual defined exports and the complete build/header/link input
branch. Map provider declarations structurally into the program's actual
physical module, declaration, binding and callable types. IDs from different
AST/type arenas are never interchangeable.

The actual development provider is `tools/stdlib/runtime_collections.c`, with
prototypes in `runtime_shared.h`. It performs collection/slot/handle algorithms;
it is not a minimal memory shim. The Rust compiler's named-call lowering is the
reference compiler implementation, not the runtime provider. Missing provider
or source authority is explicit unsupported failure, without LLVM fallback.

Origin and loan schema-one candidates always retain `unlowerable = 1`. HIR6 may
own and serialize them, but decoding or changing a candidate bit never grants
runtime authority. Fresh compiler facts consume actual typed/module/ownership
inputs, independently supplied source/provider pins and validated candidates.
Every public lowering entry rechecks those relationships. Ordinary absent
candidates use an explicit empty form; missing authority cannot silently turn a
module containing intrinsic sites into an empty payload. Preserve builtin-call
width six, deferred-type width four and ownership actions eight/nine/ten;
old HIR5 must reject explicitly.

## Construction and callbacks

The canonical `vec_new<T>` source body is a zero-valued placeholder. Concrete
lowering must instead derive T's target layout and produce actual move/drop
callback functions and their physical signatures. Call the selected
`sengoo_raw_vec_new_parts(size, align, move, drop)` and construct the public
two-field Vec value with its actual result and zero marker. Logical origin does
not imply allocation succeeded: preserve zero/OOM and cleanup behavior.

Use existing MIR function-address opcode nine. A checked callback-erasure bridge
may use opcode twenty-eight, separately owned by the native-backend child; it
does not prove T or authenticate arbitrary callback bodies. Constructor facts
must additionally bind generated function bodies, layout, move transfer/source
clearing and exactly-once owning Drop. Ordinary casts must not manufacture that
authority. Scalar width, nonnull callbacks and descriptor flags zero do not
prove Copy. Keep the descriptor ABI one contract unchanged.

## Borrowed-next representation

The development iterator provider does not validate the semantic T or family of
an arbitrary opaque handle. Authorization therefore requires the actual same-T
canonical producer, backing owner and live loan, with invalidating mutation,
release, transfer and unknown native escape excluded. A physical handle
projection must preserve its actual receiver/field relationship. An ordinary
i64 parameter cannot acquire that relationship from its type or spelling.

Use a dedicated optional-borrow operation (reserved opcode twenty-seven) for
the actual selected provider's raw pointer result. Null constructs None with an
inactive payload; a valid nonnull result constructs Some under the source,
loan, provider and concrete element contract. Do not create a standalone zero
Ref or reuse String opcode twenty-six. Payload extraction/load must require a
dominating Some test on the same aggregate, or an independently checked active
payload transfer. Unsupported generic helper paths remain explicit failures;
inactive-payload use cannot be hidden behind ordinary aggregate extraction.

Legacy scalar wrappers may retain their public signatures. Their raw handle
parameters require explicit conditional physical-handle obligations discharged
at real canonical receiver projections, or separately admitted internal typed
wrappers. Shared-reference scalar field copying alone is not a return-loan or
provider certificate. Raw next and scalar copies must preserve original
iterator/backing cleanup. Other collection operations are admitted only after
their own concrete implementation and verification; reserving an operation ID
does not implement it.

## Qualification sequence

First qualify provider-to-program mappings and corruption rejection. Then qualify
real generated constructor/callback bodies with scalar, bool, owning and aligned
elements. Verify next null/Some, active-payload control flow and backing lifetime
using actual native resource observations. Bind candidates and fresh authority
through HIR6 and mandatory MIR metadata, with read-only validation, deep owned
clones, bounds, roundtrips and old-schema rejection. Clear-provider/origin/loan
controls must remove lowering authority rather than merely change report counts.

Finally run current compiler sources, accepted/rejected compatibility cases and
the full required constructor/get/push/set/remove/iteration path. A reduced
ownership diagnostic count alone does not qualify native compiler self-building.
M3/M4/M6 and final C-runtime replacement retain their complete independent gates.
