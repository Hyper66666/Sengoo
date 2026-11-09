## Encoding and object foundation

- [x] Implement bounded binary writer and deterministic x64 byte/COFF foundation (`target/selfhost/native-foundations/evidence.json`; full MIR/backend remains open).
- [ ] Implement x64 register/memory/immediate encodings and branch/call fixups with independent byte/disassembly tests.
- [ ] Emit valid deterministic COFF sections, symbols, data and relocations.

## Supported target behavior

- [ ] Implement target type layout and Windows x64 direct/indirect/C ABI calls, aggregates and SSE values.
- [ ] Lower every supported MIR instruction, control-flow terminator and block parameter correctly.
- [ ] Preserve overflow/division/shift/floating/contract-check semantics and ownership cleanup.
- [ ] Implement stack layout/probing/nonvolatile preservation and matching unwind records.
- [ ] Emit CodeView types/source/locals for complete Windows debug/PDB linking.
- [ ] Add register allocation and optimizations needed by the frozen performance gates.

## Integration

- [ ] Run native ABI/C interop and cross-compiler semantic corpus with exact inputs and output checks.
- [ ] Reject malformed MIR/oversized objects before emission and preserve source diagnostics.
- [ ] Verify source-level debugging through the SG linker when available.
- [ ] Record full revision/host/tool/input/commands/results and validate change/all specs strictly.
- [ ] Close M4 only after complete supported backend behavior passes; keep M5-M8 integration open.
