# Explicit bounded JSON documents

New `json_parse_bounded(text,max_bytes)`, borrowed
`json_parse_buffer_bounded(buffer,input_len,max_bytes)` and
`json_doc_object_bounded(max_bytes)` keep the caller-selected limit in the native
document. Accepted bounds are 1 through 16 MiB. Invalid bounds or oversize raw
lengths are rejected before input bytes are read; buffer input must also fit its
initialized used length.

Existing parsers retain 1 MiB. Extended documents retain 64-depth/4096-node
limits and add total container-link/payload limits. Their fixed admission
reservations bound aggregate owned-document memory conservatively; owned
String/Buffer outputs remain subject to their own runtime ownership. Documents
remain single-threaded and not Send/Sync. Copy failure rolls back additions.

Serialization bounds output growth and detects cycles/depth in extended
documents. No partial serialized success is returned. The independent large
legacy reproducer is run before implementation. Public and native regressions
exercise exact/over bounds, invalid pointers with rejected lengths, embedded
NUL/Unicode, copy rollback, repeated release and admission exhaustion.
