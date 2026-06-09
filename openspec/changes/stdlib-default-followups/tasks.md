## 1. Ownership And Inventory

- [x] 1.1 Validate this change with `openspec validate stdlib-default-followups --strict`.
- [x] 1.2 Reconfirm that `stdlib-production-surface`,
  `stdlib-next-usability-wave`, `stdlib-breadth-mainstream`, and
  `stdlib-https-tls` still own their completed or active stdlib surfaces.
- [x] 1.3 Keep `mainstream-default-readiness` inventory pointed at this child
  for compression/default-follow-up ownership without marking P3 complete.

## 2. Compression Contract

- [x] 2.1 Define one-shot gzip-compatible compression and decompression public
  APIs, preserving existing Buffer helper compatibility.
- [x] 2.2 Pin the public v1 names to
  `compress_gzip_buffer(input, input_len, out)` and
  `decompress_gzip_buffer(input, input_len, out)`, returning
  `Result<i64, i64>` with used output length on success.
- [x] 2.3 Define input, output, decompression expansion, and Buffer-capacity
  limits before implementation: 1 MiB compression input, 1,048,679 byte v1
  stored-gzip decompression input so max-size encoder output can round-trip,
  min(output Buffer capacity, 4 MiB) decompressed output, 4x compressed-input
  expansion cap, and compressed output capped by the output Buffer capacity.
- [x] 2.4 Define deterministic gzip metadata behavior, including mtime,
  original filename, header handling, trailer/checksum validation, and whether
  output bytes are stable across supported hosts.
- [x] 2.5 Map invalid buffers, corrupt payloads, unsupported formats, oversize
  outputs, allocation failure, and backend/host failures to stable
  `std::status` categories.
- [x] 2.6 Document platform behavior for Windows and POSIX reference hosts,
  including byte-stability and checksum behavior.

## 3. Realworld Proof

- [x] 3.1 Add or update a committed `examples/realworld` fixture that uses
  public `std::compress` APIs for compressed JSON, logs, or package artifacts.
- [x] 3.2 Run the fixture through the locked package loop or record an evidenced
  platform skip.
- [x] 3.3 Update `examples/realworld/SUPPORT_MATRIX.md` from `Deferred` only
  after the fixture and tests pass.

## 4. Optional Future Data Follow-Ups

- [x] 4.1 Do not add streaming JSON, JSON schema validation, or streaming
  compression handles unless a fixture-backed OpenSpec update accepts the API
  shape and resource model.
- [x] 4.2 Streaming JSON/schema/streaming compression remains deferred; no new
  handle lifecycle was accepted in this change.

## Archive Gate

- [x] `openspec validate stdlib-default-followups --strict` passes.
- [x] `openspec validate mainstream-default-readiness --strict` passes after
  inventory ownership edits.
- [x] Compression support has tests for success, corrupt input, small output,
  oversize/expansion limits, invalid handles, and unsupported-platform behavior.
- [x] Gzip output determinism and checksum/trailer validation are tested.
- [x] A realworld fixture proves the supported path through public stdlib APIs.
- [x] Support matrix rows use only `Supported subset`, `Platform-specific`,
  `Deferred`, or `Accepted risk` with proof paths.
