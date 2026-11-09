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

## Development object and stack-probe SDK boundary

The selected historical SG linker rejects ordinary clang COFF metadata. Build
the two small C SDK helpers as optimized stack-free leaves, independently
inspect their instructions, and remove only unreferenced metadata symbols and
empty data sections. Record the raw and normalized objects and explicit COFF
timestamp normalization. Independently compare retained section flags, machine
code, static data, relocations and target-symbol addresses; deliberately changed
code/data/addresses/relocations must fail that comparison. SG-produced objects
are never normalized. Record actual C header dependencies before compilation
and verify the same closure and hashes after compilation.

The qualified SG compiler emits a real `__chkstk` call for a large stack frame.
Kernel32's selected SDK library does not supply that symbol. A development-only
AMD64 import library may be generated from an explicit single-export DEF for
the actual system NTDLL export. Pin the DEF, library producer, resulting import
library and actual DLL identity/export; inspect the SG-linked PE imports and
execute its real stack-probing code. This is an explicit local OS/memory SDK
dependency, not a claim that the ordinary Windows SDK exposes this export or
that final SDK packaging and portable stack-probe support are qualified.
