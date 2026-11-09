## Encoding and object foundation

- [x] Implement bounded binary writer and deterministic x64 byte/COFF foundation (`target/selfhost/native-foundations/evidence.json`; full MIR/backend remains open).
- [x] Implement x64 register/memory/immediate encodings and branch/call fixups with independent byte/CPU tests (`target/selfhost/native-foundations/evidence.json`).
- [x] Emit and inspect a deterministic single-text COFF object with short/long symbols; full sections/data/relocations remain open (`target/selfhost/native-foundations/evidence.json`).
- [x] Resolve bounded concrete Windows x64 type sizes, alignments, array strides and nominal/tuple/enum field offsets; verify recursive/overflow/claim failures and exact resources, and compare ten layout results with independently compiled C (`target/selfhost/m4-layout-final/evidence.json`). Full calling conventions remain open.
- [x] Emit deterministic multiple initialized COFF sections, short/long section and symbol names, external/static symbols and checked AMD64 relocations; independently inspect and link/execute external calls, cross-section loads and absolute data pointers (`target/selfhost/m4-object-checked/evidence.json`). Auxiliary/COMDAT/BSS/debug/unwind records remain open.
- [x] Implement and independently execute the initial validated scalar MIR consumer, including parallel CFG edge copies, integer widths, local memory, direct/indirect calls, six-argument Windows C calls, checked arithmetic failures and wrapping mode (`target/selfhost/m4-scalar-initialization/evidence.json`). Complete MIR/native ABI and cleanup support remain open.

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
