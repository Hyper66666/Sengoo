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

`layout.sg` resolves the requested concrete MIR type into an independent
Windows x64 cache. It preserves source MIR tables, detects recursive by-value
nominals, retains pointer recursion, and checks every alignment, addition and
array multiplication before computing a size. Integers/floats use natural
alignment up to eight bytes; unit/never occupy zero bytes with alignment one,
char occupies four, pointers/string/function/future handles occupy eight, and
dyn values and references to dyn occupy sixteen. Nominal fields use their
separate declaration-order layout rows rather than generic type arguments.
Tuples and structs use padded field layouts; arrays use padded element stride.
Enums preserve the reference i64 tag plus at least one payload byte rounded to
eight-byte alignment, and payload field offsets include the eight-byte tag.
Explicit MIR size/alignment/offset claims must equal the computed layout.
Each enum variant row, when physically populated, records the entire enum's
size/alignment and its own absolute payload field offsets.

The initial resolver deliberately rejects missing nominal definitions,
unspecialized type variables/projections and raw slice values rather than
assigning a guessed representation. Its size bound is caller-selected up to
2,147,483,647 bytes, and recursive expansion is capped at 128 levels. This
checkpoint does not complete ABI argument/return classification or MIR codegen.
Native C independently verifies nested aggregate/array/enum storage offsets.

COFF section/symbol/relocation tables carry stable logical IDs until final
layout. Object files retain source/target/ABI identity, literal data and
required CodeView/unwind metadata. No external assembler generates machine
instructions on behalf of SG, and failure must not silently invoke LLVM.

Primary references:

- Intel instruction reference: https://www.intel.com/content/www/us/en/content-details/671110/intel-64-and-ia-32-architectures-software-developer-s-manual-combined-volumes-2a-2b-2c-and-2d-instruction-set-reference-a-z.html
- Windows ABI: https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention?view=msvc-170
- Prologue/epilogue and stack probing: https://learn.microsoft.com/en-us/cpp/build/prolog-and-epilog?view=msvc-170
- COFF: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
