# Bounded raw stream decoder

Implement the raw format from [RFC 1951](https://www.rfc-editor.org/rfc/rfc1951),
including stored, fixed and dynamic Huffman blocks, code-length repeats,
length/distance extra bits, overlapping copies and history across blocks.
Use bounded stack tables for canonical Huffman decoding. Input and output are
separate caller-owned readable/writable ranges, each at most 64 MiB. The unsafe
caller guarantees those extents and non-overlap; arbitrary pointers are not
validated by the decoder.

Every input byte, output byte and back-reference is checked against its current
extent before an adapter call. Reject invalid lengths without any memory access.
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
