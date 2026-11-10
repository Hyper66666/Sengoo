# Tasks

- [x] Read canonical generic-collections/runtime contracts and actual family proof.
- [x] Register this unique algorithm owner separately from namespace2 lowering.
- [x] Implement SG registry lifecycle, context identities, retirement, and validation.
- [x] Implement the single SG RawVec algorithm and primitive-only platform adapter.
- [x] Execute focused real-reference differential and lifecycle/failure controls.
- [x] Observe reference iterator-handle allocation failure separately.
- [x] Execute two fresh qualified SG-linker deployments and compare exact bytes.
- [x] Freeze sources, audit complete source/tool/artifact/receipt closure, and record evidence.

## Qualified checkpoint

Final report: `target/selfhost/runtime-collections-formal-03/evidence.json`, SHA256
`2ad356a7ad4eb7527283e9c7790d57e39cdbcb70b47c154ed8a01d6a0867a60d`.
Actual start/end revision: `3542403939a61daafafa8b996441da0e5fa0dbe6`;
host `Windows-11-10.0.26300-SP0`. All source/external/tool/checkout guards are empty.
The report binds 1,368 source inputs, 218 external inputs, 41 tool identities,
328 artifacts and 45 actual commands (90 stdout/stderr receipts).

The actual verifier completes all 23 named fixture IDs and 50,000 deterministic
operations across five element shapes. It checks real C callback order/effects,
owning eight-byte and aligned 16-byte values, three-byte values, existing-prefix
preservation after push/insert growth OOM, initialized metadata reads, read-only
validation, exact not-found output canaries, slot exhaustion, identity/generation/
kind rejection, maximum-generation retirement, lifecycle and double disposal.
Backing-free iterator done/next behavior is checked against actual C too.
The same-identity two-context negative control deliberately demonstrates the
documented unsafe caller precondition; it is not counted as an input the runtime
can distinguish or reject.

The real C reference iterator calloc allocation leaks 16 bytes if subsequent
opaque-handle allocation fails. The verifier observes the allocation and missing
handle separately, then releases that known leaked fixture allocation itself.
The SG slot-exhaustion path performs no iterator heap allocation. This resource
correction is explicit, not a claim of complete reference parity.

An independent actual Windows memory test completes 100 scopes: 700 payload
VirtualAlloc/VirtualFree pairs, 800 attempts including 100 injected OOM results,
20,100 owning-element drops, no live payload/token allocations, and every free
verified with VirtualQuery `MEM_FREE`. Dispose zeroes all 12,352 context bytes.
Observed payload peak is 6,144 bytes and working-set peak 4,276,224 bytes for this
test; neither is an M8/general memory bound. The largest inspected single native
SG frame is 7,824 bytes, separately from caller state, with the qualified actual
NTDLL stack-probe import in the SG-linked product.

Two fresh directories produce equal registry/vector/smoke SG objects, explicitly
adapted leaf C SDK objects, and complete PE bytes. Both qualified SG-linker
products execute with empty PATH, return 42, and emit no stdout/stderr. C metadata
adaptation preserves actual text/data/relocations/external symbol addresses; no
real nonleaf unwind is stripped. Actual loaded memory providers and compile/link
header/library dependencies are source-bound.

Historical passed trials `runtime-collections-trial-01` (38 commands),
`runtime-collections-formal-01` and `runtime-collections-formal-02` (41 commands
each) remain immutable. Later reports bind the exact formatting/test/resource
enhancements instead of rewriting earlier report hashes. The earlier target-only
Vec-family discovery remains development evidence, not current provider admission.
OpenSpec validation at this checkpoint: child strict PASS; initial all 110 items
PASS, and terminal all 111 items PASS after an independent child was registered,
zero failures. Terminal audit rehashes every final manifest/receipt and all 11
owned files; the ledger itself is excluded from the algorithm report closure.

Remaining integration: process-global shared registry and other runtime families;
full provider/catalog admission; compiler/stdlib source lowering and compatibility
reads; installed SDK packaging, complete self-build, and M8.
