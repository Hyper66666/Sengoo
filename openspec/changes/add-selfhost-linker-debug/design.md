# SG linker architecture

The linker accepts bounded immutable AMD64 COFF byte snapshots. Checked integer
readers validate file offsets, section/symbol/relocation/string-table extents,
names and supported record kinds before appending logical records. Object-local
static symbols retain their object identity; external definitions resolve by
their exact UTF-8 name. Missing definitions, conflicting strong definitions,
invalid relocation targets and unsupported record kinds fail with located
stable error codes. Input limits cannot turn truncation into valid output.

Logical input sections map to deterministic output sections and contribution
offsets. PE RVAs, raw-file offsets and preferred virtual addresses are separate
quantities. Relocation arithmetic checks the declared field width and AMD64
kind before patching. ADDR64 patches require base-relocation records whenever
the target is image-relative. Timestamp fields and construction order remain
deterministic; repeated equal inputs produce byte-identical unsigned images.

The initial PE32+ console path provides an explicit entry wrapper and a minimal
kernel32 ExitProcess import descriptor, lookup table and IAT. It does not hide
CRT initialization in an external linker. The wrapper calls the named SG entry
and exits with its integer result. Imports and generated stubs remain ordinary
linker-owned logical records. Native `.pdata`/`.xdata` contributions are retained
and their resolved function ranges are validated before publishing the image.

Each implementation stage records its precise input subset and failure bounds.
Auxiliary symbols, weak/common/COMDAT records, bigobj, archives/import libraries,
DLL/export/TLS/resource features and unsupported debug records are rejected
until implemented. The first checkpoint does not stand in for complete linker
compatibility or full M5 completion.

CodeView input/type/local/source linking and deterministic PDB generation are
owned here and remain mandatory M5 work. Debug completion requires a real
debugger to set a source breakpoint, step and inspect local values in an SG
linked image. An inspectable section, generated PDB header or configured test
is insufficient evidence. Exception/unwind behavior likewise needs native
execution beyond syntax/metadata inspection.

Primary format reference:
https://learn.microsoft.com/en-us/windows/win32/debug/pe-format

Windows unwind reference:
https://learn.microsoft.com/en-us/cpp/build/exception-handling-x64?view=msvc-170

PDB format implementation reference:
https://github.com/microsoft/microsoft-pdb

## Initial executable checkpoint

The first implementation is `link_model.sg`, `link_coff.sg` and `link_pe.sg`.
It accepts initialized AMD64 COFF with short/long UTF-8 names and no auxiliary,
weak/common/COMDAT/bigobj records. Strong external names resolve across objects;
static names remain object-local. Same-name initialized sections merge with
checked contribution alignment; general `$` subsection ordering is not yet
qualified. The state is a one-shot link session, preventing accidental reuse
after synthetic sections/resolution have been appended.

The PE32+ console image uses 4 KiB virtual and 512-byte file alignment,
preferred base 0x140000000, zero timestamps, NX/ASLR flags, a minimal explicit
KERNEL32.dll ExitProcess import/IAT, and an entry wrapper that calls the selected
SG function and exits with its integer result. ADDR64, ADDR32, ADDR32NB, REL32
through REL32_5, SECTION and SECREL are applied directly in SG. Image-relative
ADDR64 patches produce sorted DIR64 base-relocation blocks. `.pdata` records
are relocated, sorted, range checked and joined into the exception directory;
the wrapper has its own fixed-stack unwind record.

Limits are 128 objects, 1,024 contributions, 96 PE output sections, 100,000
symbols/relocations and 64 MiB data/image size. Error codes are 7201 (malformed
object/extent/name), 7202 (capacity), 7203 (unsupported input record), 7204
(unresolved entry/symbol), 7205 (conflicting strong definitions), 7206
(relocation range), 7207 (entry/unwind range), and 7208 (session reuse).
Detailed per-field input diagnostic offsets remain later work; this checkpoint
does not claim complete public linker diagnostics or CLI compatibility.

The named gate produces two COFF objects in SG, links them with a toolchain-free
PATH, and executes the SG-written PE with exit 42. Independent Python arithmetic
checks all tested relocation fields and PE directories. LLVM independently
inspects imports, base relocations and unwind data. A C Windows probe reserves
the preferred image address before loading the PE, verifies that both absolute
pointers relocate to the actual image base, and checks RtlLookupFunctionEntry
and RtlVirtualUnwind at multiple instruction offsets. Compiler/linker failures
preserve pre-existing output bytes. This is native executable evidence for the
bounded subset; CodeView/PDB, archives, general SDKs and full M5 remain open.

## Indexed archives and native import libraries checkpoint

`link_archive.sg` reads standard COFF archives with one big-endian first
linker member and an optional little-endian second linker member. Every
indexed member offset must identify a complete regular member header; the
second index must agree with the first. Long filename references identify a
validated UTF-8 string start in the archive long-name table. Supported linker
member NUL padding and long-name LF padding are bounded to one byte. Members
are extracted lazily for the entry and unresolved strong externals, with
repeated library passes supporting dependencies across archive order.
Unused regular member bodies do not need the selected COFF subset. Unsupported
selected auxiliary/weak/common/COMDAT/bigobj/static SDK bodies still fail.
Limits are 128 libraries, 65,536 archive members, 100,000 indexed names, 4,096
extractions/closure passes and 64 MiB archive/image storage.

`link_imports.sg` reads real AMD64 MSVC short import objects, including ordinal,
name, no-prefix, undecorate and export-as forms, and code/data IAT identities.
It validates exact payload lengths and NUL-terminated nonempty UTF-8 strings.
The SG linker groups imports stably by DLL, emits descriptors, contiguous IATs,
lookup/hint/name records and executable code thunks, and applies their RVAs
through ordinary checked SG relocations. Limits are 4,096 imports and 256 DLLs.
Const import type, delay imports, full import-descriptor COFF objects, archive
weak selection and ordinary static SDK compatibility remain unqualified.
The fixed entry wrapper still imports KERNEL32.dll ExitProcess explicitly.

The gate uses the installed Windows 10.0.26100.0 x64 kernel32.Lib unchanged;
it reads its real first/second indices and extracts only required short import
members for GetCurrentProcessId and GetCurrentThreadId. SG-generated machine
code calls one through a thunk and one through its `__imp_` IAT and returns 42
on Windows. Independent LLVM inspection verifies the selected imports. Another
fixture loads two DLLs and all supported name transformations, including an
ordinal obtained independently from the recorded native DLL export table.
The type-1 IAT fixture checks loader/address behavior without claiming a
complete SDK data-ABI corpus. Independently produced llvm-lib archives verify
lazy selection, entry extraction and cross-archive closure. Repeated links
must produce identical bytes and native exact-resource scopes cover both
successful SDK linking and malformed archive/import failure paths.

These are archive/import checkpoints, not complete static SDK or M5 evidence.
Full CodeView/PDB generation, real debugger source/local gates, DLL/TLS/resource
and native-package compatibility remain mandatory open tasks.
