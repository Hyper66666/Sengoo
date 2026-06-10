## Scope

This child change is a P3 follow-up under `mainstream-default-readiness`. It
exists to prevent ad hoc stdlib expansion by naming the next demand-backed owner
for concrete gaps that survived the completed or active stdlib waves.

Accepted ownership:

- `std::compress` gzip-compatible compression and decompression, replacing the
  current unsupported placeholder with a bounded supported subset.
- Realworld proof for compressed JSON/log/artifact workflows.
- A gate for later streaming JSON/schema helpers, but only after fixture demand
  exists.

Explicitly not owned:

- Core JSON parse/build/query APIs already owned by
  `stdlib-next-usability-wave` and `stdlib-production-surface`.
- Owned string returns, recursive IO, process pipes/background, sync fd IO,
  HTTP/TLS, text formatting, regex, config, hash, encoding, and basic file-watch
  support detection already covered by existing child changes.
- Terminal raw mode, file locks, long-lived watch streams, Unicode
  grapheme/normalization/locale behavior, async network execution, and broader
  server helpers.

## Demand And Fixture Model

Compression support must be justified by committed realworld fixtures, not by a
module existing in isolation. The minimum fixture is a package under
`examples/realworld` that:

- Reads or writes a compressed payload through public `std::compress` APIs.
- Uses compressed JSON, compressed logs, or compressed package artifacts rather
  than synthetic byte arrays alone.
- Runs through the locked package loop (`sgpm check/test/fmt/doc/build
  --locked`) or has an evidenced platform skip.
- Updates `examples/realworld/SUPPORT_MATRIX.md` from `Deferred` to
  `Supported subset` only after the fixture and tests pass.

Unit tests may cover byte-exact edge cases, but they do not replace the
realworld fixture.

## API And Ownership Rules

Compression APIs must preserve existing Buffer-based compatibility. Owned
`String` variants may be added only as additive helpers when the data is valid
UTF-8 and the owned-string ABI remains compatible.

The supported subset should prefer these shapes unless the spec is updated:

- `compress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer) ->
  Result<i64, i64>`
- `decompress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer) ->
  Result<i64, i64>`
- Optional raw byte-pointer bridge functions may remain internal or explicitly
  raw; public wrappers map failures into `std::status`.

Streaming compression handles are deferred. If later accepted, they must have
explicit `new/update/finish/close` lifecycle semantics and leak/closed-handle
tests.

## Pinned V1 Compression Contract

This change pins the first supported public surface to one-shot gzip-compatible
Buffer APIs:

```sg
def compress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer) -> Result<i64, i64>
def decompress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer) -> Result<i64, i64>
```

The public helpers operate on the first `input_len` bytes of `input` and write
at most `out.capacity` bytes to `out`. On success, `value` is the used output
length. On failure, `value` is `0` and `error` is a positive `std::status`
category.

V1 gzip output must be deterministic across supported hosts for the same input
and backend version. Implementations should normalize gzip metadata that is not
semantically meaningful for the Sengoo API, including modification time and
original filename. Decompression must validate the gzip trailer/checksum and
reject corrupt or truncated streams.

V1 resource ceilings are intentionally conservative and aligned with CLI/config
workflows:

| Limit | V1 default |
| --- | --- |
| Maximum one-shot compression input | 1 MiB |
| Maximum one-shot decompression input | 1,048,679 compressed bytes for v1 stored-gzip streams |
| Maximum decompressed output | min(`out.capacity`, 4 MiB) |
| Maximum decompression expansion ratio | 4x compressed input length, capped by the output limit |
| Maximum compressed output | `out.capacity` |

Tests must cover exact-limit success plus one-byte-over failure for input size,
output Buffer capacity, decompressed byte cap, and expansion ratio. Changing a
published ceiling later requires an OpenSpec update because it changes
user-visible resource behavior.

## Resource Limits

Implementation must use the V1 defaults unless this spec is updated:

- Maximum one-shot compression input: 1 MiB.
- Maximum one-shot decompression input: 1,048,679 compressed bytes for v1
  stored-gzip streams, which is the worst-case byte length emitted by the v1
  encoder for a 1 MiB input.
- Maximum decompressed output: min(`out.capacity`, 4 MiB).
- Maximum decompression expansion ratio: 4x compressed input length, capped by
  the output limit.
- Maximum compressed output: `out.capacity`.
- Behavior when output capacity is too small.
- Behavior for truncated, corrupt, or unsupported compression formats.
- Whether gzip headers, trailers, checksums, and modification-time metadata are
  preserved, normalized, or rejected.

Oversize input/output and expansion-limit failures must return a stable
category such as `STATUS_OVERFLOW`, `STATUS_BUFFER_TOO_SMALL`,
`STATUS_INVALID_ARGUMENT`, or a future OpenSpec-approved resource status.

## Stable Status Categories

Public wrappers must return positive `std::status` categories. Required mappings:

| Failure | Required category |
| --- | --- |
| Null/invalid Buffer, negative length, invalid handle | `STATUS_INVALID_ARGUMENT` or `STATUS_INVALID_HANDLE` |
| Output Buffer too small | `STATUS_BUFFER_TOO_SMALL` |
| Corrupt/truncated compressed data | `STATUS_PARSE` or `STATUS_INVALID_ARGUMENT` |
| Unsupported algorithm/header/platform backend | `STATUS_UNSUPPORTED` |
| Expansion/input/output limit exceeded | `STATUS_OVERFLOW` or an approved resource-limit category |
| Allocation failure | `STATUS_OUT_OF_MEMORY` |
| Host IO/backend failure where applicable | `STATUS_IO` |

No public wrapper may return a generic `1` when the cause fits a stable
category.

## Platform Behavior

Compression support must be deterministic across Windows and POSIX reference
hosts or documented as `Platform-specific` in the support matrix. Unsupported
hosts must compile and link successfully, then return `STATUS_UNSUPPORTED`
through public wrappers.

Implementations that adopt a backend library must document:

- Compression formats and levels supported by default.
- Whether output is byte-stable across platforms and backend versions.
- How checksum validation failures are surfaced.
- How unsupported metadata or dictionary features are rejected.

## Accepted V1 Implementation Notes

The implemented v1 subset is dependency-free gzip with stored deflate blocks.
This keeps `std::compress` link-safe on Windows and POSIX hosts without adding a
new native library dependency. Gzip output normalizes semantically irrelevant
metadata: `mtime=0`, no original filename/comment/extra fields, and `OS=255`.
The deflate payload is byte-stable for the same input because the encoder emits
bounded stored blocks rather than backend-selected Huffman tables.

Decompression validates the gzip magic, compression method, stored block
length/complement pairs, trailer CRC32, and ISIZE. V1 rejects gzip optional
metadata flags and non-stored deflate block types as `STATUS_UNSUPPORTED`;
corrupt, truncated, or checksum-mismatched streams return `STATUS_PARSE`.
Input, output, and expansion ceilings remain the pinned public contract:
compression input <= 1 MiB, decompression input <= 1,048,679 compressed bytes
for v1 stored-gzip streams, and decompressed output <= min(output Buffer
capacity, 4 MiB, 4x compressed input length). The decompression input ceiling is
slightly above 1 MiB so the maximum-size stream produced by the v1 encoder can
round-trip through the v1 decoder.

## Future Streaming JSON And Schema Gate

Streaming JSON parsing/serialization and schema validation remain deferred
until a realworld fixture proves one of these needs:

- A payload larger than the current JSON input cap must be processed without
  loading the full document.
- A package/test workflow must validate JSON schema before accepting generated
  metadata.
- A compressed JSON stream must be processed incrementally with bounded memory.

If accepted, the follow-up must define handle lifecycle, backpressure/output
ownership, schema dialect, error offsets, memory ceilings, and stable statuses.
This change does not approve those APIs.
