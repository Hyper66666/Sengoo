# SG text algorithm boundary

`selfhost/runtime/utf8.sg` exports fixed C functions for bounded UTF-8 validation
and validated byte boundaries. It enforces shortest scalar encodings,
continuation ranges, surrogate exclusion and the U+10FFFF ceiling. NUL, BOM and
noncharacters remain valid; normalization is not performed. The independent
grammar reference is [RFC 3629, sections 3 and 4](https://www.rfc-editor.org/rfc/rfc3629).

`sengoo_runtime_memory_read_u8(pointer,index)` performs one raw byte load,
without text policy, allocation or handles. Every call follows SG bounds checks.
The unsafe ABI requires the complete readable input range; it does not validate
arbitrary pointers. Empty input permits null and performs no load. Negative
and over-64-MiB lengths reject without loads. The bound matches the native
buffer bound; continuation checks use remaining length before adding offsets.

Boundary queries validate the entire input, then accept zero/end or scalar
starts. Invalid text, continuation positions and out-of-range offsets reject.
This experimental operation does not expose the private C boundary helper's
precondition that its input was already validated.

An independent strict decoder supplies corpus expectations. A C caller checks
the SG native object, input immutability, protected-page extents and adapter
read counts. The development gate may compile its adapter/probe with qualified
clang; the algorithm itself is emitted by the pinned SG compiler. No owned
runtime resource is allocated. Owned storage, final SDK materialization,
concurrency, performance and full M7/M8 completion remain separate.
