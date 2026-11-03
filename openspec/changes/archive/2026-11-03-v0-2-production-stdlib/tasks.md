## 1. Preserve compatibility baselines

- [x] 1.1 Inventory existing file/io/net/String/Buffer APIs and runtime symbols.
- [x] 1.2 Add regression tests for current one-shot and concrete methods.
- [x] 1.3 Freeze Reader/Writer signatures, status mapping, Unicode 17.0.0, and
  `chars()` migration in docs before implementation.
  - `docs/unicode-v0-2.md`; design D2–D5.

## 2. Reader/Writer core

- [x] 2.1 Add `Reader`/`Writer` traits and `read_to_end`, `write_all`,
  and `copy_stream` helpers (`tools/stdlib/stream.sg`; runtime-owned Cursor).
- [x] 2.2 Implement EOF, partial read/write, zero-progress, capacity,
  checked-total, and error-path tests
  (`stdlib_m3_stream_cursor_read_write_copy`, including zero leaked Cursor handles).
- [x] 2.3 Add file, stdio/owned-fd, and TCP adapters while retaining old methods.
  - `AsyncFile`, `FdStream`, and `TcpStream` implement the supported trait side;
    existing concrete methods remain source-compatible.
- [x] 2.4 Prove bounded capacity behavior in stream helpers (no unbounded growth).

## 3. Unicode foundation

- [x] 3.1 Unicode 17.0.0 version/provenance metadata (`unicode_version_copy`).
- [x] 3.2 `chars()` projects `char`; `char_codepoint` migration helper.
- [x] 3.3 Strict `string_from_utf8`, `char_count`; `string_from_buffer` strict
  wrapper. Full property tables / casefold deferred honestly.
- [x] 3.4 Test ASCII, invalid UTF-8, status mapping (`stdlib_m3_invalid_utf8_*`).
- [x] 3.5 Document byte vs scalar; defer normalization/graphemes/collation/locale
  (`docs/unicode-v0-2.md`).
- [x] 3.6 `STATUS_INVALID_UTF8 = 20` + name/message + raw mapping; 0–19 retained.

## 4. Production HTTP dependency

- [~] 4.1 `http-production-serving` remains **open residual owner** (handlers,
  keep-alive, streaming, TLS server not proven). Matrix row records residual.
- [ ] 4.2 Realworld service fixture with handlers/keep-alive/streaming/TLS —
  residual under HTTP owner.
- [x] 4.3 Stream/Unicode public APIs available for composition; HTTP boundary
  documented as residual.

## 5. Verification and archive

- [x] 5.1 Native stream/UTF-8 gates via `sgc` stdlib native runtime tests.
- [x] 5.2 `sgc` stdlib M3 tests green.
- [x] 5.3 Existing String iterator / status surfaces retained.
- [x] 5.4 Stdlib docs, language features, support matrix updated.
- [ ] 5.5 Archive this corrective active change after residual HTTP note and
  current-SHA verification are recorded.
- [~] 5.6 The original change is archived as
  `2026-07-16-v0-2-production-stdlib`; this post-archive remediation remains active.
