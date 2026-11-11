# Design

Implementation owner: compiler-source-snapshots child. Shared semantic module
wiring lands only after concurrently qualified producer closures are frozen.

## Representation and authority

`ShCompilerSnapshotArena` owns a Vec of immutable source records. Each record owns
a 1,024-byte aligned native guard buffer, frozen UTF-8 bytes, original metadata,
canonical DOS and volume-GUID paths, the stable entity identity and SG SHA-256.
The native struct is 536 bytes within that opaque extent, with eight-byte
alignment; native compilation and executed adapter checks establish that layout.
The guard owns one regular-file handle and at most 64 ancestor handles.
There is no process-global C source table or C snapshot owner registry.

The platform boundary implements path normalization, fixed-volume detection,
retained root-to-volume-GUID traversal, reparse/type checks, one `ReadFile`, one
metadata observation, final-path conversion and close. SG owns lexical path
policy, complete-read loops, EOF checks, metadata comparison, resource budgets,
identity deduplication, source admission and hashing. The existing SG
`runtime/sha256.sg` and `runtime/utf8.sg` are linked unchanged; only primitive
memory/pointer ABI operations remain native.

Metadata is ten 64-bit unsigned-field values: volume, file index high/low, size
high/low, attributes, creation time high/low and write time high/low. SG preserves
the existing identity spelling `%08x:%08x%08x:%08x%08x` so independently pinned
provider identities remain comparable. Metadata and copied bytes are captured
under the same retained file/ancestor ownership. No close-and-reopen check
substitutes for this owner.

Every source-authority check repositions that same retained handle, compares
the complete physical content with the copied buffer, observes EOF and metadata
again, and recomputes the SG SHA-256 digest. It also derives the identity and both
paths again from the retained handle. Mutable SG fields cannot authorize changed
source bytes merely by changing their recorded digest or metadata together.
Buffer capacity and used length are checked before raw source reads; rejected
copies leave the destination untouched.

## Budgets and duplicate admission

The module loader preserves its caller graph-limit range of 1 through 1,000,000.
Its separate compiler snapshot resource limit is `min(max_modules, 1024)`;
creating a small graph with a larger caller limit remains valid, while a 1,025th
unique retained source fails with the existing resource diagnostic 5204. This
does not claim that a million simultaneously retained modules are supported.
The graph preserves the caller's source-byte limit. The arena's independent
resource limit is `min(max_bytes, 64 MiB)`, without newly rejecting a larger
caller configuration. The actual native driver still passes 16 MiB, and that
budget is never silently enlarged. The arena additionally bounds stored
DOS/GUID path bytes to 8 MiB.
At most one probe guard is live beyond the unique-module limit: identity lookup
happens before rejecting a new unique file for capacity, so duplicates still
work when accepted capacity is full. Accepted guards never close during such a
probe. A duplicate must match the accepted retained witness before it is reused.

Native guard storage is at most 1,025 KiB, metadata is 80 bytes per accepted
record, and a read/hash workspace is 64 KiB + 1 KiB + digest/path buffers. Frozen
bytes obey the caller budget. Temporary byte/text conversions, graph text/AST,
Vec capacity and OS handle tables are separate measured resources; these figures
do not claim an RSS ceiling. Failures drop the probe and partial allocations
without changing admitted counts/bytes. Arena destruction closes every guard.

## Integration

A dedicated `selfhost/compiler_support` package supplies SG arena code and
`native/*.c` adapters through the existing seed package-native discovery, without
editing `tools/stdlib/runtime.c`. Actual discovery and source closure are tested.
`ShModules` owns the arena; module units retain an arena index. The loader,
unchanged-source check, HIR module witness, origin/liveness byte extraction and
MIR source length consumers must all use the new snapshot operations. No raw
legacy FileSnapshot handle may remain in that graph path.

## Qualification

The initial failure is the actual whole compiler source graph under a pinned SG
native producer. A target-only 32-to-128 macro counterfactual reaches semantics
5124 in `stdlib/file.sg`; it is diagnostic evidence only and does not qualify the
new implementation. New evidence must cover the actual compiler graph, 31/32/33
and full-capacity duplicates, alias identity, wrong/oversized input, fragmented
reads, missing EOF, metadata mismatch, guarded writer/rename/reparse controls,
first-call resource restoration and unchanged ordinary compiler regressions.
The formal gate builds its independent OS-probe DLL from the same captured C
sources consumed by the compiler build, and runs the source-integrity controls
through actual module/HIR consumers. Historical OS reports alone cannot qualify
different current adapter bytes.
The current runtime is single-threaded; no concurrent source mutation or hostile
privileged-filesystem immunity claim exceeds the existing public contract.
