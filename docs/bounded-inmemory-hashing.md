# Bounded in-memory SHA256

This additive development API supports native CLI context fingerprints larger
than the legacy 64-KiB hash limit. It requires a rebuilt toolchain/runtime;
published v0.2 binaries do not automatically contain it.

```sengoo
import std::hash;
let digest = ffi_buffer_new(64)?;
hash_sha256_str_bounded(serialized.as_str(), 786432, digest)?;
```

`hash_sha256_str_bounded(value, max_bytes, out)` and
`hash_sha256_bytes_bounded(pointer, length, max_bytes, out)` accept explicit bounds
from 1 through 16777216 bytes. They return 64 lowercase hexadecimal bytes in the
managed output buffer. The raw-byte function includes embedded NUL; the string
function retains existing borrowed-string semantics. Input is borrowed only for
the call, and no hash handles or retained allocations are created.

Length above the declared cap returns overflow status 13. Invalid maximum or
negative length returns invalid-argument status 2. Output requires 64 bytes.
Raw pointers must reference a valid range for the supplied length, as with the
existing raw-byte API. The synchronous work is bounded by max_bytes.

Legacy `hash_sha256_bytes` and `hash_sha256_str` continue to accept at most
65536 bytes. Their names and ABI are unchanged. This explicitly preserves the
old boundary instead of enlarging every existing caller's resource allowance.

`tools/stdlib/tests/hash_bounded.sg` checks independent SHA256 vectors at 0, 3,
65537 and 1048576 bytes, the exact legacy boundary, invalid bounds, output size
and embedded NUL. Windows x64 development evidence is recorded separately from
release or other-platform evidence in `target/file-media/hash-evidence.json`.
The CLI's preserved 90684-byte failure is under its
`target/context-refinement/hash-limit-before-run.log`.
