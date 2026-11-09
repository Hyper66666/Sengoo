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
- [x] Emit fixed-frame Windows x64 `.pdata/.xdata` unwind records and checked `__chkstk` probes for >4 KiB frames; verify LLVM inspection, `RtlLookupFunctionEntry`, `RtlVirtualUnwind`, deterministic objects and exact resource recovery (`target/selfhost/m4-unwind-final3/evidence.json`). Exception handlers, XMM saves and CodeView remain open.

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
