## Why

This active copy is a post-archive corrective change. It reopens over-claimed
stream ownership and Unicode wording from the 2026-07-16 archive; archive tasks
remain open until this remediation passes the current gates.

Sengoo's 35-module stdlib already supports files, processes, networking, JSON,
config, compression, regex, databases, and concurrency. The remaining native
default-path gaps are composability and text correctness: file/stdin/TCP reads
use similar but unrelated APIs, partial I/O and EOF lack one trait contract,
Unicode iteration still exposes transitional scalar typing, and the HTTP server
does not yet provide production handlers, keep-alive, streaming responses, and
TLS server proof.

## What Changes

- Complete/archive the existing `http-production-serving` owner change.
- Add source-level synchronous `Reader` and `Writer` traits with pinned Buffer,
  partial-I/O, EOF, flush, timeout, cancellation, and ownership semantics.
- Adapt supported file, stdio/fd, and TCP types without removing existing APIs.
- Pin the v0.2 Unicode provenance to 17.0.0 and complete strict UTF-8 validation,
  `char` iteration, and scalar counts while explicitly deferring table-backed
  properties, simple case mapping, and default case folding.
- Add realworld fixtures that compose these public APIs under resource bounds.

## Capabilities

### Modified Capabilities

- `stdlib-mainstream-usability`: add bounded stream composition and Unicode text
  foundation requirements and consume production HTTP archive evidence.

`stdlib-http-server` remains owned by `http-production-serving`; this change
does not duplicate its handler/keep-alive/streaming/TLS requirements.

## Impact

- Runtime and stdlib I/O/string/network adapters, status mapping, docs, tests,
  and realworld fixtures.
- Existing Buffer capacity/used-length and String byte-length semantics remain
  source-compatible except the documented `chars()` item-type correction.

## Non-Goals

- Async `Reader`/`Writer` traits, process-pipe stream handles, or arbitrary
  zero-copy borrowing.
- Unicode normalization, grapheme segmentation, collation, locale-specific case
  mapping, or embedded locale databases.
- HTTP/2, WebSocket expansion, proxying, or a web framework.
- Replacing existing one-shot helper APIs in v0.2.
