# Network and JSON implementation evidence

Owner: network/data agent. Baseline Sengoo revision:
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a`.
Verification host: Windows x64, 2026-11-04; working-tree candidate over that
baseline (not a committed release SHA). Native compiler: bundled clang 19.1.7.

- [x] Implement bounded TCP/TLS worker, copied request, per-stream queues,
  pending/bytes/EOF/error states and managed Buffer used length.
- [x] Implement cancellation, connect/handshake and idle deadlines, idempotent
  close, Drop, bounded resolver limitation, and live resource counters.
- [x] Verify certificate chain/hostname and one explicit connection-local root;
  do not modify trust stores or log authorization.
- [x] Decode JSON Unicode escapes/surrogate pairs; reject invalid raw UTF-8 and
  invalid escapes; preserve parsed NUL bytes in values/keys and serialization.
- [x] Repair recursive JSON parent pointers after node-array reallocation.
- [x] Run `cargo test --locked -p sengoo-runtime --lib --features native-bridge
  net -- --test-threads=1`: 54 passed, 0 failed, 0 ignored (48 existing + 6 new).
- [x] Add `JsonDoc.copy` with exact-value independent deep copy and rollback on
  stale source, cycles, depth, node and byte limits.
- [x] Run `cargo test --locked -p sgc --test json_unicode -- --test-threads=1`:
  2 passed, 0 failed, 0 ignored (Unicode and transactional deep-copy tests).
- [x] Preserve `async` through attribute scanning when `#` occurs in ordinary
  strings or modifiers are separated by comments. Run `cargo test --locked -p
  sengoo-compiler --lib parser::attribute_expander::tests -- --test-threads=1`:
  8 passed, 0 failed, 0 ignored, including both new parser/type-check regressions.
- [x] Run `cargo test --locked -p sgc --test net_byte_stream --
  --test-threads=1`: 1 passed, 0 failed, 0 ignored after the parser fix. Its
  source contains a `#` string and imports the complete standard network module.
- [x] Add checked raw `Buffer.get_u8`, `consume` and `index_of_bytes`, source
  bounds for managed append and overlapping self-copy. Run `cargo test --locked
  -p sgc --test buffer_bytes -- --test-threads=1`: 1 passed, 0 failed, 0 ignored
  (12.23 s), covering split UTF-8, NUL, range failures and stale handles.
- [x] Add source-borrowing `Buffer.append_copy` while retaining `append_from`.
  Repeat the buffer native test: 1 passed (12.52 s), including 50 conditional
  repeated copies with source readability and constant native handle counts.
- [x] End temporary method argument loans when concrete resolved return types
  contain no borrows; retain references in generic payloads, struct fields and
  overlapping call arguments. Six targeted positive/negative regressions pass.
  Repeat `buffer_bytes`: 1 passed (15.12 s), including 50 alternating mutable
  reads and shared copies. Run the complete compiler library suite with clang
  on PATH: 1151 passed, 0 failed, 0 ignored (69.03 s). Full output is recorded
  locally in `%TEMP%/sengoo-compiler-borrow-final.log`.
- [x] Repair zero-field aggregate construction: emit a typed value instead of
  an allocation pointer. The new native ABI regression covers construction,
  return, value arguments, shared/mutable receivers and nested empty fields;
  all 13 struct-codegen tests pass. Restore the original empty `BufferReader {}`
  in `buffer_bytes`: 1 passed (14.32 s). Final compiler library suite with clang:
  1152 passed, 0 failed, 0 ignored (64.32 s), with full local output at
  `%TEMP%/sengoo-compiler-empty-final.log`.
- [x] Run `cargo test --locked -p sgc --test json_unicode --test net_byte_stream
  -- --test-threads=1`: the initial Unicode/stream pair passed before the new
  deep-copy test (2 passed, 0 failed, 0 ignored). Tests compile and run real
  synchronous Sengoo source using the source-development runtime and clang.
- [ ] External Sencode HTTP/SSE integration and candidate-wide release evidence
  remain the root integrator's gates; native transport tests do not replace them.

The runtime TLS fixture verifies four simultaneous authorized requests with
Unicode JSON and fragmented raw responses, one independently backpressured
queue, certificate rejection, cancellation during handshake/full queue, idle
and handshake deadlines, and eventual zero handles/workers. Native source tests
verify Buffer content/length, four owned streams, cancellation and Drop, Unicode
round trips, 14 invalid Unicode cases, NUL preservation and recursive growth.

Limits: maximum 8 handles/workers, 1 MiB initial request, queue 4 KiB..1 MiB.
The operating system resolver cannot be interrupted portably; canceled or timed
out resolution retains its worker slot until it returns. A single global native
deadline monitor is process-scoped and excluded from per-request worker counts.
No Linux/macOS run or new cross-platform release success is claimed here.
JSON builder/lookup inputs retain the existing C-string NUL boundary; parsed
JSON values and keys preserve embedded NUL internally and during serialization.
