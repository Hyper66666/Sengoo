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
