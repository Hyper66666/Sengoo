# Bounded raw stream decoder

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

This foundation does not establish gzip headers/checksums/concatenation policy,
tar extraction, package-manager compatibility, performance, final SDK packaging
or full M7/M8 completion.

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
