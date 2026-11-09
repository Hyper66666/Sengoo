## Encoding and object foundation

- [x] Implement bounded binary writer and deterministic x64 byte/COFF foundation (`target/selfhost/native-foundations/evidence.json`; full MIR/backend remains open).
- [x] Implement x64 register/memory/immediate encodings and branch/call fixups with independent byte/CPU tests (`target/selfhost/native-foundations/evidence.json`).
- [x] Emit and inspect a deterministic single-text COFF object with short/long symbols; full sections/data/relocations remain open (`target/selfhost/native-foundations/evidence.json`).
- [x] Resolve bounded concrete Windows x64 type sizes, alignments, array strides and nominal/tuple/enum field offsets; verify recursive/overflow/claim failures and exact resources, and compare ten layout results with independently compiled C (`target/selfhost/m4-layout-final/evidence.json`). Full calling conventions remain open.
- [x] Emit deterministic multiple initialized COFF sections, short/long section and symbol names, external/static symbols and checked AMD64 relocations; independently inspect and link/execute external calls, cross-section loads and absolute data pointers (`target/selfhost/m4-object-checked/evidence.json`). Auxiliary/COMDAT/BSS/debug/unwind records remain open.
- [x] Implement and independently execute the initial validated scalar MIR consumer, including parallel CFG edge copies, integer widths, local memory, direct/indirect calls, six-argument Windows C calls, checked arithmetic failures and wrapping mode (`target/selfhost/m4-scalar-initialization/evidence.json`). Complete MIR/native ABI and cleanup support remain open.
- [x] Verify the initial SG source-to-native-object CLI through frontend/semantics/owned HIR/MIR/x64/COFF against the frozen qualified Rust/LLVM reference for nine exact source programs and three output-preserving rejections (`target/selfhost/m4-source-supported/evidence.json`). Public sgc/module/tool parity remains open.
- [x] Emit fixed-frame Windows x64 `.pdata/.xdata` unwind records and checked `__chkstk` probes for >4 KiB frames; verify LLVM inspection, `RtlLookupFunctionEntry`, `RtlVirtualUnwind`, deterministic objects and exact resource recovery (`target/selfhost/m4-unwind-final3/evidence.json`). Exception handlers, XMM saves and CodeView remain open.
- [x] Verify bounded scalar SSE f32/f64 instruction encoding with independently specified bytes, CPU arithmetic/load/store/bit-pattern execution, damaged-byte rejection and 100 exact resource scopes (`target/selfhost/m4-sse-final/evidence.json`). Floating MIR and complete ABI/NaN/conversion behavior remain open.
- [x] Execute scalar floating MIR arithmetic, constant bits, memory/call/return storage and ordered/unordered comparisons against independent C tests, including NaN/infinities/signed zero and mixed six-position Windows integer/float calls (`target/selfhost/m4-float/evidence.json`). Decimal literals and numeric conversions remain open.
- [x] Classify and execute initial aggregate parameters/results with size-aware values, typed field construction/extraction, aggregate CFG edge snapshots, indirect argument copies and hidden return pointers; independently verify 3/8/16-byte structures and enum payload storage using C (`target/selfhost/m4-aggregate-abi/evidence.json`). Complete cleanup/indexing/overlap/ABI coverage remains open.
- [x] Convert bounded decimal floating literals entirely in SG with exact rational IEEE rounding and an edition-2026-compatible f64-to-f32 path; independently verify 3,332 cases, 15 actual reference double-rounding executions and 100 resource rounds (`target/selfhost/m4-float-literal-exact/evidence.json`). Numeric casts and complete backend integration remain open.
- [x] Connect reference-compatible decimal literals and compare 19 exact supported programs through the SG native CLI and frozen Rust/LLVM path, including generic instances, structures, float literals and class methods (`target/selfhost/m4-source-expanded/evidence.json`). Full language/tool/backend support remains open.
- [x] Preserve saturating float-to-signed/unsigned integer conversion, integer-to-float rounding and f32/f64 width conversion with independently executed boundary/NaN/infinity cases and 4,096 integer bit patterns (`target/selfhost/m4-numeric-casts/evidence.json`). Full M4 semantics remain open.
- [x] Execute signed/unsigned division and remainder, debug zero/overflow hooks and release wrapping behavior for signed minimum divided by minus one (`target/selfhost/m4-division/evidence.json`). Full numeric/cleanup coverage remains open.
- [x] Execute initial owned-local initialization, conditional user Drop, cleared flags, nested/array reverse cleanup and active-only enum payload cleanup; reject pending resource glue and forged nested Drop calls without emitting an object (`target/selfhost/m4-cleanup-safe/evidence.json`). Runtime resource glue, conditional partial-field cleanup and full ownership support remain open.
- [x] Run the qualified SG native emitter with the separately qualified SG linker and real SDK import archive under an empty PATH; execute 32 programs in two clean directories and compare object/PE bytes exactly (`target/selfhost/m5-native-source-link-qualified/evidence.json`). Compiler self-rebuild, full runtime and M4/M5 closure remain open.
- [x] Emit typed array/raw-pointer index strides and unsafe opcode-22 byte memmove; independently execute 76,875 overlap cases across u8/u64/aggregate pointees, unaligned/separate ranges, zero-count null pointers, protected-page boundaries and 1/3/8/16-byte strides (`target/selfhost/m4-memory-ranges/evidence.json`). Slices, full runtime extents and supported-corpus closure remain open.
- [x] Emit decoded string literals into readonly `.rdata` with private symbols and checked RIP-relative relocations; independently verify UTF-8, escapes, empty/interior-NUL strings, repeated storage identity and Windows page permissions (`target/selfhost/m4-string-data-checked/evidence.json`). Owned String/runtime SDK behavior remains open.
- [x] Replay HIR schema 3 through the native source CLI for 46 programs, including arrays, string literal ABI, impl discarded contexts and aggregate operator calls; retain 43 actual frozen-reference equal executions and three explicitly unequal reference gaps (`target/selfhost/m4-source-hir3-qualified/evidence.json`). Full parity remains open.
- [x] Separate validated native object construction from serialization and optionally collect instruction/terminator/prologue/epilogue addresses for an external debug adapter; verify all ranges, fresh-state rejection and byte-identical objects (`target/selfhost/m4-native-object-debug-api/evidence.json`). Automatic source/local/PDB integration is a separate M5 gate.

## Supported target behavior

- [ ] Implement target type layout and Windows x64 direct/indirect/C ABI calls, aggregates and SSE values.
- [ ] Lower every supported MIR instruction, control-flow terminator and block parameter correctly.
- [ ] Preserve overflow/division/shift/floating/contract-check semantics and ownership cleanup.
- [ ] Implement stack layout/probing/nonvolatile preservation and matching unwind records.
- [ ] Emit CodeView types/source/locals for complete Windows debug/PDB linking.
- [ ] Add register allocation and optimizations needed by the frozen performance gates.

## Integration

- [ ] Run native ABI/C interop and cross-compiler semantic corpus with exact inputs and output checks.
- [x] Reject malformed binary operands, patch ranges and oversized objects before emission (`target/selfhost/native-foundations/evidence.json`).
- [ ] Verify source-level debugging through the SG linker when available.
- [ ] Record full revision/host/tool/input/commands/results and validate change/all specs strictly.
- [ ] Close M4 only after complete supported backend behavior passes; keep M5-M8 integration open.

Concrete layout evidence: report SHA-256
`dedfdb9d9ac5334b9c4d7205e8a6679d90907e35d7d87c3b8c072ecafb39c28f`,
revision `4aa6e5d219ba4b491af05da8cea258f4ff32d96d`, Windows 11 x64.
Seven commands passed with unchanged source/tool identities. The SG fixture
ran 100 success/failure scopes with exact String/Buffer/opaque resource recovery;
its ten emitted layout results matched the C probe. `m4_complete` remains false.

Multi-section object evidence: report SHA-256
`dc37b0e3b7950bbf8eb06490a4eca0c1b52cf787ef4eb2f7de99d27e858343a8`,
revision `d5ee01a` (the report records its full revision), Windows 11 x64.
All nine commands passed with empty source/tool change lists. One hundred
native success/failure scopes restored resources exactly; repeated COFF bytes
matched, LLVM independently inspected all three sections and their symbols and
relocations, and the linked C probe verified the external call plus data/pointer
relocations. These are object-writer gates, not completed M4/M5 milestones.

Initial scalar MIR consumer evidence: report SHA-256
`deccc25e5284604e395a59509779792bb3aa6e8a2e7ddc50e19d83001cf4638f`,
revision `4a190f17c6279b52daf32001202938dfa9cf4859`, Windows 11 x64.
Nineteen commands passed with unchanged source/tool identities. Ten native
builder/validator scopes recovered resources exactly, repeated objects matched,
independent inspection/disassembly and C linking succeeded, and C execution
verified loop/parallel-swap/call/memory behavior. Five signed/unsigned/narrow
overflow executions exited 63 through the verification hook; wrapping arithmetic
returned the expected bits without invoking it. The earlier unsigned multiply
encoding failure remains in `target/selfhost/m4-scalar-final/`; it was fixed
before this successful run. This evidence does not complete M4.

Initial source pipeline evidence: report SHA-256
`ed54ccce7056c7fb2eb65e003cb05d37b5a7bf2929a912124b5b10ac952e5257`,
revision `8182d7931ab882963c4fe583201f16d07ac50531`, Windows 11 x64.
All 68 commands passed with unchanged source/tool identities. Nine exact UTF-8
source files passed both paths, including for/while, scalar match, narrow suffix
arithmetic, functions and a six-argument C call. SG objects were deterministic;
both linked executables exited 42 with equal stdout/stderr. Three deliberate
failures preserved previous output bytes. The initial trial in
`target/selfhost/m4-source-final/` used an unsuffixed narrow-literal form rejected
by the frozen reference and remains a failed report; the successful gate uses
its explicitly supported `40i8`/`2i8` counterpart. These gates do not close M4.

Fixed-frame unwind evidence: report SHA-256
`9144e97e16d31a91f435363ca5b91785aad21ddb2366d03e78deabc8c00b1e95`,
revision `709fb7c` plus the recorded native backend source edits, Windows 11
x64. Twenty commands passed with unchanged source/tool identities, including
deterministic repeated objects, LLVM `ALLOC_SMALL` inspection, Windows runtime
function lookup/unwind at multiple offsets, and five overflow exits plus
wrapping. Boundary fixtures cover invalid unwind frame sizes, incomplete
buffers and the >4 KiB `__chkstk` encoder. The prior report remains retained
as development evidence; exception handlers, XMM saves, CodeView and actual
large-frame execution remain open.

Large-frame execution now supersedes that residual:
`target/selfhost/m4-large-stack-bounded/evidence.json`, SHA-256
`13b93f912b43531ab07a95bb4168605dbfbada3e641a7d3e15869e3c921ae0ff`,
revision `9a62b4e2c24d9610451b18915dea5629ce356b9e`, Windows 11 x64.
Twenty commands passed with unchanged source/tool identities. An independently
linked C caller executed a 600-local frame through `__chkstk`, validated all
six ABI arguments, the deepest local load/store and result 42, and located the
generated large function during a Windows stack walk. The test fixes the call
relocation and longer unwind prologue offsets; earlier emission-only evidence
did not prove these. Exception handlers, XMM saves and CodeView remain open.

Scalar SSE encoder evidence: `target/selfhost/m4-sse-final/evidence.json`,
SHA-256 `910dda9aa543f0cde288b0dd35a3c70379408a94e0c6480a460fcc7576bb5895`,
revision `9a62b4e2c24d9610451b18915dea5629ce356b9e`, Windows 11 x64.
Nine commands passed with unchanged sources/tools. One hundred SG
success/failure scopes recovered tracked resources exactly, repeated byte
streams matched, the independent C probe compared known instruction bytes and
executed f32/f64 arithmetic/memory/bit-pattern fixtures, and a deliberately
corrupted instruction stream was rejected with exit 66 before execution.

Initial floating MIR evidence: `target/selfhost/m4-float/evidence.json`,
SHA-256 `0d5d48803b91e2b4170ee713518d691fe8ede80e57ee58a406f7c7d7adb1c547`,
revision `abb12a966a25fe34c5f23f401ad04c01aba84824`, Windows 11 x64.
Twenty-one commands passed with unchanged sources/tools; all preceding integer,
large-frame, unwind and overflow/wrapping regressions remained green. C callers
verified scalar f32/f64 arithmetic, mixed ABI positions and NaN/infinity/zero
comparison behavior. This does not qualify decimal parsing, numeric conversions,
aggregates or complete M4 behavior.

Initial aggregate ABI evidence: `target/selfhost/m4-aggregate-abi/evidence.json`,
SHA-256 `2c05ab3a9ee3ead9654c04bacdf36b4fe6b8e63e4d90149eb4637f00ab9c50f7`,
revision `cb3277bcfdaf55ce10c04adcb053517f011fba70`, Windows 11 x64.
Twenty-six commands passed with unchanged sources/tools. The ABI classifier
passed 100 exact resource scopes; ten MIR/native builder/resource scopes,
deterministic objects, independent inspection/disassembly and all previous
integer/float/unwind/probe regressions passed. C execution verified odd/small/
large aggregate storage and call/return conventions, enum tag/payload values
and both aggregate merge branches. This partial gate does not close M4.

Decimal conversion evidence: `target/selfhost/m4-float-literal-exact/evidence.json`,
SHA-256 `5989a6bc4f92e5d88e20c8ec060d621bee4e3930932339577f091510853679ee`,
revision `cb3277bcfdaf55ce10c04adcb053517f011fba70`, Windows 11 AMD64.
Fourteen commands passed with unchanged sources/tools; 3,332 strict and
reference-compatible inputs plus repeated bytes and 100 exact resource rounds
were checked. Fifteen f32 double-rounding boundaries matched actual frozen
Rust IR compiled and observed through C. One UCRT strtod oracle discrepancy
was retained and resolved using independent clang constants, Python builtin,
exact Fraction and SG agreement. The initial failed comparison remains at
`target/selfhost/m4-float-literal-final/`; no language parity exception is
introduced by this converter.

Expanded source integration evidence:
`target/selfhost/m4-source-expanded/evidence.json`, SHA-256
`c33560f9ef18543c7c0734d34355409568b863d0b024c2a71e9a969fe8ab3272`.
The report records revision `def09df340fffae2b0e32c8841b7d199f032e4da`,
Windows 11 x64 and actual input/tool hashes. All 138 commands passed with no
input/tool changes: nineteen positive programs produced deterministic objects
and matched reference exit 42/stdout/stderr, while three deliberate failures
preserved prior outputs. The source CLI still rejects unimplemented numeric
casts explicitly. The runner's later wording-only boundary update does not
alter the recorded executed cases or results.

Numeric conversion evidence: `target/selfhost/m4-numeric-casts/evidence.json`,
SHA-256 `9204fede55b4d7f10eac55d66708fa7df8c1c9f4e67145052326bbc5c373d462`,
revision `77fa0ce6fda291a678921feb07698c458fca5bdb`, Windows 11 x64.
Twenty-six commands passed with unchanged inputs/tools. C execution verifies
NaN/infinity/zero/range saturation to i8/u8/i64/u64, f32 and f64 sources,
in-range truncation and f32/f64 width casts, plus 4,096 signed/unsigned integer
rounding cases. All preceding scalar/aggregate/call/unwind/probe/overflow and
resource regressions remain green; a complete backend gate remains open.

Integer division evidence: `target/selfhost/m4-division/evidence.json`, SHA-256
`e76b1a4b8c3db854a9d812e92cc2f92ee908c0b300049ce09f9f3cc1c2be9b0b`,
revision `c68939c6f3a7bb12c126581f31e5f02456825bfe`, Windows 11 x64.
Twenty-eight commands passed with unchanged inputs/tools. C verified defined
signed/unsigned quotient and remainder results. Debug zero and minimum/-1
overflow cases invoked their hooks (verification exits 62 and 63); wrapping
mode returned minimum/zero without an overflow hook. The reference overflowing
LLVM divide is not a defined result oracle; this edge is recorded explicitly
against the existing numeric overflow specification. All prior native
regressions and exact resource checks passed.

Initial cleanup evidence: `target/selfhost/m4-cleanup-safe/evidence.json`,
SHA-256 `5c605ebb4d2989b06d08fd9d702b2059e1ea9f455ff22899b3105c657fad824d`,
revision `94c7a24fc99053c220fb6c4d36849de6451c9eea`, Windows 11 AMD64.
All 28 commands passed with unchanged inputs/tools. The ABI fixture ran 100
exact resource scopes and native builder/validator tests ran ten. Independent
C execution verified false/true initialization paths, exactly-once user Drop,
cleared flags, reverse tuple/array cleanup and only the active enum payload.
Pending runtime glue returns 6502 before object publication. A forged recursive
Drop target is rejected by MIR validation with 5007; field cleanup delegation
without sufficient initialization facts remains pending rather than running
twice. Earlier numeric/ABI/unwind/probe/resource regressions remain green.
Complete ownership and M4 remain open.

Expanded ownership/source integration:
`target/selfhost/m4-source-ownership-qualified/evidence.json`, SHA-256
`027a64742436a707a724625b4a49a3ea9e84e84237593c5b4846e9d14a5d21b7`,
revision `94c7a24fc99053c220fb6c4d36849de6451c9eea`, Windows 11 AMD64.
All 243 commands passed with unchanged inputs/tools. Thirty-four SG programs
emitted repeat-identical objects and executed 42. Thirty-three also ran through
the frozen Rust/LLVM path with identical exit/stdout/stderr, including numeric
casts/division, impl effect tails, early for exits, Option try, Result error
propagation, reverse user Drop and early-return Drop. The remaining Result try
case requires the documented frozen-reference invalid-LLVM link failure and
records `result_equal=false`; `selfhost/REFERENCE_GAPS.md` retains this gap.
Four syntax/semantic/native failures preserve prior output, including strict
class unit tails. This is not full reference parity or a completed M4 gate.

SG-source-to-SG-link integration:
`target/selfhost/m5-native-source-link-qualified/evidence.json`, SHA-256
`928c4004fd61f9397cadad532a0c8cddc936e5d693c39ae0e87d2b4718e30745`.
Start/end revision is `94c7a24fc99053c220fb6c4d36849de6451c9eea`,
Windows 11 AMD64; all 205 inputs and recorded tool/SDK hashes were unchanged.
All 201 commands passed. Two fresh Unicode output directories each compile,
SG-link and execute 32 programs with exit 42; source, object, PE and observed
output hashes match across directories. A real SDK import program calls both
GetCurrentProcessId and GetCurrentThreadId. The SG verification-only numeric
SDK uses ExitProcess for checked overflow 63 and division-zero 62. Missing
runtime symbols reject with 65 and preserve the previous output. Every executed
command uses an empty PATH and removes source-runtime/toolchain overrides.
The report first revalidates passed emitter/linker evidence and exact generated
binary hashes. Those tools were built earlier with the Rust seed and clang;
this gate does not rebuild them or prove an S2/S3 fixed point, process tracing,
full SDK/runtime behavior, debugger support or complete M4/M5 parity.

Memory-range evidence: `target/selfhost/m4-memory-ranges/evidence.json`,
SHA-256 `027f4f81ca944a644d578e41ea0972fa6144645f0323e36e4e0a9d530ed67547`,
revision `c68790bb9c9dc168e353419ccd88da6e10e58042`, Windows 11 AMD64.
All 29 commands passed with unchanged inputs/tools. Existing ABI, native exact
resources, deterministic object, numeric, cleanup, stack probe and unwind
regressions remained green. The C oracle compares full buffers with libc memmove
for 25*25*41 overlapping range shapes for each of three MIR pointee sizes; byte
counts are never scaled by that size. Additional separate/unaligned ranges,
NULL zero count and exact spans adjacent to PAGE_NOACCESS prove byte access
boundaries. Typed array/reference and raw-pointer indices match independent
C addresses with 1/3/8/16 strides, including valid negative raw offsets.
The unsafe producer remains responsible for complete runtime byte extents;
slice metadata and complete M4 support remain open.

Readonly string evidence: `target/selfhost/m4-string-data-checked/evidence.json`,
SHA-256 `ab975fe008d845113ef43bb86003c9fe666d524f2d12eee113f6d3d111ddc5c6`,
revision `66043a401c96350bfdd5fed2a4b1326477a52f82`, Windows 11 AMD64.
All 29 commands passed with unchanged inputs/tools, including exact resource
scopes, repeat-identical objects and every preceding native regression.
The C caller observes the complete UTF-8/escape/non-BMP byte sequence, terminal
NUL, empty literal, interior NUL plus remaining bytes, and identical references
to one interned literal. VirtualQuery verifies the loaded literal storage is
not writable. Independent LLVM inspection accepts all data relocations and
unwind metadata after insertion of the additional section. The earlier trial
used an unsupported seed integer-to-char cast in its NUL fixture; the checked
fixture constructs the explicit NUL character instead. Literal bytes do not
implement owned String allocation, runtime glue or complete string semantics.

HIR3/source integration: `target/selfhost/m4-source-hir3-qualified/evidence.json`,
SHA-256 `9dc4e4f1e6f1969da7ce8a51c7d45650799335d4f26cd1e0d55a23ba473c2b80`,
start revision `c32037ff9941ae3f10094c5b17dd8909fd63a698`, Windows 11 AMD64.
All 325 commands passed with unchanged inputs/tools. Forty-six programs produce
repeat-identical SG objects and execute 42; forty-three also execute with equal
exit/stdout/stderr through the frozen reference. Array break and Result try
preserve named invalid-LLVM link failures, while tuple-loop destructuring
preserves the named source rejection; all three record `result_equal=false`.
Adjacent simple array loops and tuple field reads still require both executions.
Five negative cases preserve the previous output, including constant array
bounds and pending nested destructor glue. This checkpoint consumes HIR schema
3 but does not qualify complete semantics, ABI, ownership, runtime or M4.

Native object/debug address API evidence:
`target/selfhost/m4-native-object-debug-api/evidence.json`, SHA-256
`0facf37dfd7c481c6c701bbdc16d45cde427f41f8942b6180ab7edcc7c2a6d6f`,
revision `f1967f2f3167d3ef802f6dff17b2fcaa8d558d9f`, Windows 11 AMD64.
All 29 commands passed with unchanged inputs/tools. Ten resource scopes verify
optional address tables cover every emitted MIR instruction/terminator, all
prologues and every return epilogue. Reusing a populated builder/object rejects
with 6401. Debug collection on/off produces identical objects, also identical
to the preceding string checkpoint's `checked.obj`, SHA-256
`1d0b25ae1f50e18f165c050267ea241bc27405f53f5acef9709dc4c8a0374b61`.
All previous C ABI, numeric, cleanup, strings, memory, stack and unwind checks
remain green. These are machine-address facts, not qualified source debugging.
