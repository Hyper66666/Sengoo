# Decode general raw DEFLATE in SG

The package port must preserve general DEFLATE/gzip compatibility; the existing
stored-block stdlib helper is insufficient. Start with a bounded raw DEFLATE
decoder written in SG, covering stored, fixed and dynamic Huffman blocks.
Gzip framing, tar extraction, package resolution and installed replacement
remain separate integration work.

This child owns the allocation-free decoding algorithm and independent tests.
M3/M4 own language and native lowering; native SDK owns provider packaging.
Minimal C adapters may read or write one byte of caller-owned memory and must
contain no compression policy. No Rust or C decompressor implements the SG
algorithm. Independent zlib execution supplies comparison data only.
