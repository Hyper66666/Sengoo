## 1. Preserve compatibility baselines

- [ ] 1.1 Inventory existing file/io/net/String/Buffer APIs and runtime symbols.
- [ ] 1.2 Add regression tests for current one-shot and concrete methods.
- [ ] 1.3 Freeze Reader/Writer signatures, status mapping, Unicode 17.0.0, and
  `chars()` migration in docs before implementation.

## 2. Reader/Writer core

- [ ] 2.1 Add `Reader`/`Writer` traits and generic `read_to_end`, `write_all`,
  and `copy_stream` helpers.
- [ ] 2.2 Implement EOF, partial read/write, zero-progress, flush, capacity,
  checked-total, and error-path tests.
- [ ] 2.3 Add file, stdio/owned-fd, and TCP adapters while retaining old methods.
- [ ] 2.4 Prove exact ownership/Drop and no unbounded allocation under sanitizer
  and resource-limit gates.

## 3. Unicode foundation

- [ ] 3.1 Add deterministic Unicode 17.0.0 version/provenance metadata and table
  generation or checked-in tables.
- [ ] 3.2 Make `String.chars()`/`str.chars()` project `char`; add
  `char_codepoint` migration helper and update iterator tests.
- [ ] 3.3 Add strict `string_from_utf8`, `char_count`, scalar property helpers,
  simple upper/lower mapping, and locale-independent `casefold`; retain
  `string_from_buffer` as a strict compatibility wrapper.
- [ ] 3.4 Test ASCII, multibyte BMP, supplementary planes, combining sequences,
  invalid UTF-8, case expansion, and output/resource limits.
- [ ] 3.5 Document byte length/indexing versus scalar count and explicitly defer
  normalization, graphemes, collation, and locale behavior.
- [ ] 3.6 Add stable `STATUS_INVALID_UTF8 = 20` mappings, name/message helpers,
  raw-runtime mapping tests, and compatibility checks for status values 0-19.

## 4. Production HTTP dependency

- [ ] 4.1 Complete/archive `http-production-serving` in its existing lane.
- [ ] 4.2 Add a realworld service fixture using registered handlers, bounded
  keep-alive, streamed output, and real TLS where the platform claim is proven.
- [ ] 4.3 Compose service request/response data with public Reader/Writer and
  Unicode APIs where their ownership models permit; document any intentional
  boundary instead of adding ad hoc bridges.

## 5. Verification and archive

- [ ] 5.1 Run runtime native-bridge net/I/O tests and sanitizer/leak gates.
- [ ] 5.2 Run `sgc` stdlib/runtime tests and all affected realworld locked loops.
- [ ] 5.3 Run compiler generic-trait/String iterator tests and `sglsp` stdlib
  metadata tests.
- [ ] 5.4 Update stdlib docs, language reference, and support matrix.
- [ ] 5.5 Run warnings-denied Clippy, formatting, and strict OpenSpec validation.
- [ ] 5.6 Archive this change after `http-production-serving` is archived.
