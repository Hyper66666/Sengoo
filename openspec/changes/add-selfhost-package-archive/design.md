# Tar scan and package integration

## Concrete interface

`sengoo_sg_tar_scan(source, length, table, table_capacity, paths, path_capacity)`
is an unsafe C ABI function with primitive byte pointers and i64 extents.
The caller supplies mutually disjoint readable input and writable output
extents, stable for the entire call. Input is bounded to 512 MiB independently
of the current gzip decoder's 64 MiB output bound. The table has a 32-byte
little-endian u64 header (schema=1, count, effective content sum, consumed
offset), followed by 48-byte records (header offset, data offset, effective
size, UTF-8 path offset, path length, type: file=0/directory=1). Path bytes are
caller-owned and have no trailing NUL. Success returns count <=10,000.

Errors are -1 malformed/truncated/checksum/numeric, -2 insufficient output
arena, -3 invalid API extent, -4 unsafe path, -5 unsupported entry type,
-6 duplicate or file/directory prefix conflict, -7 entry/content/metadata
budget. Errors can leave a prefix in the output; the caller MUST discard all
metadata after any negative return. Header publication occurs only at success.

## Format and policy

Read old, GNU and ustar headers, unsigned checksums, bounded positive octal
and base-256 sizes, 512-byte framing, GNU long-name records and local PAX
records. PAX path and effective size are interpreted; GNU long-name takes
precedence over PAX path, matching tar 0.4.46. Real entries are only type NUL,
'0' and '5'; hard/symbolic links, sparse, devices and global PAX are rejected.
Metadata is bounded separately: 1 MiB per extension and 16 MiB aggregate;
normalized paths are at most 65,535 UTF-8 bytes. Effective content, rather
than a stale underlying header size, counts towards 256 MiB. At most 10,000
real entries are accepted. Directory nonzero content is rejected explicitly.

Normalize slash/backslash and discard empty/dot components; reject parent
components, absolute/UNC/drive/colon paths, NUL, invalid UTF-8, control bytes,
Windows-invalid characters, trailing spaces/dots, DOS device names and the
three sgpm root checksum/metadata marker names. Compare duplicate paths and
prefix conflicts with Windows ordinal case-insensitive semantics, using SG
UTF-8/scalar decoding and an OS-only CompareStringOrdinal adapter. A file
cannot be an ancestor of another record in either order. This is a deliberate
safety strengthening of the existing resolver's narrower normalization.

The selected Windows comparison is an observed OS ordinal relation, not a
claim of universal Unicode casefold or every filesystem's identity. Later
staging must use exclusive creation and prevent reparse/8.3 alias overwrites;
logical scan metadata alone does not prove those filesystem obligations.

Terminal behavior and PAX corner cases require independent Python/Rust
observations before claiming parity. Current Rust stops at its first zero
block or exact header EOF; a strict scanner may reject nonzero tails and
missing end blocks with a named compatibility ledger rather than silently
claiming equivalent archive acceptance. No filesystem safety follows without
the later staging and publication transaction.

## Integration boundary

sgpm budgets are 64 MiB compressed input, 256 MiB cumulative unpacked file
content and 10,000 entries. Tar headers, padding and extension data are not
file content and have separate limits. The scan's 512 MiB addressable input
is not a replacement gzip streaming design. The current gzip decoder cannot
materialize every valid package; a later chunk/state decoder must enforce the
256 MiB content budget while draining header/footer CRC and all tar content.

Primary format references: GNU tar Standard/Extensions and POSIX pax. Actual
behavior is additionally compared with this repository's pinned tar 0.4.46,
package.rs GNU builder and resolver.rs staging/manifest checks. Reference
algorithms are independent oracles only, not implementation providers.

## Development C SDK and unwind

The memory/OS adapter is built as a small no-entry/no-CRT DLL with only an
explicit pinned Kernel32 import. Its non-leaf CompareStringOrdinal adapter
keeps the real compiler-generated .pdata/.xdata. The formal gate binds actual
C/header/tool/object/DLL/import-library bytes, OS provider file and RVA, a
registered RtlLookupFunctionEntry range, and a live debugger backtrace after
the adapter has allocated its real stack frame. No unwind is stripped from
this provider. The pure SG linker consumes its import library, not a hidden
foreign parser or an unwind-less replacement object.

Only the stack-free test fixture object undergoes metadata normalization.
All real section bytes, flags, relocations and non-metadata symbols/addresses
must remain identical. Two fresh builds compare SG objects, normalized leaf
fixture, actual provider DLL/import-library and SG-linked PE bytes. This is a
development SDK dependency, not a final compiler-free SDK packaging proof.
The provider's raw COFF timestamp is recorded; its normalized object changes
only header bytes 4..7 to zero, with all code, unwind, data, symbols/auxiliary
records and relocations unchanged. Raw object byte equality is not claimed.

## Incremental scanner and extraction transaction (implementation owner: selfhost_core)

The new incremental scanner reuses `tar.sg` numeric and path helpers without
changing the qualified synchronous scanner. Its caller-owned workspace is
18,438,400 bytes: 1,024 prefix/control bytes, 512 header bytes, 1,048,576
extension bytes, two 65,536-byte pending-name buffers, 480,000 entry-record
bytes and 16,777,216 namespace bytes. Initialization writes only the control
prefix. Header, extension and namespace reads require their initialized
prefixes. Reset invalidates the namespace; disposal invalidates the state and
does not promise to erase file-name bytes. There is no heap or external handle
inside this state. Copying a live state is unsupported. The unsafe caller must
provide disjoint stable state/input extents and must not mutate a live state.

`init/reset(state,18438400)`, `step(state,18438400,input,length,finish)`,
`query(state,18438400,field)`, `ack(state,18438400,result)` and `dispose`
use primitive C ABI parameters. Step returns 0 (need input), 2 (finished),
3 (entry event), 4 (data event), or the scanner's negative error. Query fields
0..6 expose per-call consumption, cumulative wire/content/entries, status,
phase and workspace size; 7..13 expose current path offset/length, kind,
declared size, data offset/length and entry ID; 14 is delivered file content.
Events require an explicit acknowledgement before another step. Data points
into the current caller input only until acknowledgement. Failed I/O (-8) or
cancellation (-9) latches failure. Invalid API arguments return -3 before
touching buffers; uninitialized state is -10. Finish is sticky. Two zero
blocks and zero-only terminal padding are not publication: the scanner must
also receive finish after the gzip framing core has validated every footer.

The extraction pump feeds bounded compressed and decoded chunks through the
existing gzip continuation and this scanner. The configured limits are 64 MiB
compressed wire, 512 MiB tar wire, 256 MiB declared file content, 10,000 real
entries, 1 MiB per extension and 16 MiB aggregate extension/name storage.
These are separate budgets. Workspace size is not an RSS qualification.

The filesystem adapter contains memory and Windows handle operations only.
SG selects normalized path components, creates parents, writes data events,
checks entry completion, drains gzip framing and requests publication. A
session uses exclusive private-root creation and handle-relative child opens
with reparse traversal disabled. It retains handles for every created object
during extraction. SG requests file flushes and the adapter closes child
handles before root publication. If rename fails, cleanup reopens only
recorded components and requires their original volume/file IDs. Cleanup
deletes only those objects in reverse order and never recursively enumerates
an arbitrary caller path. Publication is a single same-parent,
no-replace directory rename after complete gzip and tar validation. Existing
targets are preserved. Disk error, cancellation, late CRC/member failure,
reparse point or a target appearing before rename must leave no partial final
tree. The parent directory is a caller-admitted trusted handle; this contract
does not authorize following an untrusted textual parent path. A same-user OS
adversary with write access to this admitted private parent is not a sandboxed
principal. Zero sharing on a directory handle does not prevent creating a new
child by pathname. Independent-process controls intentionally insert an unknown
child/junction or replace a recorded file; cleanup preserves unknown/changed
objects and returns -11 for a private residue. This is an explicit recoverable
failure, never permission to recursively delete that residue. No final target
is replaced by these controls. The 32,768 created-node handle bound includes
implicit directories and is an additional development resource limit.

The host API takes exactly one admitted parent HANDLE and two 1..64-byte ASCII
components made of letters, digits, underscore and hyphen, excluding DOS names.
It does not take two independent absolute destination paths. SG rejects equal
stage/final components under ASCII case-insensitive comparison before creating
the private root. All create and rename operations use the same parent handle;
its actual volume/file ID is recorded before and after extraction. FILE_CREATE
never reuses an existing object, including a pre-existing plain ASCII 8.3 alias.
The preview rejects a leaf reparse parent. Intermediate aliases may reach the
same admitted physical parent; the single handle/component contract still
applies. Supporting arbitrary Unicode host basenames is outside this preview.

This byte-extraction transaction is distinct from resolver admission. Full
`Sengoo.toml` parsing, expected package identity, registry/checksum metadata and
the public sgpm command remain separate explicit integration obligations.
Neither a successful scanner nor manifest-file presence alone satisfies them.
