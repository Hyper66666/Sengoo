## Why

Passing functional tests is necessary but insufficient for a mainstream-usable
language. Sengoo crosses Rust, C, OS, network, FFI, allocator, and generated-code
boundaries. Failures in those boundaries often appear only under malformed
input, sanitizer instrumentation, long-running concurrency, different hosts, or
version skew.

The project also needs a compatibility contract and measurable performance
budgets so users can upgrade and operate Sengoo with confidence.

## Proposal

- Add retained-corpus fuzzing for lexer/parser/typecheck/MIR and package/archive
  inputs.
- Add sanitizer, leak, invalid-handle, and long-running async/concurrency gates.
- Version the runtime/toolchain ABI and document source compatibility,
  deprecation, editions, and supported-host policy.
- Turn compile time, peak RSS, artifact size, startup, and representative
  runtime throughput into non-regression budgets.
- Run realworld and selected first-party packages with installed release
  archives, not only workspace binaries.
- Define security and release-response expectations for registry, FFI, TLS, and
  archive extraction boundaries.

## Impact

- CI workflows, compiler/runtime/tool tests, fuzz targets/corpora, release
  metadata, compatibility docs, realworld fixtures, and support matrix.
- Parent: `language-maturity-roadmap`, Phase 4.
- No new source syntax is required.

## Non-goals

- Formal verification of the compiler.
- Claiming every third-party library or OS version is supported.
- Optimizing benchmarks by weakening correctness, diagnostics, or safety.
- Counting package quantity as ecosystem quality without release-loop evidence.
