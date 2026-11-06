# Bounded Base64 for large binary buffers

This additive development stdlib API keeps edition 2026, runtime ABI 1 and
the existing encoder's 65536-byte limit unchanged. It requires the matching
updated Sengoo source library; published v0.2 packages do not acquire it
automatically. Its implementation is Sengoo, using the existing native codec.

```sengoo
import std::encoding;
let mut output = ffi_buffer_new(((input.used_len() + 2) / 3) * 4)?;
let size = encoding_base64_encode_buffer_bounded(
    &input, input.used_len(), 8388608, &mut output
)?;
```

`max_bytes` must be 1 through 16777216. The input length must fit initialized
`used_len`, and source and output must be distinct buffers. Invalid arguments
return status 2. Input above the chosen maximum or insufficient output capacity
returns status 13 before conversion. Output is cleared after validation; empty
input succeeds even with zero output capacity. Discard output on any error:
later conversion failures may leave partial bytes, which must not be sent.

The result includes every binary input byte, including NUL. Internally the
helper uses 65535-byte chunks and one 87380-byte scratch buffer; this preserves
exact Base64 padding at chunk boundaries. Input/output are borrowed for the
call and the scratch is released on every exit. The caller still owns output
allocation and its selected maximum bounds synchronous work.

`tools/stdlib/tests/encoding_bounded.sg` checks independent vectors, legacy
65536/65537 behavior, cross-chunk padding, initialized bytes, maxima, capacity
and binary NUL. Local Windows x64 logs and hashes are recorded separately in
`target/encoding-bounded-evidence.json`. CLI original-image conversion has
separate loopback evidence and is not inferred from these library vectors.
