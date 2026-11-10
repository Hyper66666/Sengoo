# Bounded raw stream decoder

## Incremental implementation owner and transition

`selfhost_core` owns the new SG continuation core and its subsequent shared
raw/gzip integration. The first prototype is `runtime/deflate_stream.sg`;
the qualified `deflate.sg` and `gzip.sg` remain unchanged until independent
chunked proof and explicit source-freeze coordination. A prototype does not
claim that the existing one-shot exports already use it. Final integration
must remove the duplicate implementation path and rerun all existing raw and
gzip qualifications before replacing those exports.

### Caller-owned continuation ABI

`init(state, extent, exact, input_limit, output_limit)` and `reset` initialize
exactly 45,056 caller-owned bytes; no heap or external handle is allocated.
`step(state, extent, input, length, output, capacity, finish)` returns 0
need-input, 1 need-output, 2 finished, -1 malformed/truncated, -2 cumulative
budget exhaustion, -3 invalid API extent/flag/field, or -4 uninitialized or
invalid state. `query(state, extent, field)` gives last-call consumed/produced
(0/1), cumulative consumed/produced (2/3), status (4), phase (5), or the fixed
state byte size (6). `dispose` erases the fixed state and releases no allocation;
the caller owns the memory. Unknown query fields and invalid extents reject
before memory access. Fresh/failed/finished queries have explicit results.

All C exports use primitive pointers/integers. The unsafe caller supplies
readable/writable disjoint extents for state, input and output, no concurrent
or reentrant access, and an intact opaque state produced by init/step. No
input/output pointer is retained. Bytewise copying a valid inactive state is
allowed and creates an independent decoding snapshot; there are no external
handles to alias. Arbitrary corrupted state is not a serialization trust
boundary, although all internal indexes, tree lengths, bits, matches and
counters remain bounded before memory access.

The state is 5,632 i64 words, with little-endian scalar memory access through
a minimal C memory adapter. Header words [0,64) use 512 bytes; the 320 code
lengths occupy words [64,384); literal, distance and code-length trees each
use 305 words at [384,689), [689,994), [994,1299), comprising 16 counts,
288 symbol slots and a used count. Tree-construction offsets use [1299,1315).
The 32 KiB ring history starts at byte 10,520 and ends at byte 43,288; the
remaining 1,768 bytes are reserved and zeroed. All word offsets are 8-byte
aligned. State size and peak storage do not grow with stream output.

Input budget is at most 64 MiB and output budget at most 512 MiB, cumulative
counts rather than arenas. The later package consumer independently enforces
256 MiB file content and a separate tar wire/metadata budget. A step may accept
at most 64 MiB input/output extents; normal use supplies small chunks. A
zero-length input may drain buffered symbols/matches, and zero capacity may
parse only until a pending byte requires output; that byte remains in state.
`finish` is sticky once asserted and subsequent calls must also assert it,
including retries of unconsumed input after need-output. Need-input at final
EOF becomes truncation. Prefix mode finishes at the final EOB's exact byte
boundary without reading a following framing byte; exact mode waits for final
EOF and rejects complete trailing bytes. Pending state includes partial
Huffman symbols, repeats, extras and overlapping matches across calls.

This stage does not implement gzip header/CRC continuation, incremental tar,
staging or atomic publication. It does not widen existing raw/gzip arenas.

Implement the raw format from [RFC 1951](https://www.rfc-editor.org/rfc/rfc1951),
including stored, fixed and dynamic Huffman blocks, code-length repeats,
length/distance extra bits, overlapping copies and history across blocks.
Use bounded stack tables for canonical Huffman decoding. Input and output are
separate caller-owned readable/writable ranges, each at most 64 MiB. The unsafe
caller guarantees those extents and non-overlap; arbitrary pointers are not
validated by the decoder.

Every input byte, output byte and back-reference is checked against its current
extent before an adapter call. Reject invalid lengths without accessing either
supplied range.
Return the decoded byte count on success, and distinct negative values for a
malformed/truncated stream, exhausted output capacity and invalid limits.
Failure may leave a decoded prefix within the authorized output range; it must
not claim successful publication or retain either pointer. A successful bounded
operation consumes exactly its supplied raw stream bytes, allowing the final
byte's unused padding bits. A future framed caller needs an explicit consumed
length contract instead of guessing where the next frame starts.

Reject reserved block/symbol uses, invalid stored complements, oversubscribed
trees, disallowed incomplete trees, repeats beyond the declared table, missing
end-of-block codes, distances beyond produced history and capacity overflow.
An empty distance alphabet is permitted only while no distance is decoded.
Resolve any format/oracle disagreement against the primary format and retain
the case explicitly rather than silently weakening either oracle.

The independent corpus includes multiple compressor levels/strategies, every
block kind, cross-block and overlapping distances, independently constructed
edge streams, truncations and malformed trees. Guard-page probes check actual
read/write extents, unchanged input and failure prefixes. Qualified SG emission
and SG linking must produce identical artifacts in two fresh directories;
clang may build only memory adapters and independent verification callers in
the explicit development SDK procedure. Source/tool/revision/host evidence is
required before any implementation task is checked.

The raw foundation is separately qualified below. The gzip framing increment
uses that same decoder; tar extraction, package-manager integration,
performance, final SDK packaging and full M7/M8 completion remain independent.

## Entry and implementation

`sengoo_sg_raw_inflate(source: *const u8, input_length: i64,
target: *mut u8, capacity: i64) -> i64` is an unsafe C entry. Success returns
the decoded byte count; -1 means malformed, truncated or an extra complete
trailing byte, -2 means insufficient output capacity, and -3 means an invalid
input/output limit. Both limit checks precede all supplied-range access. A
capacity failure can publish only a bounded decoded prefix; it is not success.
Neither zero output capacity nor an empty decoded stream makes an empty raw
input valid: a final block and its required data must still be present.

The SG implementation owns fixed stack arrays for canonical count/symbol
tables. The code-length alphabet must be complete; literal/length and distance
alphabets may have the format's single one-bit symbol, and an empty distance
alphabet is accepted only until a match requires a distance. Oversubscription,
missing end-of-block, repeat overruns and reserved decoded symbols reject.
Reads fetch exactly the next required input byte without speculative lookahead.
Thus success requires the consumed byte count to equal input_length while the
unused bits of its final byte remain padding. Stored alignment discards only
the current partial byte. Match copies read an already produced byte for each
new output byte, preserving overlap and history across blocks.

The current language admits unsafe function bodies through `unsafe extern C`
and restricts their public argument types to its supported C ABI primitives.
Private bit/symbol operations therefore use scoped SG declarative macros within
the actual unsafe entry; they evaluate width expressions once and preserve the
enclosing error return. Ordinary SG helpers build trees over typed references.
No integer-to-pointer cast or erased unsafe helper ABI establishes authority.
The three C memory functions only load an input byte, load an already produced
output byte, or store an output byte. They contain no format policy.

## Format and oracle boundaries

The dynamic HDIST field in RFC 1951 section 3.2.7 describes 1..32 lengths,
while zlib 1.3.2 rejects counts above 30 before reading unused symbols. This
decoder accepts independently constructed empty streams with HDIST 31/32 and
no decoded distance, but always rejects decoded reserved distances 30/31.
Those two acceptance differences remain explicit corpus ledger entries; they
are not represented as zlib parity. Ordinary compressed streams are checked by
the independent zlib decoder with EOF and unused-data checks.

Length symbol 284 with extra value 31 duplicates length 258. Although the
section 3.2.5 table labels 284 as 227..257 and reserves 285 for 258, existing
zlib and the repository's miniz tables accept the duplicate encoding. The SG
decoder accepts it for general stream compatibility and tests 284+30,
284+31 and 285 separately. Extra-bit integers are read least-significant-bit
first as required by the overall packing convention and deployed encoders.
[Reported erratum 7764](https://www.rfc-editor.org/errata/eid7764) identifies
the conflicting extra-bit prose; its status is Reported, not Verified.
Constructed non-palindromic length/distance extras test the actual direction.

## Evidence and development SDK

The formal gate consumes the frozen UTF-8 source-bound qualification, including
its actual SG emitter and SG formatter preview binaries, complete historical
linker chain, pinned Kernel32 import library and explicit actual-NTDLL stack
probe import. It verifies every consumed source, tool and artifact digest
before copying immutable inputs. The existing UTF-8 runner and locked Rust
formatter remain unchanged. Two fresh directories must reproduce untouched SG
objects, verified metadata-normalized C SDK objects and SG-linked PE bytes.

C leaf metadata normalization has the same explicit development-only boundary
as the UTF-8 gate: code/data bytes, section flags, relative symbol addresses and
each relocation target must survive unchanged. Nonempty fixture writable data
is retained; only empty metadata sections and unreferenced metadata symbols may
be removed. Raw C timestamps are recorded separately from normalized objects.
Actual header dependencies are discovered before compilation and verified
against the compiler's consumed dependency list. Instrumented tests prove read
order, initialized history and sequential bounded writes; production-adapter
tests report absent instrumentation as null, without claiming unobserved
failure-prefix/sequence counters.

## Shared prefix result and gzip framing

One private unsafe C-ABI raw decoder accepts an exact-input flag and returns
`(consumed_bytes << 27) | decoded_bytes` on success. Both counts are at most
64 MiB, so their disjoint fields fit within signed i64. The existing
`sengoo_sg_raw_inflate` exact-input export keeps its original four-argument
decoded-count result and errors. The new `sengoo_sg_raw_inflate_prefix` has
the same four arguments and returns the packed counts without rejecting bytes
after the final raw block. Both call the single shared LZ/Huffman algorithm;
neither an extra decoder nor pointer/integer authority is introduced. Prefix
tests check unused trailing bytes are not read and empty output accepts a null
zero-capacity pointer without a memory adapter call.

The unsafe `sengoo_sg_gzip_decode(source, input_length, target, capacity, mode)`
returns those same packed consumed/decoded counts, or -1 for a malformed
header/body/trailer, -2 for insufficient output, and -3 for an invalid mode or
negative/oversized extent. It rejects bad extents/modes before any supplied
range access. It validates the RFC 1952 magic, method and reserved flags;
examines the little-endian extra length and bounded zero-terminated name and
comment; verifies FHCRC as the low 16 bits of the header CRC32; and verifies
the output CRC32 and ISIZE trailer. With a 64 MiB output bound, decoded length
never wraps the 32-bit ISIZE field. Header metadata is skipped without being
published as a filename or interpreted as a filesystem path. FTEXT, MTIME,
XFL and OS do not change decoded bytes. CRC32 policy is SG code.

Mode zero decodes and fully verifies the first member, returning its precise
wire consumption and ignoring any following bytes. Mode one requires exactly
one member and EOF immediately after its trailer. Mode two requires at least
one member and consumes a strict concatenation to EOF. Empty input fails in
all modes. Neither strict mode permits inter-member or trailing zero padding;
Python gzip's extra tolerance is not an RFC requirement. Each member resets
raw history while output capacity is shared across the entire result.

The actual package path uses `flate2::read::GzDecoder` in
`tools/sgpm/src/resolver.rs`. Cached flate2 1.1.9 code uses first-member policy
and verifies CRC/ISIZE when its stream is fully read; its buffered reader may
read ahead, so its cursor position is not the SG consumed-wire contract.
FNAME and FCOMMENT each accept at most 65,535 non-NUL bytes, matching that
implementation's `MAX_HEADER_BUF`; the framing API retains this explicit
consumer compatibility bound even though Python zlib accepts longer metadata.
The separate Rust MultiGzDecoder boundary and zero/trailing policies are tested
explicitly rather than inferred from Python gzip.

The current tar consumer iterates entries lazily and is not demonstrated to
drain the gzip trailer on every path. Completing this bounded framing decoder
does not silently establish checksum enforcement or safe publication in that
package extraction path. Tar and actual consumer integration remain unchecked.
The C extension only offsets already bounded primitive pointers; offset zero
returns the original pointer directly so a null zero-capacity output does not
perform undefined C null-pointer arithmetic.
