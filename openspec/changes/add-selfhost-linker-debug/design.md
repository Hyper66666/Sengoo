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


## SG-owned CodeView/PDB debugger checkpoint

`link_hash.sg` implements SHA-256 in SG using bounded 32-bit arithmetic;
`link_msf.sg` emits MSF 7.00 superblock, duplicate free-page maps, block map,
stream directory, stream pages and deterministic zero padding. The first
container limit is 16 MiB, 4,096 blocks and 256 streams, so additional FPM
intervals are rejected rather than emitted incorrectly.

`link_debug.sg` exposes checked source/function/line/local records and appends
a real PE debug directory and RSDS record. It grows the PE header table when
needed and adjusts raw section offsets without changing code RVAs. The PE/PDB
GUID and age bind the unsigned input image, source names and complete source
bytes, function/local/line records and PDB path through SG SHA-256. Source
checksums use CodeView SHA256 records and remain independently verifiable.
No timestamp, random identifier or external PDB producer participates.

`link_pdb.sg` emits the PDB Info/named-stream table, TPI/IPI, DBI module and
section contribution/map/file information, global source string table, module
symbols/C13 lines/checksums, PE section-header stream, symbol records and
public/global hash streams. Function symbols include a procedure type, frame
metadata, scoped local symbols and explicit SP-relative lifetime ranges.
The checkpoint supports one UTF-8 source module, 256 nonoverlapping no-argument
i64 functions, 4,096 i64 stack locals, 65,536 ordered lines, 1 MiB source and
16 MiB image/PDB output. Other local/type forms and multiple source modules
report explicit unsupported errors; malformed source/address/lifetime ranges
and conflicting names fail before successful output publication.

The debugger fixture constructs the corresponding x64 instructions in SG,
then the SG linker writes both PE and PDB with an unavailable toolchain PATH.
LLVM independently parses all named streams, line/checksum data and PE RSDS.
LLDB must set a source breakpoint, stop at line 3 with local answer = 40, step
to line 4 with answer = 42 and resume to Windows exit 42. The fixture keeps its
multiline source bytes because reformatting would change the address/line map.
A separate fixture forces PE header growth, and equal-length source comment
changes must change both checksum and GUID. SHA-256 is compared to hashlib on
1,024 deterministic messages, including padding boundaries and one million
bytes. Multi-page MSF streams are independently reconstructed and allocation/
free maps checked. Native scopes repeatedly exercise success and twelve
failure classes with exact tracked resource counts and preserve old outputs
on failed command invocations.

This establishes a usable SG-written debug artifact subset. Reading/remapping
arbitrary native COFF CodeView source/type/local records, multiple source
modules, native compiler automatic debug-address generation, full aggregate/
generic/async/reflection types and the complete M5 debugger corpus remain
open. These checkpoint APIs do not silently accept those unsupported records.

Debug format references:
https://llvm.org/docs/PDB/MsfFile.html
https://llvm.org/docs/PDB/PdbStream.html
https://llvm.org/docs/PDB/DbiStream.html
https://llvm.org/docs/PDB/TpiStream.html
https://github.com/llvm/llvm-project/blob/llvmorg-19.1.7/lldb/source/Plugins/SymbolFile/NativePDB/PdbIndex.cpp


## Relocatable COFF CodeView checkpoint

`sh_link_codeview_attach(object, debug)` writes real `.debug$S` and `.debug$T`
contributions before object emission. Function names resolve to existing COFF
function symbols; declared input section and offset must match those symbols.
Procedure addresses, local lifetime range addresses and source line fragments
use ordinary AMD64 SECTION/SECREL relocations against the function symbol.
No final PE RVA or section ordinal is baked into the input debug addresses.
The type contribution contains a signature and the supported argument-list/
procedure type records; symbols include compiler/module/function/frame/local
records and C13 line/string/SHA256 checksum subsections.

`sh_link_codeview_read(debug, linker, image, object_id)` reads the actual
relocated section contributions after SG PE linking, validates types, symbol
scope order, frame form, local lifetimes, ordered source lines, canonical UTF-8
source pathname and complete source checksum, and maps them into the PDB model.
Unknown debug sections/subsections/symbols/types and unsupported checksum or
frame encodings fail explicitly. The first reader accepts one source module
with the named no-argument i64 prototype and SP-relative i64 local subset.
Source path bytes must match exactly, so callers choose a canonical pathname
style consistently. Multiple modules, native compiler/SDK type merging and
other frame/local forms remain separate supported-corpus work.

The gate writes a real COFF object, independently decodes its CodeView and
relocations with llvm-readobj, loads the same existing object back through SG,
and requires equal PE/PDB bytes. The resulting PDB is independently inspected
and used by real LLDB for the source breakpoint/step/answer=40 then 42/exit=42
sequence. Nineteen malformed or unsupported COFF CodeView mutations fail while
preserving existing PE/PDB bytes. Twenty native success/failure scopes recover
all tracked resources exactly. Debug input contributions currently remain
mapped in the PE; debug stripping and the public non-debug CLI policy remain
integration work rather than implied full debug-input support.


## Fixed RBP frames and multiple-return local ranges

A function `epilogue` value of zero explicitly means that there is no single
common epilogue start; its CodeView procedure `DbgEnd` is zero and supported
local ranges are bounded by the complete function extent. Nonzero epilogue
values retain the initial single-tail contract. `sh_link_debug_local` creates
the first range and returns a local identity. `sh_link_debug_local_range(debug,
local_id, start, length)` appends a further ordered, nonoverlapping range for
the same SP-relative location. The model admits at most 65,536 extra ranges.
Each range must start after the prologue and end within its declared function
lifetime bound. Extra ranges participate in the source-bound debug identity.
The PDB writer emits one S_LOCAL followed by multiple defrange records, and the
COFF writer/reader relocates, validates and preserves those separate ranges.
Changing a local's SP base between ranges is explicitly unsupported.

Native callers split ranges around every SP-changing epilogue and begin them
after definite initialization. An RBP function can still use the SP-relative
location model while the stack pointer is stable: local SP offset equals the
allocated frame size minus the RBP slot offset. The frame metadata includes
saved nonvolatile stack slots; the PDB writer accounts separately for the
return address. It must not use the first return as the entire function's end.

The named RBP fixture saves RBP, establishes a fixed 48-byte frame, has two
returns, and describes answer at SP+40 over [19,39) and [44,56). A real debugger
observes 40 then 42 while stepping through its body and reports the local as
unavailable after entering the first epilogue. A Windows probe checks
RtlLookupFunctionEntry and RtlVirtualUnwind at eleven offsets spanning partial
prologue, function body, and both epilogues, restoring caller RIP/RSP/RBP.


## Automatic native compiler debug mapping checkpoint

`link_native_debug.sg` deliberately imports MIR validation and the CodeView
writer rather than the native backend. `ShNativeDebugMap` is a checked adapter
contract consisting of flat records: functions [MIR function, absolute text
start, end-exclusive, frame metadata bytes, relative prologue end, input COFF
section]; instructions [MIR instruction, absolute start, end-exclusive];
terminators [MIR terminator, absolute start, end-exclusive]; locals [MIR local,
SP offset]; and epilogues [MIR function, absolute start, end-exclusive].
The native backend's optional metadata collector supplies those facts without
changing ordinary object bytes. Its frame allocation plus saved-RBP slot forms
the debug frame metadata; local SP offsets are allocation minus RBP slot.

The adapter normalizes map order with bounded bottom-up merge sorting, validates
complete unique MIR identities, function/type/source boundaries, disjoint
instruction/terminator code intervals and return-to-epilogue correspondence,
then builds source line indices once and uses binary lookups for source spans.
Function/local/line records are derived from MIR spans and names and actual
native code offsets; no fixture machine addresses are supplied to the adapter.
An i64 local is initially visible only after a direct entry-block store that
dominates reachable blocks. Additional ranges split at every recorded epilogue.
Unsupported lexical scopes, unproven branch-only initialization, moves/storage
invalidation, parameter/other function prototypes and non-i64 named locals fail
explicitly. The initial contract supports one unchanged UTF-8 source file,
no-argument signed-i64 functions and named signed-i64 stable stack locations.
No macro-expanded source coordinate remapping is claimed by this checkpoint.

The automatic gate runs SG frontend, semantics, MIR lowering, native object
construction, CodeView attachment, SG object/PE linking, relocated CodeView
reading and SG PDB generation under an empty toolchain PATH. One source has a
preceding function so main's debug addresses are nonzero; another has two return
paths and native RBP frames. LLVM independently inspects actual COFF CodeView
and PDB records, while LLDB sets source breakpoints, steps and observes local
answer = 40 then 42 before Windows exit 42. Repeated object/PE/PDB bytes and
complete source-pipeline resource scopes are checked. Malformed maps preserve
all prior outputs; unsupported source/type/local forms produce no artifacts.
Errors 7311/7312/7313 identify malformed mapping/span, capacity and unsupported
automatic mapping respectively; underlying MIR/CodeView diagnostics retain their
own stable codes. This remains a primitive mapping subset and does not qualify
the complete M5 type/module/SDK/debug corpus.


## Primitive CodeView signatures and typed parameter/local records

`link_cv_types.sg` owns a bounded primitive type pool rather than assigning one
fixed function prototype to every procedure. It interns LF_ARGLIST and
LF_PROCEDURE records deterministically, validates their counts, builtin kinds,
calling convention and backward argument-list references, and returns an
explicit input-to-canonical type-index map when reading a COFF type stream.
Limits are 64 parameters, 4,096 type records and 1 MiB type-record bytes.
Malformed counts/references, capacity overflow and unsupported leaves/kinds use
7321, 7322 and 7323; the CodeView input reader retains its 7301/7302/7303 family
when exposing those failures through its object-input API.

The debug model supports primitive void results, bool, signed/unsigned 8/16/32/
64-bit values, Unicode char32, f32 and f64. Typed function records refer to the
interned prototype, and locals retain their actual builtin type. Parameters
carry S_LOCAL's parameter flag and must match the declared prototype in number,
order and type. Their stable stack lifetimes begin after the native entry store
and use the same checked, disjoint range mechanism as ordinary locals. The
native adapter maps MIR builtin types and ordered parameter locals into these
records; the PE console main contract remains signed-i64() for the explicit
entry-result wrapper. Other compiled functions can use supported scalar
parameters/results. Native stack locations retain their minimum eight-byte
slot contract; arbitrary packed SDK frame layouts remain separate work.

The COFF writer serializes the actual type pool. Its reader remaps procedure
references, checks local/parameter flags and scope/prototype agreement, and
writes the corresponding PDB TPI records. Unknown aggregate/pointer/type-server
leaves, reserved calling-convention forms, variadic prototypes and unsupported
local types remain explicit failures. No copied fixed i64() prototype is used
to represent a differently typed function.

A separate SG fixture calls double(double,double), writes real typed COFF and
PDB records, and proves via LLDB that both parameters are 20 and a double local
steps from 40 to 42 before exit 42. One hundred type-pool remap/resource scopes,
twenty complete typed PDB scopes per invocation, repeated identical bytes and
ten malformed prototype/parameter cases complement the earlier object/debug/
unwind corpus. Automatic source fixtures additionally exercise real SG f64
parameters/results/locals and bool/i32/u64 parameter values when the qualified
frontend/MIR/native interfaces are synchronized. Complete aggregate/generic/
module/SDK debug support still retains its own open gates.

Primitive CodeView type reference:
https://github.com/llvm/llvm-project/blob/llvmorg-19.1.7/llvm/include/llvm/DebugInfo/CodeView/TypeIndex.h


## Pointer, reference and const modifier types

The type pool accepts the supported LF_MODIFIER const form with canonical
LF_PAD2/LF_PAD1 padding and LF_POINTER Near64 forms of exactly eight bytes.
Pointer mode and lvalue-reference mode are distinct. Immutable SG references
qualify the referent with const before forming the reference; mutability does
not become an incorrect const flag on the pointer itself. Raw void pointers
are representable; references to void, references to references and pointers
to references are rejected explicitly so C++ reference collapsing cannot
misrepresent an SG type. Raw/ref chains have a bounded 64-step type validation,
including const modifier wrappers. Constructors reserve their new outer layer
when validating the base, so every returned type is immediately usable;
attempts to add the 65th wrapper fail before the pool is changed. Short pointer
records fail the record-length check before any attributes are read.
Unsupported pointer modes/sizes/options and forward/dangling type references
fail before output publication. Aggregate and function pointer targets retain
separate open qualification work.

CodeView argument/result/local references are remapped into canonical type
indices before prototype and parameter validation. The native adapter maps
supported scalar pointer/reference types recursively and preserves stable
stack locations and source-derived lifetimes. Readonly and mutable reference
fixtures require real debugger values, not just inspectable type names.
A raw pointer fixture obtains its value from GetCurrentProcess through an
explicit unsafe C declaration and an SG-owned KERNEL32 import binding, then
observes the opaque pointer bits without dereferencing the handle. It retains
the existing frontend's FFI/cast rules rather than inventing an integer cast.

An independent clang C++ const-i64-reference object supplies its actual first
four type records. SG reads, remaps and rewrites that prefix byte for byte;
64 valid duplicate records canonicalize to the same prefix. The unimplemented
remaining native type leaves in the complete clang stream fail explicitly.
This exact format comparison is independent verification, not an external
compiler participating in product PE/PDB generation.

Pointer record reference:
https://github.com/llvm/llvm-project/blob/llvmorg-19.1.7/llvm/include/llvm/DebugInfo/CodeView/TypeRecord.h

## Bounded aggregate debug types

The SG type pool writes and remaps LF_FIELDLIST with public LF_MEMBER or
LF_ENUMERATE entries, LF_STRUCTURE, LF_UNION, LF_ARRAY and the supported
unsigned-64 LF_ENUM form. Structure members must fit their declared size in
nonoverlapping offset order; union members all start at zero and fit the union.
Member names are unique within a list. Counts, sizes, backward references,
NUL-terminated UTF-8 names and LF_PAD bytes are checked before publication.
The numeric subset uses canonical immediate unsigned values or LF_ULONG;
negative/other numeric leaves and unsupported properties (including C++ forward
references, unique names, inheritance and methods) fail explicitly. It does
not accept a complete arbitrary native C++ type stream.

Bounds are 256 members per field list, 1,024 UTF-8 bytes per name, 16 MiB per
represented aggregate/array, 4,096 interned records and 1 MiB type bytes.
The existing 64-layer cap now includes aggregate/array wrappers. A per-record
depth cache accounts for shared subtypes without recursively expanding an
exponential DAG. Array byte sizes must be exact multiples of nonzero element
sizes; references to field lists and procedure records are not value types.
Local validation includes the entire represented byte extent in the fixed
stack frame, rather than just checking its starting address.

MIR retains aggregate field types/offsets but not their declared names.
ShNativeDebugMap therefore adds checked flat quadruples
`(mirType, variant, fieldOrdinal, nameSymbol)`. A variant ordinal of -1 denotes
a struct; an enum field ordinal of -1 names the variant. The independent
link_checked_debug adapter obtains these names from ShChecked layout bindings
whose symbol indices are retained by MIR. The map rejects malformed widths,
unknown types/variants/fields/symbols and duplicate entries, with a 4,096-row
bound. The native emitter has no debug module dependency and needs no changes.
Fresh map caches memoize supported MIR type heights and emitted CV indices;
cycles, generic nominal types, class layouts and function targets remain
unsupported. Tuple member names are `_0`, `_1`, and so on.

The adapter reuses sh_target_layout for array strides, tuple padding and named
field offsets. Nongeneric enums are represented by their real eight-byte
`__tag` and `__payload` union at byte eight, with named variant payload structs.
Tag enumerators carry the actual MIR variant ordinals. This is a physical
tag/union view; conditional active-variant display, inactive-payload
initialization and a language-specific debugger pretty-printer remain open.
Real LLDB fixtures inspect a known initialized Value payload, and do not infer
meaning from inactive union arms.

Native qualification snapshots the complete SG entry/import closure and the
whole standard library/runtime.c before building. The source and snapshot
hashes, producer command/tool hashes and full revision interval are recorded.
Module maps and source-runtime overrides point to those snapshots. Later
checkout edits are reported separately and cannot silently replace a producer
input; snapshot/runtime-Rust/tool changes still fail the run. Independent
format probes use clang only as an oracle, while product source compilation,
object emission, PE linking, CodeView remapping and PDB writing remain SG.

Aggregate record layout reference:
https://github.com/llvm/llvm-project/blob/llvmorg-19.1.7/llvm/lib/DebugInfo/CodeView/TypeRecordMapping.cpp

## Multiple physical sources in one compilation unit

One SG-produced object continues to correspond to one PDB module. Its C13 file
checksum table contains a separate SHA-256 record for every physical source;
line file-block NameIndex values reference byte offsets in that table, not
file ordinals. The COFF F3 string table, PDB /names and DBI module FileInfo all
retain the physical paths. This scope does not implement independent COFF
input modules, inline origins or language-specific source pretty-printing.

The debug model keeps bounded file records and separately delimited byte
storage, with explicit physical MIR file IDs, per-function file ownership and
per-line file references. Counts are at most 256 files and 256 functions, each
source is at most 1 MiB, and combined source bytes are at most 8 MiB. Source
text comes from the same module graph snapshots used by semantic checking and
physical-MIR rebasing; the driver verifies the files remain unchanged before
publishing any artifacts. Native code offsets and local stack ranges remain
the emitter's actual facts. Existing single-source APIs retain their sole-file
defaults; the single-source identity encoding is retained for byte comparison.

S_GPROC32 has no source-file field. A reader cannot detect replacement of an
entire function's only file block with a different otherwise-valid file from
the COFF line record alone. Therefore the supported multiple-source reader
requires a caller-supplied function-name to physical-MIR-source registry before
parsing symbols. The native adapter derives it from validated typed MIR; the
reader checks every file block against that independent registry. It never
infers the registry from the record being verified. Missing, duplicate,
unresolved or excessive registry entries fail explicitly. Third-party
multiple-file COFF without that contract remains unsupported. The initial
production subset allows one physical source per function and rejects inline
or otherwise mixed-file function fragments instead of guessing ownership.

The target-only proof is frozen at
`target/selfhost/multi-debug-poc/freeze.json` (SHA-256
`3f3a6ae407ace41902c788e0e72fc7aa198f9c25d6c96796fa45aa9183a4003d`).
Its 8-command report
`target/selfhost/multi-debug-poc/evidence.json` (SHA-256
`b490257a0ee604ca37d1fb5504a5c9c672675d4eaff021c47c52d9814be75507`)
proves one PDB module with two separate file checksums, actual source
breakpoints, main-to-worker stepping, local values 40/42 and Windows exit 42.
Production admission requires the additional Unicode-path, registry/reader
negative, first-call resource and previous complete regression gates; the
prototype does not close the corresponding production tasks.

The production follow-up now passes the four reports named in the tasks:
44-command multiple-source qualification, 93-command pure CodeView regression,
79-command real-source native regression, and an independent Python binary
oracle. The public internal entry points are
`sh_link_debug_source_file(debug, physicalFileId, path, bytes)`,
`sh_link_native_debug_fill_sources(debug, mir, map)` and
`sh_link_native_debug_register(debug, mir)` before COFF decoding. The legacy
`sh_link_debug_source` and `sh_link_native_debug_fill` wrappers remain available.
All nine prior one-file COFF fixtures remain byte-identical. PE/PDB identities
include the output PDB path, so comparison at different output locations
checks every remaining byte after explicitly accounting for that path,
derived identity and copied debug-section extents; it does not assert raw
PE/PDB equality. Independent COFF modules and mixed-file function origins stay
outside this admission.


## Independent supported SG COFF debug units

This admission keeps each COFF contribution in its own `ShLinkDebug` until
its physical source registry, local type namespace and relocated code facts
pass. `ShLinkDebugModules` binds an ordered set of at most 32 all-debug SG
objects to an immutable linked-image/name/section/symbol/group snapshot. Every
actual COFF object must be represented once; missing, duplicate, unordered or
unsupported units reject. C/SDK objects without debug input, archive unit
selection and type-server forms are not admitted by this contract.

`sh_link_debug_modules_begin` fixes the image and RSDS path;
`sh_link_debug_modules_read` consumes one independently registered fresh unit.
A reader failure poisons the program and repeated reads reject without further
mutation. Successful units are copied into flat owned module storage. Their
source/file IDs remain local to their module; registries are never combined
before the unit check. A same physical path is interned globally only when raw
bytes agree; each module retains its own DBI file contribution and checksum-row
order. The source registry is derived from validated typed MIR, never inferred
from the line block being checked.

A fresh local-to-global type map imports each unit into the program-wide TPI.
Serialization patches procedure/local type references without changing the
unit's local types. Two objects may both use type 4098 for unrelated records.
The target fixtures also use same-named `Local` structs with different real
member names, whose program TPI indices and debugger views must remain distinct.
Static procedures are checked against their own COFF storage/address, use
S_LPROC32 and module-specific S_LPROCREF records, and do not become publics.
External functions alone enter the public address map. The test caller marks
its same-named helper symbols storage 3 after native emission; native machine
addresses are unchanged. Automatic compiler visibility/internalization is a
separate concern and is not claimed by that fixture.

Streams 0..5 retain their fixed meanings; N separate module streams start at
6, followed by section headers, symbol records, GSI and publics. DBI contains N
module descriptors, actual per-object contributions, dynamic stream indices
and per-module file associations into a deduplicated physical-name table. Each
module's F2 references remain byte offsets into its own F4 table; its F4 path
indices point into the shared /names buffer. Procedure references carry the
one-based owning module and that module's symbol byte offset.

Completion seals all ordered module/source/registry/visibility/type/symbol/line
bytes and bound code facts. Export validates the seal, bound link metadata,
full type pool, each independently reconstructed module's symbols/lines/source
checksums and image ranges before serializing. Mutated or incomplete programs
fail before publishing outputs. One all-global unit delegates to the legacy
writer with the same image/path and retains its exact raw PE/PDB bytes.

Two fresh artifact directories use the same explicit logical module names
`sg-main.obj`/`sg-worker.obj`, actual physical source paths and fixed RSDS path.
This is an equal-input/options determinism test. Logical object identities do
not replace physical source identities. It does not prove different source
checkout roots, path remapping or M6/M8 reproducibility. This admission retains
256 total functions, 256 unique files, 1 MiB per file / 8 MiB combined physical
source, 4,096 type records / 1 MiB and a 16 MiB MSF bound. Full SDK/debug support,
inline origins and complete M5 remain open pending their own evidence.


The production evidence for this bounded independent-unit contract is
`target/selfhost/m5-independent-units-checkout-bound/evidence.json`, SHA-256
`3c271fba6c7d22fb273ee19753b343f86fba50d8fe3edc8403a2016fc55af2a0`.
Its 109 commands include 52 SG rejection/resource cases, nine exact legacy
single-unit PE/PDB comparisons, two true independent-object programs, and two
fresh artifact directories per program. One variant deliberately gives both
objects different LF_FIELDLIST records at local type 4098; its distinct Local
structs remain global TPI IDs 4099 and 4101. Independent byte inspection checks
module stream indices, global member types/names/offsets, two static helper
references, public exclusion and shared physical-source associations. Four
malformed-byte negative controls verify that inspector. LLVM and real LLDB
confirm two independent modules and source/stack/local behavior. Full revision
`efed463dc06be086d97718089e5d4cc1322333e0` is recorded at both ends with no
changed captured sources, tools, Cargo configurations or original checkout
inputs. Complete previous 93/79/44-command debug gates also pass as listed in
the tasks. These bounded results do not close the broader exclusions above.
