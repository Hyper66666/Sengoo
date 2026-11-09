## Checked input and resolution

- [ ] Implement bounded AMD64 COFF section, symbol, relocation and string-table reading.
- [ ] Verify malformed extents, symbol conflicts, unresolved names and unsupported record rejection.
- [ ] Implement external/static resolution across multiple object files with deterministic identities.
- [ ] Implement archive and import-library reading, weak/common/COMDAT selection and required native SDK compatibility.

## Native PE images

- [ ] Implement deterministic PE32+ layout, imports/exports and checked AMD64 relocation application.
- [ ] Execute an SG-linked multi-object Windows EXE and verify independent image inspection.
- [ ] Preserve and execute unwind behavior through linked `.pdata`/`.xdata`.
- [ ] Implement supported DLL, TLS, resources, native package and SDK link behavior.
- [ ] Verify byte equality on repeated links, invalid input/output limits and exact resource recovery.

## Debug and integration

- [ ] Implement CodeView source/type/local linking and deterministic PDB generation.
- [ ] Prove real debugger source breakpoint, stepping and local-value inspection in SG-linked images.
- [ ] Verify supported native ABI/ownership/async/reflection/Python/native-package corpus through SG linking.
- [ ] Record full revision/host/tools/inputs/commands/results and validate this change and all specs strictly.
- [ ] Close M5 only after every supported linker/debug gate passes; keep M6-M8 self-build/performance integration separate.

## Initial executable checkpoint evidence

- [x] Read the named initialized AMD64 COFF subset with bounded tables, UTF-8 long names, object-local static and cross-object external identities.
- [x] Apply the named ADDR64/ADDR32/ADDR32NB/REL32..REL32_5/SECTION/SECREL fixtures and directly emit a deterministic PE32+ console EXE with ExitProcess import/IAT and entry wrapper.
- [x] Execute the SG-linked multi-object fixture with exit 42, independently inspect its PE fields/imports/base relocations/unwind data and verify forced rebasing plus Windows runtime unwind.
- [x] Verify 25 malformed/unsupported/conflicting/overflow/entry/unwind negatives preserve existing output, and 50 native success/failure scopes recover String/Buffer/opaque/FileSnapshot resources exactly.
- [x] Validate the child strictly after creating its four required documents; retain full M5 tasks above until their own complete gates pass.

Evidence: `target/selfhost/m5-linker-checked/evidence.json`, SHA-256
`2f0b0488345583e336438d1fc49b9cffde50b8613ddcc8b51f82ceaca337bb30`.
The report records identical start/end revision
`18873cb6d7396394358acd3bd8d764e1bab81417`, Windows 11 AMD64, verified seed,
formatter, clang and independent inspector identities. All 172 input files and
tool identities were unchanged. Thirty-eight commands passed, including the
native build, exact resource scopes, repeated equal PE bytes, real exit 42,
independent inspection and forced image rebasing/unwind. SG object emission and
link commands ran with a PATH containing no compiler/linker tools; external
tools built only the independent verification probe and inspected the image.

The initial failed trial remains under `target/selfhost/m5-linker-final/`.
Its bad-pdata overflow fixture correctly reported relocation overflow 7206;
the final gate keeps that check and separately tests an in-range RVA pointing
outside code, which reports 7207. `m5_complete` remains `false`; full archives,
import libraries/SDKs, DLLs, CodeView/PDB and debugger evidence remain open.


## Indexed archive and SDK import checkpoint evidence

- [x] Validate standard COFF archive first/second indices, bounded member extents, long UTF-8 filenames and supported padding; lazily select required members across library order.
- [x] Parse AMD64 MSVC short import ordinal/name/no-prefix/undecorate/export-as records, create stable DLL groups, code thunks and contiguous IATs directly in SG.
- [x] Link the real Windows 10.0.26100.0 x64 kernel32.Lib without modification, call GetCurrentProcessId through a thunk and GetCurrentThreadId through an IAT, and execute the SG PE with exit 42.
- [x] Verify multiple DLLs, ordinal imports and supported import name transformations, independent import inspection, repeated identical PE bytes, lazy archive extraction and cross-archive closure.
- [x] Verify 37 total malformed/unsupported/conflicting/overflow negatives preserve existing output, 50 foundation scopes and 10 complete SDK success/failure scopes recover resources exactly.

Evidence: `target/selfhost/m5-linker-sdk-checked/evidence.json`, SHA-256
`d6a2d672db09d27f14782a2d6be1580300580adafe9d5b31640201a0c83c6518`.
Start/end full revision is `0acab29421d07b497cb3592f32daf9f34fcd4c08` on
Windows 11 AMD64. All 174 source inputs and recorded tool/SDK/native-DLL hashes
remained unchanged; all 69 commands passed. The real SDK library contains
1,401 members including its two native index members; unused full COFF import
descriptor records were not extracted. The gate records the installed SDK and
native kernel32.dll paths and SHA-256 identities. Independent llvm-lib creates
the static archives; production SG links run with an empty toolchain PATH.

Earlier failed trials under `m5-linker-sdk-trial` and `trial2` exposed standard
archive internal NUL/LF padding. `trial3` passed the earlier corpus, and the
final checked report adds cross-library closure, ordinal/type-1 IAT cases and
stronger malformed index/long-name checks. Full static SDK auxiliary/weak/common/
COMDAT compatibility, DLL/TLS/resources, CodeView/PDB and real debugger gates
remain open; `m5_complete` is `false`.


## SG CodeView/PDB and real debugger checkpoint evidence

- [x] Implement SG SHA-256 and deterministic bounded MSF 7.00 block/directory/free-map emission; independently compare 1,024 SHA messages and reconstruct multi-page streams.
- [x] Emit source-bound PE RSDS plus PDB Info/TPI/IPI/DBI/module/line/SHA256 source checksum/section/public/global records directly in SG.
- [x] Independently parse the PDB streams and matching PE signature with LLVM; prove exact repeated unsigned PE/PDB bytes and equal-length source changes updating checksum/GUID.
- [x] Force PE header-table growth to 18 sections and 1,536 header bytes and execute the updated image with exit 42.
- [x] Use real LLDB to stop at source line 3, read answer = 40, step to line 4, read answer = 42 and resume to Windows exit 42.
- [x] Verify 20 native success scopes plus twelve failure classes per scope recover String/Buffer/opaque/FileSnapshot counts/bytes exactly, and twelve failed CLI calls preserve both previous outputs.

Evidence: `target/selfhost/m5-link-debug-qualified/evidence.json`, SHA-256
`c1245a5ba503b57fcfa0e19a560ccb360de32c60dc39b2ff128620f10e4f8e8f`.
Start/end full revision `c68790bb9c9dc168e353419ccd88da6e10e58042`, Windows 11
AMD64, all 177 source inputs and 37 recorded tool/runtime dependency identities
unchanged. Thirty-two commands passed. The report and retained logs include
actual debugger source stops, stepping/local observations, independent PDB/PE
inspection, the SHA corpus, source-bound identities and resource failures.
Product emission ran with an empty toolchain PATH.

LLDB 19.1.7 requires Python 3.10. Its missing standard-library dependency was
provided only for independent verification by the Python.org 3.10.11 embeddable
runtime under `target/selfhost/m5-linker-dev/lldb-python310`. The original zip
SHA-256 is `608619f8619075629c9c69f361352a0da6ed7e62f83a0e19c63e0ea32eb7629d`.
The final gate records all local Python DLL/PYD/stdlib and liblldb identities
and queries the debugger's actual loaded Python DLL path. An earlier identity
trial retained under `m5-link-debug-checked` had a buffered Python probe;
the qualified run explicitly loads a target, flushes stdout and verifies the
recorded dependency path.

The corresponding machine code is explicitly constructed by the SG fixture.
Automatic native compiler debug mapping, arbitrary COFF CodeView input/type
merging, multiple modules, aggregates/generics/async/reflection debug types and
complete native SDK/DLL/debug support remain open. This usable debugger subset
does not close the complete M5 tasks above; `m5_complete` remains `false`.


## Relocatable COFF CodeView checkpoint evidence

- [x] Emit real SG-owned `.debug$S/.debug$T` object contributions with SECTION/SECREL relocations for procedure/local/line addresses.
- [x] Read the actual relocated contribution bytes, validate the named source/type/frame/local/line subset and map it to SG PDB streams.
- [x] Independently inspect the native COFF CodeView and relocations, prove equal PE/PDB bytes when rereading the object, and use real LLDB for source/step/local/exit behavior.
- [x] Verify 19 malformed/unsupported CodeView cases preserve both previous output files and 20 native success/failure scopes recover resources exactly.

Evidence: `target/selfhost/m5-link-codeview-checked/evidence.json`, SHA-256
`bbf35f5657649f2a785b6ab4fd0c2589614a6242d348652af197d9aab78ceb78`.
Start/end revision `66043a401c96350bfdd5fed2a4b1326477a52f82`, Windows 11
AMD64, all 179 source inputs and 36 recorded tool/runtime identities unchanged;
28 commands passed. Production SG object/link/read/PDB operations use an empty
toolchain PATH. The preceding failed trial kept a Windows slash variant in its
negative-case source argument and correctly failed exact source-name matching;
the checked gate consistently uses the same canonical source pathname bytes.

This checkpoint does not claim arbitrary native SDK CodeView or automatic
compiler debug-address generation. Multiple modules, aggregate/generic/async/
reflection types, RBP/native multiple-return lifetime ranges, complete static
SDK compatibility, debug stripping and final M5 qualification remain open.


## RBP frames and multiple-return ranges checkpoint evidence

- [x] Add an explicit no-single-epilogue function form and ordered, bounded additional local lifetime ranges; bind those ranges into the deterministic identity.
- [x] Preserve multiple defranges through real COFF writing, relocation, reading and PDB generation, and reject overlap, zero extent, out-of-function lifetime and unsupported changing SP bases.
- [x] Debug a fixed RBP frame with two returns, read answer = 40 then 42 while stepping, and prove the local becomes unavailable on entering a declared epilogue gap.
- [x] Independently verify native RBP unwind metadata and Windows runtime lookup/unwind at eleven offsets across the partial prologue, body and both epilogues.
- [x] Regress the original direct PDB/SHA/MSF/resource suite and the original COFF CodeView suite against the updated SG model/writer/reader.

Main evidence: `target/selfhost/m5-link-ranges-final/evidence.json`, SHA-256
`b06ca597151e600b943235e4a7905dee435ab962d7121b3fd71782a477b8ef7f`.
Thirty-eight commands passed, including 23 malformed/unsupported CodeView
cases, 20 native resource scopes, independent RBP inspection/runtime unwind,
real source stepping/local values and actual epilogue unavailability. All 181
source inputs and 36 tool/runtime identities were unchanged. The recorded full
revision interval is `c32037ff9941ae3f10094c5b17dd8909fd63a698` through
`befcfd6d1898fefc971939bd4d7565f3da119601`; the concurrent commit did not change
any member of this gate's real import/source/tool closure.

COFF regression: `target/selfhost/m5-link-codeview-ranges-final/evidence.json`,
SHA-256 `65c40aa5cf3f7a83a339402e1cf7215c462b4a7397f92ce18f774c3556034df1`;
28 commands, 179 unchanged source inputs, 36 unchanged tool/runtime identities,
start/end revision `befcfd6d1898fefc971939bd4d7565f3da119601`.
Direct debug regression: `target/selfhost/m5-link-debug-ranges-regression/evidence.json`,
SHA-256 `f83090371fac0e51f007010657029e36fe169d95782a6282aa1392549c3b232c`;
32 commands, 177 unchanged source inputs, 37 unchanged tool/runtime identities,
start/end revision `c32037ff9941ae3f10094c5b17dd8909fd63a698`.
All runs used Windows 11 AMD64 and retained exact commands/results. Product
emission remains SG-only with an empty toolchain PATH. These are fixed-frame
primitive debug fixtures; automatic compiler mapping and the complete M5
native/type/debug corpus remain open.


## Automatic SG native source debugging checkpoint evidence

- [x] Consume checked native function/instruction/terminator/stack/epilogue maps through an isolated MIR-to-CodeView adapter, with bounded sorting and indexed source-line lookups.
- [x] Derive function/local/source records automatically from actual SG frontend/semantic/MIR/native results, including a main function at nonzero text offset and a two-return RBP frame.
- [x] Run complete source-to-object-to-PE-to-PDB operations with an empty toolchain PATH, independently inspect COFF/PDB, and prove real source stepping with answer = 40 then 42 and exit 42.
- [x] Require byte-identical repeated object/PE/PDB artifacts and five exact-resource complete source-pipeline scopes plus failure scopes per invocation.
- [x] Reject nine malformed metadata maps while preserving all three previous outputs, and reject five unsupported function/local/initialization/scope source forms without creating artifacts.

Evidence: `target/selfhost/m5-link-native-debug-checked/evidence.json`, SHA-256
`17f41545670b4e5f70f3255bce4e48ffb304b316ca5d1979af6c67858aecfab5`.
Start/end full revision `c20cb681fff8880ad4e4488c599fa266e9723f60`, Windows 11
AMD64. All 204 source inputs, 37 imported SG implementation modules and 36
recorded tool/runtime dependency identities were unchanged. Twenty-nine commands
passed. Each of two source fixtures is compiled twice and exercises five
success/failure resource scopes per invocation; tracked String/Buffer/opaque/
FileSnapshot counts and bytes recover exactly.

The preceding before function is 76 bytes; main starts at text offset 80 and
has 365 bytes, with its automatically initialized local at SP+256 over
[202,436). The real two-return main is 762 bytes, with SP+496 local ranges
[122,509) and [518,753), excluding both native return epilogues. These values
come from the SG native compiler and parsed PDB rather than fixture-supplied
code offsets. The retained independent LLVM/LLDB logs show the actual source
breakpoints, local 40/42 values, stepping and exit 42.

The initial trial passed its smaller corpus. The checked run additionally
rejects nested entry-block lexical locals and overlapping instruction/terminator
maps; source lines use a prebuilt index and binary search. The supported subset
still requires one unchanged source, no-argument signed-i64 functions and
named signed-i64 locals whose entry-block initialization and scope are proven.
Full type/module/macro/SDK/native-package debug integration and complete M5
qualification remain open; this checkpoint does not close the broad tasks.


## Primitive prototypes, parameters and locals checkpoint evidence

- [x] Intern/remap bounded LF_ARGLIST/LF_PROCEDURE records and retain real builtin result/parameter/local kinds in COFF and PDB.
- [x] Verify parameter flags, count/order/type agreement, malformed references/prototypes and unsupported calling-convention rejection.
- [x] Exercise 100 type-pool exact-resource scopes, twenty complete typed COFF/PDB scopes per invocation, repeated bytes and real LLDB double parameters/local values.
- [x] Regress the prior PDB/SHA/MSF, multiple-return/gap and Windows unwind corpus with the new type pool.
- [x] Automatically compile and debug real SG f64 parameters/results/locals and bool/i32/u64 parameters through the synchronized native metadata API.

Pure COFF/PDB evidence: `target/selfhost/m5-codeview-primitives-checked/evidence.json`,
SHA-256 `d2d4a92ca7bbe291235594aeda40b84906dd328fe78637fc125ec83f5c913871`.
Sixty commands and 33 negative cases passed, with 185 source inputs and 36
recorded tool/runtime identities unchanged. The full revision interval is
`190ef7ca9fb26656f0a2be4e9adf1c60b6b787c2` through
`35b163db0bb259fe74e4f786d74f084f9cdc51e1`; the concurrent native commit was
outside this gate's real closure. The gate includes previous RBP/body/epilogue
Windows runtime unwind checks, real source stepping/local lifetime gaps,
primitive signature remapping, typed parameter/local resources and LLDB
left = 20, right = 20, answer = 40 then 42, with exit 42.

Direct debug regression: `target/selfhost/m5-debug-primitive-regression/evidence.json`,
SHA-256 `f91561cb0a7a316f91280b455089140921ee8f4f382fac38a60909e3edb9bb88`;
32 commands, 178 unchanged sources and 37 unchanged tools/runtime identities,
start/end revision `190ef7ca9fb26656f0a2be4e9adf1c60b6b787c2`.

Automatic source evidence: `target/selfhost/m5-native-debug-primitives-trial/evidence.json`,
SHA-256 `f892e6ec6e27e7eb868dc59abe6d265ef3053081aeb3e6a590d50de58a53c7ca`.
Despite the directory's development label, this complete report is `passed`:
40 commands, 13 negative cases, 207 unchanged source inputs, 38 imported SG
modules and 36 unchanged tool/runtime identities, start/end full revision
`35b163db0bb259fe74e4f786d74f084f9cdc51e1`. All runs use Windows 11 AMD64.
Four real sources are compiled twice each with byte-identical objects/PE/PDBs;
every invocation exercises five complete success/failure resource scopes.
LLVM inspects the resulting COFF/PDB records and LLDB proves each source stop,
step, answer 40/42 and exit 42. The typed fixtures additionally read double
left/right = 20 and bool flag = true, i32 small = 40, u64 big = 2. The new run
includes the synchronized HIR4/MIR26 interfaces and the qualified native typed
load fix; the earlier failed development build with a transient missing
ShChecked builtin_calls field is retained and is not qualification evidence.

At the primitive checkpoint, the reader/writer supported the named builtin kinds; pointer,
aggregate, type-server/variadic and complete native SDK/debug/module coverage
remained open. The console main contract is still signed-i64(), and local
initialization/scope/lifetime proofs retain their stated restrictions.
`m5_complete` remains `false`.


## Pointer/reference type-pool checkpoint evidence

- [x] Preserve distinct Near64 pointer and lvalue-reference modes with checked const referent modifiers.
- [x] Remap builtin and backward pointer/modifier references through arguments, results and locals; reject unsupported or unrepresentable combinations explicitly.
- [x] Independently compare a real clang C++ const-i64-reference type prefix byte for byte, including 64-record deduplication and unsupported full-stream rejection.
- [x] Regress the prior typed COFF/PDB, multiple-return ranges, actual debugger and Windows unwind corpus with the extended type pool.
- [x] Prove 64 pointer/modifier wrappers remain usable, reject the 65th before pool mutation, and reject short pointer records before reading attributes.
- [x] Compile and debug seven real SG sources, including readonly/mutable references and a raw void pointer from an explicitly unsafe WinAPI declaration.

Pure COFF/PDB evidence: `target/selfhost/m5-codeview-pointer-types-bounded/evidence.json`,
SHA-256 `766baa8fababace263b4c01782b9d1c5469256a9f22c6c7fd76ebb2c82492112`.
Seventy-seven commands and 44 negative cases passed on Windows 11 AMD64;
186 source inputs and 36 tool/runtime identities were unchanged. Start/end
full revision: `ac077024a358e5f4b2fb1a12a643e96353f56b67`.
The gate retains the 100 type-pool resource scopes and twenty complete typed
COFF/PDB resource scopes per invocation, deterministic bytes, source stepping,
local lifetime gaps and runtime unwind checks. The independent clang fixture
is only a format oracle; product type remapping and PE/PDB generation run in SG
with an unavailable compiler/linker PATH.

A read-only review found the initial constructor's depth check reserved no
space for its new wrapper: it could return an unusable 65th-layer type. Both
constructors now validate the base starting at depth one. Independent 64-layer
pointer and mixed pointer/modifier round trips pass; the two 65-layer forms
fail without publishing output. Both short-record forms report malformed
types. The earlier 71-command report is retained as historical evidence and
is superseded by the bounded report above.

Automatic source evidence:
`target/selfhost/m5-native-debug-pointers-bounded/evidence.json`, SHA-256
`e3a582c5a0cbb078cb9c0ecaa570654d316584e1f4d9936576185c3dca80de16`.
Sixty commands and 14 negative cases passed on Windows 11 AMD64, with 210
source inputs, 38 imported SG modules and 36 tool/runtime identities unchanged.
Start/end full revision: `ac077024a358e5f4b2fb1a12a643e96353f56b67`.
All seven real sources compile twice with identical object/PE/PDB bytes;
each invocation runs five complete success/failure resource scopes. LLDB
reads a const-i64 reference with pointee 40, a mutable reference changing
40 to 42, and a void pointer equal to the real GetCurrentProcess handle
`0xffffffffffffffff`; source stepping, local 40/42 values and exit 42 pass.
The prior scalar and multiple-return source fixtures remain in the same gate.
Production source/object/link/PDB commands have no compiler/linker tools in
PATH. The raw-pointer fixture's explicit KERNEL32 binding is part of the test
driver; arbitrary FFI admission/runtime handle validation is not qualified.

Earlier development evidence is retained: the first raw-pointer declaration
correctly failed the frontend's unsafe-FFI rule (5122), and was corrected to
an unsafe function inside the extern-C block. That failed trial is not
qualification evidence. The complete 60-command pre-depth-fix native report
also remains available, and is superseded by the bounded report above.

This checkpoint supports checked scalar/void pointer targets and scalar
references. Aggregate/function targets, richer pointer modes, type servers,
and the complete native SDK/debug/module corpus remain open; complete M5 is
not qualified.
