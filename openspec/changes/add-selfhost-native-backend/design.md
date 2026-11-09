# Native target architecture

Consume only validated typed MIR. Keep Windows x64 type layout and the
language-to-C call ABI explicit, including integer extension, SSE floating
arguments/returns, aggregate passing and indirect calls. Reserve shadow space,
preserve nonvolatile registers, maintain required stack alignment, probe large
frames and emit matching unwind records for the actual prologue/epilogue.

Implement instruction bytes in SG from Intel's architectural reference, with
checked register numbers, operand widths, ModRM/SIB/displacements, immediates
and branch/call fixups. Use a bounded binary writer whose partial append and
patch failures cannot publish malformed output. Tests compare both exact byte
encodings and independently disassembled/executed instructions.

Initially allocate MIR values to explicit stack slots and use bounded scratch
registers. Add linear-scan allocation and supported MIR optimizations only
after correctness and ABI tests pass. Numeric overflow/division/shift/NaN and
contract checks follow the existing supported language behavior rather than
whichever instruction sequence is easiest to generate.

COFF section/symbol/relocation tables carry stable logical IDs until final
layout. Object files retain source/target/ABI identity, literal data and
required CodeView/unwind metadata. No external assembler generates machine
instructions on behalf of SG, and failure must not silently invoke LLVM.

Primary references:

- Intel instruction reference: https://www.intel.com/content/www/us/en/content-details/671110/intel-64-and-ia-32-architectures-software-developer-s-manual-combined-volumes-2a-2b-2c-and-2d-instruction-set-reference-a-z.html
- Windows ABI: https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention?view=msvc-170
- Prologue/epilogue and stack probing: https://learn.microsoft.com/en-us/cpp/build/prolog-and-epilog?view=msvc-170
- COFF: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
