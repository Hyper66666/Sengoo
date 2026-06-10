## ADDED Requirements

### Requirement: Compression helpers SHALL be demand-backed and bounded

Sengoo SHALL promote compression from a deferred placeholder only when a
committed realworld fixture demonstrates a compressed JSON, log, or package
artifact workflow through public `std::compress` APIs. Compression helpers SHALL
define API shape, output ownership, resource limits, platform behavior, and
stable failure statuses before implementation.

#### Scenario: A realworld fixture proves compression demand

- **WHEN** compression support is claimed as supported or supported subset
- **THEN** `examples/realworld` contains a committed fixture that reads or writes
  compressed JSON, logs, or package artifacts through public `std::compress`
  APIs
- **AND** the fixture passes the locked package loop or records an evidenced
  platform skip
- **AND** `examples/realworld/SUPPORT_MATRIX.md` cites the fixture and does not
  leave compression as a stale deferred row

#### Scenario: One-shot compression preserves Buffer ownership

- **WHEN** a program calls public one-shot gzip-compatible compression or
  decompression helpers with managed `Buffer` inputs and outputs
- **THEN** successful helpers return the number of meaningful bytes written
- **AND** existing Buffer capacity semantics remain source-compatible
- **AND** any owned-string helper is additive and only succeeds for valid UTF-8
  output

#### Scenario: V1 gzip API names are stable

- **WHEN** compression support is promoted
- **THEN** `std::compress` exposes
  `compress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer)` and
  `decompress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer)` returning
  `Result<i64, i64>`
- **AND** the success value is the used output length
- **AND** failures use positive `std::status` categories

#### Scenario: Gzip metadata and checksum behavior is deterministic

- **WHEN** the same bytes are compressed on supported hosts
- **THEN** semantically irrelevant gzip metadata such as modification time and
  original filename is normalized or documented so fixture outputs remain
  deterministic
- **AND** decompression validates trailer/checksum data and rejects corrupt or
  truncated payloads with stable status categories
- **AND** the v1 supported subset is documented if it intentionally rejects
  gzip optional metadata or non-stored deflate block types

#### Scenario: Compression enforces resource limits

- **WHEN** input bytes, output bytes, decompression expansion ratio, or Buffer
  capacity exceed documented limits
- **THEN** the helper returns an error-shaped result with a stable
  `std::status` category
- **AND** the helper does not allocate unbounded memory, write past the output
  Buffer, or return a partially successful result

#### Scenario: Compression failures use stable statuses

- **WHEN** compression or decompression fails because of an invalid handle,
  invalid argument, too-small Buffer, corrupt or truncated payload, unsupported
  format/backend, allocation failure, expansion limit, or host/backend I/O
  failure
- **THEN** the public wrapper maps the failure to `STATUS_INVALID_HANDLE`,
  `STATUS_INVALID_ARGUMENT`, `STATUS_BUFFER_TOO_SMALL`, `STATUS_PARSE`,
  `STATUS_UNSUPPORTED`, `STATUS_OUT_OF_MEMORY`, `STATUS_OVERFLOW`, or
  `STATUS_IO` as appropriate
- **AND** it does not collapse known causes into a generic `1`

#### Scenario: Unsupported platforms remain link-safe

- **WHEN** the compression backend is unavailable on a host
- **THEN** `sgc check`, `sgc build`, and `sgc run` still link programs that
  import `std::compress`
- **AND** public compression helpers return `STATUS_UNSUPPORTED`
- **AND** the support matrix records the platform-specific or deferred behavior

### Requirement: Streaming data helpers SHALL require fixture-backed follow-up design

Sengoo SHALL keep streaming JSON parsing/serialization, JSON schema validation,
streaming compression handles, and dynamic data-object mapping gated behind a
later OpenSpec update with committed realworld demand, lifecycle semantics,
memory ceilings, platform behavior, and stable statuses. These helpers must not
be added opportunistically.

#### Scenario: A future fixture needs streaming JSON or schema validation

- **WHEN** a realworld workflow needs to process JSON beyond the documented
  one-shot cap, validate package/test metadata against a schema, or combine
  compressed JSON with bounded memory
- **THEN** a child change defines the parser or validator API shape, schema
  dialect where applicable, handle lifecycle, resource ceilings, output
  ownership, platform behavior, and stable statuses before implementation
- **AND** the existing one-shot JSON helpers remain source-compatible

#### Scenario: No fixture-backed demand exists

- **WHEN** implementation agents are working on stdlib thickness without a
  committed fixture that needs streaming JSON, schema validation, streaming
  compression, terminal control, file locks, long-lived watch streams, richer
  Unicode behavior, or broader network helpers
- **THEN** those features remain deferred rather than being added as ad hoc
  stdlib surface area
