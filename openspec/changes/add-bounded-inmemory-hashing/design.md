# Design

Extract the existing SHA256 implementation behind a private input-limit helper.
Keep its old public entry point calling that helper with 65536. Add a separate
entry point requiring max_bytes in 1..16777216, nonnegative length not exceeding
that maximum, and at least 64 bytes of declared output capacity. The managed
buffer API continues to validate actual output storage. Return status 13 for an
input above its explicit limit and status 2 for an invalid bound or length.

The raw-byte interface hashes exactly the supplied byte range, including NUL.
The string interface follows the existing borrowed-string contract. No copies,
threads, heap-owned handles or resumable state are introduced. Synchronous
execution remains bounded by the explicit cap.

The CLI hashes exact serialized context using 786432 as its bound. Its source
SHA remains ordinary SHA256, not a concatenated-chunk checksum. Replacing the
legacy hash with a custom chunk digest or temporary file was rejected because
those alternatives would change its meaning or introduce unnecessary disk I/O.

Regression vectors are compared with Python hashlib outputs for empty input,
abc, 65537 x bytes, one MiB of x bytes and x/NUL/y. The original 64-KiB exact and
one-byte-over boundary are independently checked. A preserved CLI failure log
shows the 90684-byte source passes history validation but fails legacy hashing.
