# SG Windows x64 native backend

## Why

The self-hosting frontend must eventually produce native code without emitting
LLVM IR or invoking an external assembler. Validated SG MIR is the boundary
between language semantics and target code generation.

## Changes

- Implement checked binary writing, x64 instruction selection/encoding,
  deterministic target layout, register/stack allocation and relocation.
- Emit standard COFF objects with symbols, data, relocations and Windows unwind
  metadata, plus CodeView input required by the SG linker's PDB writer.
- Preserve supported numeric, aggregate, reference, call, resource, reflection,
  FFI and async MIR semantics and optimization/contract-check settings.
- Verify native code through ABI interoperability, differential execution,
  malformed-MIR rejection, real debugger and resource/performance gates.

## Boundary

This change owns M4 code generation, not PE image linking/PDB aggregation or
SDK extraction. External tools may inspect/verify objects during development;
they are excluded from the final self-build path. An encoder or scalar example
does not complete M4 or permit default cutover.


## Checked C callback address erasure

Extend MIR schema one's instruction admission with opcode 28 for the bounded
physical move/drop callback ABI. Its operand is an actual local C-ABI function
value produced by opcode 9; its output is only raw mutable byte pointer storage.
This supplies the checked callback-address boundary needed by the separately
owned collection-intrinsics work. It does not certify an element type, Drop
body, provider, constructor or borrowed reference, and does not complete M4.
