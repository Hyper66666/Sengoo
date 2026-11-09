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

The source-to-object CLI now uses the reference-compatible decimal converter
for MIR literal symbols. Expanded exact-source comparisons cover generic
reuse/nesting/recursion/aggregate fields, structured returns, f32/f64 literal
arithmetic, inherited reads/mutable updates and independently mangled same-name
methods, along with the earlier scalar subset. The native path emits COFF
without invoking the reference; the latter is run separately by the gate.

`float_literal.sg` uses bounded base-2^30 rational integers, exact remainder
comparison and nearest-even rounding to produce IEEE bits entirely in SG. It
handles subnormals, signed zero, overflow and underflow. `sh_float_literal`
rounds directly to a requested target; the edition 2026 source path uses
`sh_float_literal_reference` to preserve the reference parser's initial f64
rounding before f32 conversion. Fifteen actual reference IR/C executions prove
that compatibility at double-rounding boundaries. Input limits are 16,384
bytes, 4,096 significant digits and 1,024 integer limbs; errors 6801 and 6802
report malformed literals and bounded-resource failures. Signs are accepted
by the reusable API although the source lexer normally represents a unary sign
separately. Decimal fractions, exponents, separators and f32/f64 suffixes are
supported; inf/nan/hex spellings are not parsed as decimal literals.

The scalar conversion path uses SSE integer/float conversion instructions with
explicit unsigned-64 handling. Float-to-integer conversions check NaN and both
range boundaries before truncating, preserving the reference saturating
intrinsics: NaN yields zero, negative unsigned inputs yield zero, and overflow
clamps to the signed or unsigned limit. The upper half of u64 is handled around
2^63 without signed overflow. Integer-to-float u64 conversion rounds a shifted
sticky-bit value and doubles it; f32/f64 conversion uses the corresponding SSE
width conversion. Independent C cases cover NaN, infinities, signed zero,
in-range truncation, boundaries and 4,096 deterministic integer bit patterns.

Signed and unsigned integer division/remainder use explicit high-half
preparation and the appropriate DIV/IDIV instruction. Debug division calls the
existing division-by-zero hook with the typed divisor. Signed minimum divided
by minus one follows the existing numeric overflow contract: debug uses the
overflow hook, wrapping mode returns the minimum value (or zero remainder).
The LLVM reference emits an overflowing signed divide with no defined result
for that corner; the independent native test checks the specified overflow
contract rather than treating that reference expression as a value oracle.
Defined nonzero signed/unsigned divisions and remainders are checked against C.

The initial ownership consumer allocates per-local initialization flags and a
bounded cleanup scratch region in the described fixed frame. Flags begin false,
explicit MIR initialization/move facts update them, conditional root drops
test them, and an executed root drop clears the flag. User Drop calls use the
validated target function and saved receiver address. Structural cleanup walks
tuple/struct fields and array elements in reverse; tagged enum cleanup selects
only the active payload. Projection cleanup leaves the root flag untouched.
Unknown resource glue, explicit pending glue and conditional projection drops
remain unsupported errors until the necessary runtime or field facts are
available. A type name never authorizes an external resource free. The current
defensive rejection list for known unresolved runtime wrappers is temporary;
the typed MIR glue-kind table is the authoritative contract.

MIR validation rejects a forged destructor target whose call graph recursively
invokes another Drop target, indirect call or projected cleanup. Source lowering
marks unresolved nested field release as pending until per-field initialization
facts can suppress automatic cleanup exactly. The native consumer never treats
pending glue as a successful no-op.

Opcode 19 computes element addresses from the resolved physical element stride
for fixed arrays and raw pointers. Typed slice metadata remains unsupported.
Opcode 22 consumes the validator's unsigned byte-count contract. Its inline
memmove selects forward copying when destination is at or below source and
backward copying otherwise. It uses only volatile registers and one-byte loads
and stores, leaves the direction flag untouched, and performs no access at zero
count. It does not invoke a C memory routine, scale counts by pointee size,
guess runtime bounds, or transfer an owning value. The binary helper rewinds
all emitted bytes if its capacity is exhausted.

Decoded MIR string symbols are emitted as explicit-length UTF-8 bytes followed
by one NUL byte. Interior NULs are preserved, and empty literals still receive
their terminator. Interned symbol identity deduplicates storage deterministically.
Private `.sg.literal.N` symbols cannot collide with source identifiers, and
RIP-relative references target readonly `.rdata`. The object writer accounts
for total code/data capacity; literal emission does not authorize resource
allocation or resolve pending owned String glue.

`sh_native_new` and `sh_native_emit_into` provide a fresh-state object builder
before serialization, allowing a separate debug adapter to attach COFF records
without introducing a debug/linker dependency into normal native emission.
The ordinary `sh_native_object` wrapper retains its byte output. Optional
`collect_debug` records global MIR instruction/terminator start/end tables,
per-function prologue ends and `(function,start,end)` epilogue triples. Addresses
are absolute `.text` offsets and ends are exclusive. Frame allocation and local
slot offsets remain separate; a Windows SP-relative local is allocation minus
slot, and frame metadata includes the separately saved RBP. The caller must
discard partially built state on failure. Source/initialization/lifetime facts
must still be proved by the external adapter before claiming debug variables.

Primary references:

- Intel instruction reference: https://www.intel.com/content/www/us/en/content-details/671110/intel-64-and-ia-32-architectures-software-developer-s-manual-combined-volumes-2a-2b-2c-and-2d-instruction-set-reference-a-z.html
- Windows ABI: https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention?view=msvc-170
- Prologue/epilogue and stack probing: https://learn.microsoft.com/en-us/cpp/build/prolog-and-epilog?view=msvc-170
- COFF: https://learn.microsoft.com/en-us/windows/win32/debug/pe-format
