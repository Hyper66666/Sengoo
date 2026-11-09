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

`object.sg` extends the single-function object foundation with logical section,
symbol and relocation IDs. It writes multiple aligned initialized sections,
defined/undefined/absolute symbols, long UTF-8 names through the COFF string
table, and checked AMD64 ADDR64, ADDR32, ADDR32NB, REL32 through REL32_5,
SECTION and SECREL records. Relocation fields must lie within section data and
cannot overlap. File timestamps are zero, table order follows stable IDs, and
repeated emission of the same object produces identical bytes. Emission
revalidates every flat row rather than trusting builder calls; partially built
output is never returned as a successful object.

This initial object schema caps sections at 1,024, symbols/relocations at
100,000, per-section relocations at 65,535, output at 64 MiB, and alignment at
8,192. Auxiliary/weak/common/COMDAT records and BSS remain later work. BSS,
COMDAT and relocation-overflow section flags are rejected until their required
records are implemented. Native verification independently inspects names and
relocation kinds, then links a generated function that calls a C external,
loads another section through REL32 and exposes an ADDR64 data pointer.

`native.sg` is the first validated scalar MIR consumer. It emits x64 directly,
uses eight-byte stack slots and volatile scratch registers, spills all incoming
ABI parameters before user code, reserves Windows shadow/outgoing stack space,
preserves RBP and maintains stack alignment. CFG edge arguments use a separate
snapshot area before destination assignments so loop parameter swaps work.
Direct calls and function addresses use COFF REL32 records; indirect calls use
a register. The initial scalar path supports unit, bool, integer, char and
thin-pointer/function/handle values, integer arithmetic/bit operations, casts,
branches/switches, local addresses, narrow loads/stores and scalar calls.
Integer results are normalized at their declared width. Debug arithmetic
preserves the reference overflow hook for signed/unsigned add/subtract/multiply;
wrapping mode omits that hook. Scalar initialization facts may be elided only
because this initial consumer rejects every local requiring ownership cleanup.

At the initial checkpoint frames larger than 4,000 bytes rejected. Floating
values, aggregate values, string literal data, division/remainder, ownership/drop
and async operations remain explicit unsupported errors. The first path lacked
unwind/CodeView records. Independent C callers execute actual generated
machine code and test checked failure exits and wrapping results; raw MIR
validation and output limits are checked before publishing an object.

The current scalar path also emits fixed-frame `.xdata`/`.pdata` records. The
record describes the actual RBP push and fixed allocation, uses the shortest
Windows `UWOP_ALLOC_SMALL`/`UWOP_ALLOC_LARGE` form, and is linked through
section-relative relocations. The generated prologue is checked against the
record at each instruction boundary by `RtlVirtualUnwind`; the C probe also
looks up both generated functions through `RtlLookupFunctionEntry`. Frames over
one page emit a checked `mov RAX, size; call __chkstk; sub RSP, RAX` sequence;
the external `__chkstk` relocation points at the call displacement, and unwind
allocation offsets describe the longer probe prologue. A 600-local fixture
executes the probe, reads six ABI arguments, writes/loads its deepest local,
returns 42 and is found by a Windows stack walk. Exception/termination
handlers, chained entries, XMM saves and
CodeView locals remain open.

`x64_sse.sg` provides a separate checked scalar SSE encoder for f32/f64
memory operations, add/subtract/multiply/divide, ordered comparison instruction
bytes and integer-bit transfer to XMM. Legacy prefixes precede REX and opcode
bytes. Independent C fixture bytes and direct execution verify arithmetic,
loads/stores and a f64 bit pattern; negative mutation must fail before executing
the buffer. This encoder checkpoint does not yet integrate floating MIR, full
NaN comparison rules, conversions or the complete float call ABI.

The scalar MIR consumer now also accepts IEEE-bit float constants, f32/f64
arithmetic, unary sign changes, scalar memory and direct/indirect floating
returns/calls. Windows floating arguments use XMM0-XMM3 by argument position,
integer arguments retain their corresponding general-purpose positions and
fifth/later arguments use stack slots. Comparisons use UCOMIS plus explicit
parity handling to preserve ordered equality/relations and unordered inequality,
including NaN and signed zero. The C gate independently checks NaN, infinities,
mixed six-argument calls, f32 arithmetic and constant bits. Decimal literal
conversion and integer/float conversions remain explicit unsupported errors at
this checkpoint; an implicit integer bit copy is not a valid numeric cast.

`tools/native.sg` joins the SG frontend, semantic checker, owned HIR/MIR and
scalar consumer into a single-source development CLI. Its positional interface
is `INPUT OUTPUT.obj [--wrap]`; JSON schema-version-1 diagnostics identify the
failed phase, and output is opened only after every compiler phase succeeds.
It does not yet implement the public sgc command/module/configuration surface.
The source integration gate verifies exact input bytes against the qualified
Rust reference pinned in `reference-baseline.lock`, emitting LLVM independently
and linking both artifacts with the same C verification shim. Native SG object
emission itself does not invoke a compiler, assembler or linker.

The current aggregate consumer uses a validated reusable layout cache rather
than treating every MIR value as eight bytes. Value/local slots and parallel
CFG edge snapshots are sized and aligned for their actual types. Aggregate
construction defines padding deterministically; field addresses/extraction use
computed physical offsets, and scalar/aggregate memory loads/stores share
typed storage. A loop emits large copies without unrolling their entire body.

`abi.sg` classifies Windows parameters and results as unit, integer, SSE or
indirect. Small aggregates of 1/2/4/8 bytes use integer storage, arrays and other
aggregate sizes use aligned indirect copies, and indirect results insert a
hidden first argument. The caller copies indirect arguments before assigning
argument registers, preserving source SSA values even if an external callee
mutates its ABI copy. The callee spills incoming registers before copying
indirect parameters. Hidden returns copy into the caller's result slot and
return its address in RAX. Independent C execution covers 3/8/16-byte values,
enum tag/payload storage and a branch merging aggregate values. Ownership/drop,
full array indexing/bounds, typed overlapping moves and complete ABI behavior
remain separate open work.

Primary references:

- Intel instruction reference: https://www.intel.com/content/www/us/en/content-details/671110/intel-64-and-ia-32-architectures-software-developer-s-manual-combined-volumes-2a-2b-2c-and-2d-instruction-set-reference-a-z.html
- Windows ABI: https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention?view=msvc-170
- Prologue/epilogue and stack probing: https://learn.microsoft.com/en-us/cpp/build/prolog-and-epilog?view=msvc-170
- COFF: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
