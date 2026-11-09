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


## Bounded aggregate type and source-debug checkpoint

- [x] Emit and remap checked struct/union/array/enum field records with target sizes, offsets, backward references and bounded shared-type depth.
- [x] Compare independent clang member/array bytes and supported nominal records after explicitly removing unsupported unique-name metadata.
- [x] Prove 100 aggregate type-pool resource scopes and reject malformed members/counts/extents/references/properties.
- [x] Compile real SG struct/tuple/array/tagged-enum locals through native COFF, SG PE/PDB generation and actual debugger field observation.
- [x] Reject malformed/duplicate/missing field-name maps and native local extents outside the real frame allocation.
- [x] Bind the compiler producer to immutable SG/stdlib snapshots, Rust runtime inputs, tool identities and actual checkout-change reporting.

The earlier aggregate development reports are retained without promoting
failed runs to qualification. One old unknown-type mutation became a supported
structure leaf and correctly changed from unsupported to malformed; the
negative now uses the still-unsupported LF_FUNC_ID leaf. The first snapshot
native build caught an if-without-else branch returning bool and was corrected
to unit. Whole-local frame checking also exposed the established distinction
between an RSP-only fixture's allocation and a native RBP frame's additional
saved-register slot. Generic CodeView validation checks the declared frame;
the native adapter additionally checks the exact allocation before that slot.
The bounded final evidence follows; the broad M5 completion tasks remain open.

Pure CodeView/PDB evidence:
`target/selfhost/m5-codeview-aggregate-frame-checked/evidence.json`, SHA-256
`1c41ebf853647cd627567e1f78830785df1d75ee3a5567c8f7a8469143af35ed`.
Ninety-three commands and 52 negative cases passed on Windows 11 AMD64;
191 guarded source/snapshot inputs and 36 tool/runtime identities were unchanged.
Independent clang supplies actual member-list and array bytes, plus structure
and enum records with their unsupported unique-name property removed explicitly
and their field-list index remapped. SG round trips those supported records
exactly. The complete native C++ stream is still rejected. The gate includes
100 primitive-pool and 100 aggregate-pool exact resource scopes, twenty typed
COFF/PDB scopes per invocation, the prior actual debugger and Windows unwind
corpus, depth limits, bad field references/attributes/counts/offsets/properties,
and output preservation.

Automatic source/native evidence:
`target/selfhost/m5-native-debug-aggregate-final/evidence.json`, SHA-256
`0f9e8af3a3bbe1fd93063a4a367a3aac9257544ef1ecd2689704951bed0c3ced`.
Seventy-nine commands and 21 negative cases passed on Windows 11 AMD64;
217 source/snapshot inputs, 40 imported SG modules and 41 tool identities were
unchanged, including the actual Cargo/rustc/rustup shims and resolved binaries
with their versions. Existing user Cargo configurations are hashed and guarded.
The producer contains 155 exact SG/stdlib/runtime.c snapshots; all original
checkout hashes also remained unchanged during this final run. Each of nine
real SG sources compiles twice with identical object/PE/PDB bytes and five
complete resource-check scopes per invocation. LLVM inspects the resulting
COFF/PDB and LLDB observes struct bool/i32/u64 fields true/40/2, tuple members
40/2, array elements 40/2, and enum Value tag/payload 40. Source stepping, answer
40/42 and Windows exit 42 pass. Malformed/duplicate/missing field names and a
whole aggregate outside the native allocation fail before output publication.

Both reports record the full revision interval
`d5144a0fbce78d9d5fc01b9b72195604131fb852` through
`682446a8f9d407f5cb86d81af4ea816ed90210b4`; all guarded sources/tools and all
reported producer checkout changes are empty. Production compile/object/link/
PDB commands run with an unavailable compiler/linker PATH. Development seed
construction still uses the explicitly recorded Rust/C runtime; this is not
M6 Rust-free completion. Generic/class/recursive/function type support,
conditional active-variant display, full C++/SDK type streams, multiple debug
modules, complete M5 and full PDB qualification remain open.


## Multiple physical files in one SG object/PDB module

- [x] Preserve independent physical paths, raw-source checksums and checked C13 file-block references in COFF, PDB /names and DBI FileInfo.
- [x] Consume validated physical MIR function-source ownership and real native address/stack/lifetime metadata without flattening the debugger's source identities.
- [x] Require an independent typed-MIR function-source registry for supported multi-file COFF reading and reject missing/duplicate/wrong/bounded entries.
- [x] Verify Unicode/space/ampersand paths, exact source stops, main-to-worker stepping, locals 40/42 and Windows exit 42.
- [x] Exercise first-call success and failure resource scopes, deterministic repeated artifacts and malformed file/checksum/line-block cases.
- [x] Regress the existing single-source primitive/aggregate/pointer/type/unwind/debugger corpus and compare prior qualified one-file artifacts where their identities permit exact comparison.

The target-only proof cited in the design is complete. Production admission is
recorded by these fresh reports, all with no changed consumed sources/tools:

- `target/selfhost/m5-multi-physical-debug-qualified/evidence.json`, SHA-256
  `d92cc89a2d9ffb65dcc24c63c2a010668e40612db2ac64225207dc6caa3aa80d`:
  44 commands, 215 input hashes, 42 tools, 18 rejection cases. Both physical
  files live in a Unicode/space/ampersand directory. Native LLDB frame-format
  output records their exact paths and source lines, including an independent
  worker-file breakpoint. Five first-call scopes compare all five resources
  after every success/failure; both assertion negative controls actually exit
  1. The source/COFF/link/PDB product chain uses an empty PATH. Repeated object,
  executable and PDB bytes match; Windows exits 42.
- `target/selfhost/m5-codeview-multi-file-regression/evidence.json`, SHA-256
  `2d9e3e7572cc14f85066487e5067f3ac02a54288324a67d1ac7a70874668c1ee`:
  all 93 original pure CodeView/range/primitive/pointer/aggregate commands pass.
- `target/selfhost/m5-native-debug-multi-file-regression/evidence.json`, SHA-256
  `2b25449d65d40d17182d23aa26a33d5c358a739e8399382f19fdc53984b61d65`:
  all 79 original real-source native/debugger commands pass, including nine
  source fixtures and 21 rejection cases; producer checkout changes are empty.
- `target/selfhost/m5-multi-debug-byte-compatibility-final/evidence.json`, SHA-256
  `d9c75721b600a43a164debb80f6e51077ffda38fc4428e164b28688bede3e137`:
  an independent Python oracle checks COFF F3/F4 and PDB /names/DBI/C13 paths,
  SHA-256 and file references with four malformed-byte negative controls.
  Against the earlier qualified aggregate-native report, all nine one-file
  COFF objects are byte-identical. PE/PDB comparison removes only the exact
  output PDB path, its debug-section extent copies, and the path-derived
  GUID/signature; all other PE bytes and all 11 PDB streams match. A changed
  code byte is observed for every fixture. Raw PE/PDB bytes at different output
  paths are not claimed identical.

The main and pure reports start/end at full revision
`3eeefa14f2ae6112053c404b753a4d2b468b3f92`; the native report ends and the
independent oracle starts/ends at
`081abb656e0c939bcfd3653bbcc15c000fb8ff8d` on the recorded Windows host.
The development seed still uses the recorded C/Rust runtime; the new gate
pins Cargo/rustc and observes Cargo actually invoking that rustc for both a
build script and library. This does not complete M6. The earlier trial remains
a failed report because an unflushed LLDB Python print did not satisfy the
full-path oracle; it is not qualification evidence. Independent COFF/PDB
modules, mixed-file functions, inline-origin policies and complete M5 remain
open.


## Independent supported SG COFF units

- [x] Keep ordered all-debug objects in separate source/registry/type namespaces before their unit readers pass.
- [x] Merge global TPI references and preserve independent module streams, DBI contributions and per-module shared-file associations.
- [x] Preserve same-named static procedures with module-specific local references and only real externals in publics.
- [x] Execute true cross-object SG calls, same-index distinct aggregate types and exact physical source/debugger observations.
- [x] Reject malformed input types/sources, missing/duplicate units and program/stream/link/image mutations with first-call resource checks and poisoned-program behavior.
- [x] Prove nine legacy N=1 raw PE/PDB byte comparisons, equal-input/options two-fresh-artifact-directory bytes and complete existing pure/native debug regressions.
- [x] Record exact snapshot/tool/host/revision evidence and strict spec validation without closing full M5 or later reproducibility/runtime milestones.

The target current-boundary proof is preserved in
`target/selfhost/m5-independent-coff-review/evidence.json` (SHA-256
`f134cb54a600dc283573f57e82c8caf9fbaae6c1505b4b33fbfb51f7b0595dc3`),
frozen by `freeze.json` SHA-256
`8fa2b41ab8930fefcec5a8fb8154cd97a232ed3931c11c37981b7ab47d05a09f`.
It proves two real SG objects/cross-call 42 but only a legacy partial one-module
PDB: adding a second unit to the old model rejects 7303, and the worker's
physical-file breakpoint stays pending. It is not production multi-module
qualification. The production tasks above require the new named full gates.


Production admission for the independent SG unit subset passes these reports:

- `target/selfhost/m5-independent-units-checkout-bound/evidence.json`, SHA-256
  `3c271fba6c7d22fb273ee19753b343f86fba50d8fe3edc8403a2016fc55af2a0`:
  109 commands, 408 consumed input hashes and 42 tools. The original SG/stdlib
  checkout manifest is recorded explicitly; any change to it fails the gate,
  as do changes to consumed snapshots, tools or present/absent Cargo configs.
  Two independently lowered SG objects call a real external worker, preserve
  separate physical registries and remap incompatible local type indices.
  A second variant has distinct LF_FIELDLIST records at local index 4098 in
  both objects; its differently named Local members survive as distinct global
  types. Two same-named static helpers retain module-specific S_LPROC32 and
  S_LPROCREF records and have two actual debugger locations. Both modules refer
  to the same physical common.sg: /names stores it once and DBI preserves four
  file contributions over three unique paths. LLVM inspection and an
  independent byte oracle check module/stream/file/type/global-reference
  ownership with four negative controls. LLDB reads main state 38/2 and input
  40, crosses into worker state 40/2, reads answer 42, unwinds back to main and
  exits 42; a shared-file breakpoint independently hits both helper frames.
  Each variant emits two fresh artifact directories with byte-identical
  objects, PE and PDB using the recorded equal logical module names and RSDS
  path. This uses the same physical source inputs and does not qualify changed
  source roots or path remapping. Nine legacy all-global one-unit fixtures
  compare the old and new raw PE/PDB bytes at exactly equal image/path inputs.
  Twenty-two one-unit state mutations, twenty-five multiple-unit source/
  registry/module/stream/link/image mutations and five malformed COFF records
  reject with exact diagnostics. Each success/failure invocation compares all
  five resource counters after every one of five scopes beginning before the
  first call; reader failure poisoning is tested by a repeated read without
  further program mutation. Both assertion negative controls actually exit 1.
- `target/selfhost/m5-codeview-independent-unit-artifacts/evidence.json`,
  SHA-256 `74a1124ca3639110214bbbedacc27200e53a8456a7f301777ae0ccb75f2606dd`:
  all 93 existing pure CodeView/unwind/range/primitive/pointer/aggregate commands.
- `target/selfhost/m5-native-independent-unit-regression/evidence.json`,
  SHA-256 `a0d84e17f6b2286970a4354edf4c048b19990c958cd0c301a7710f7c18fa686d`:
  all 79 existing real-source native/COFF/PDB/LLDB commands.
- `target/selfhost/m5-physical-files-independent-unit-regression/evidence.json`,
  SHA-256 `fed54f822c826f7114d69b489cdb27238d3c4d4b2a3719e2a374a852f2c2cfee`:
  all 44 existing one-object/multiple-physical-file commands and 18 refusals.

The 109-, 79- and 44-command reports start and end at full revision
`efed463dc06be086d97718089e5d4cc1322333e0`. The replacement 93-command pure
report starts at that revision and ends at
`836777a0fd1e7083d59d80af8053d33691959e63` on the recorded Windows host.
That concurrent DEFLATE commit changed none of the pure report's consumed
inputs; its source, tool and artifact guards remain empty. All four reports
have empty source/tool/config and reported checkout-change arrays. The new
runner pins actual Cargo/rustc, observes the selected rustc execute build-script
and library compilation, snapshots the complete 46-entry SG import closure
and whole stdlib/C runtime, and guards actual Rust runtime/Cargo inputs. Product
source/object/link/PDB commands run with an empty PATH. Development compilation
still uses C/Rust and does not complete M6. Nondebug SDK objects, archive debug
unit admission, automatic native internalization, mixed-file functions/inline
origins, complete PDB/SDK support, M5 and different-checkout M8 reproduction
remain open. Failed development trials are retained and are not qualifications.


Terminal artifact retention is checked separately from command success. The
first pure regression report, `m5-codeview-independent-unit-regression`, is
retained with an identified retention limitation: its listed SG pointer prefix
was later overwritten by an aggregate-test scratch output. That historical
report is not used for terminal-artifact qualification here. The replacement
93-command `m5-codeview-independent-unit-artifacts` gate saves the actual SG
pointer output once as `sg-pointer-prefix-qualified.cv` and verifies every
listed artifact hash at completion. Its `changed_artifacts`, source and tool
arrays are empty; the retained prefix SHA-256 is
`fc7303ab8b2cbb325270d46c3b26f374a03cdc6112b6f65397d689b71631e7af`.
The 109/79/44-command reports do not consume this pure runner in their recorded
or actual Python import closures; their input hashes remain matched after this
runner-only repair.


## Explicit nondebug objects beside checked SG debug units

- [x] Bind schema-2 unit classifications and complete actual COFF metadata including relocations without reducing the expected object count.
- [x] Require explicit nondebug submission and reject known, unknown and partial `.debug` contributions.
- [x] Preserve real nondebug module/section/symbol identity with zero invented source/type/line records.
- [x] Requalify the historical immutable C fixture branch from source, headers, raw/normalized object, tool and normalization evidence.
- [x] Execute two real SG debug units plus the C leaf/data object, inspect the three modules and verify precise SG source debugging and scoped static helpers.
- [x] Check exact public hash/address structures and distinguish actual C-address debugger stops from unsupported name lookup with an independent clang/lld control.
- [x] Verify first-call success/failure resources, missing/mislabeled/poisoned units and actual relocation/source/stream mutations, including an explicit all-nondebug refusal.
- [x] Regress the 109-command independent-unit/N=1 corpus and retain only unchanged, artifact-matched 93/79/44 reports for unaffected closures.
- [x] Freeze exact reports/source/tool/artifact identities and strict specs while full SDK/debugger/runtime milestones remain open.


This extension is qualified by
`target/selfhost/m5-mixeddebug-sdk-qualified/evidence.json`, SHA-256
`f0bcc7423d652813588ffe67f80eff9318a691b325e2d17e93252ce2cb1022e8`:
66 commands, 258 consumed inputs, 46 recorded tools and 25 SG rejection cases.
It emits two actual SG debug objects and links the independently requalified
C leaf/data object as a third real object. Its three PDB modules have file
counts 2/2/0 and C13 byte counts 208/200/0; DBI retains four SG file
contributions over three unique physical paths. The C module contributes real
.text/.data/.rdata sections, four external public addresses and four static
data names only in bound metadata. An independent byte oracle checks complete
public hash buckets/bitmap/record pointers and the address map, with four
malformed-byte controls. Real SG-to-C calls, SG source/stack/local debugging
and shared-file static helper stops execute separately to exit 42.

The exact C public file address is `0x140001a20`. A separate LLDB process
actually stops there at RIP `0x7ff6903c1a20`, equal to its recorded module slide
`0x7ff5503c0000` plus that file address, reads RCX 0 and has no invented source
frame; the task explicitly kills that stopped process. The SG PDB and an
independent clang/lld PDB both retain a pending C-name breakpoint under pinned
LLDB 19.1.7. Those negative observations do not qualify name lookup or successful
continuation through the retained untyped int3 limitation. The primary source
path, SHA-256, canonical tag URL and actual physical line numbers are recorded
in the design; a newer debugger and full SDK typed/name/source experience
remain open.

The historical SDK branch consumes only the pinned C source/header/tool/raw
object/normalization branch of report
`runtime-deflate-final-checkpoint`, SHA-256
`7896f7263042359b916d3efe2905da312e8563bf60d554891b229445f708c718`.
Fresh normalization reproduces exact historical object SHA-256
`d5b3c8cea56a930f7f8e2d707b84773331e05c5dfc835b3fec242a76a130ee70`;
raw code/data, relocation targets, defined/referenced symbol addresses and
static data preservation are independently checked. This does not consume
unrelated live SG DEFLATE algorithm sources or qualify arbitrary raw clang
auxiliary metadata. Raw auxiliary COFF rejects 7203. Single `.debug$S`, single
`.debug$T` and unknown `.debug` sections reject 7333. The 21 state cases also
exercise missing/duplicate/mislabeled units, schema 1, minimal streams, actual
sections/symbols/image/relocations, bound relocation copies and all-nondebug
rejection. Each of all 25 SG failures checks all five resource counters after
each of five scopes starting before its first call and preserves sentinel
outputs; both assertion controls really exit 1.

Two fresh artifact directories use the same three logical names
`sg-main.obj`, `sg-worker.obj`, `sdk-leaf.obj`, identical actual physical source
inputs and the explicitly recorded fixed RSDS path. Their objects, PE and PDB
bytes match. This is not different-source-checkout or path-remapping evidence.
The schema-2 and relocation-binding changes additionally pass the unmodified
109-command gate at
`target/selfhost/m5-independent-units-mixed-regression/evidence.json`, SHA-256
`75a27779a3684c4ae826475d096a2f6498fc1fd41dfc4b64ea092663d607fce1`:
408 inputs, 42 tools, nine exact legacy N=1 PE/PDB comparisons and all 52
existing rejection/resource cases. Both new reports start and end at full
revision `32c70b734e05f9101b914334e4fd00dd147276e5` on their recorded Windows
host, with empty source/tool/config/original-checkout change arrays.

The unchanged 93/79/44-command reports named above remain qualified for their
unaffected closures: every consumed source/tool and every listed terminal
artifact has been rehashed and still matches. They do not import either
changed module-container source. Their historical revision intervals remain
as recorded above; they are not represented as freshly rerun. The final freeze
separately records recursive output manifests, exact current source/tool and
present/absent-config rechecks and the primary debugger source. Development
compilation still uses its recorded C/Rust runtime. Full M5, archive/debugger
coverage, installed SDK packaging, M6 and different-checkout M8 remain open.
