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
