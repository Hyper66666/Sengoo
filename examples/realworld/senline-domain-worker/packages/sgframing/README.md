# sgframing

`sgframing` is an incubating `0.x` Sengoo package for bounded
`u32-big-endian length + payload` framing over exact standard I/O.

## Scope

- `frame_validate_length` validates non-zero caller-supplied bounds.
- `frame_prefix_encode` and `frame_prefix_decode` operate on four-byte
  big-endian prefixes.
- `frame_read_stdin` distinguishes clean EOF before a prefix from a truncated
  frame and returns an owned payload Buffer for a complete frame.
- `frame_write_all_with` retries an injected writer from the exact next offset,
  rejects zero progress and over-reported counts, and preserves writer errors.
- `frame_write_with` composes a complete prefix/payload frame over injected
  writer and flusher callbacks. It initializes binary mode before either
  callback so Windows cannot translate protocol bytes.
- `frame_write_stdout` writes a prefix and payload completely, then flushes.
- `frame_init_stdio_binary` enables binary protocol streams, including
  `_O_BINARY` on Windows.

The caller owns product limits. Senline currently supplies 32 KiB input and
8 KiB output limits; those values are not package defaults. The current
runtime Buffer allocation limit is 64 MiB, so a larger caller maximum can
still fail allocation.

Stable package errors are `SGFRAMING_ZERO_LENGTH`,
`SGFRAMING_LIMIT_EXCEEDED`, `SGFRAMING_INVALID_PREFIX`,
`SGFRAMING_TRUNCATED`, and `SGFRAMING_INVALID_PAYLOAD`. Allocation,
binary-mode, and output I/O failures currently retain their underlying
stdlib status. This mixed error surface must be resolved before `1.0`.

## Incubation Status

- First consumer: `senline-domain-worker`.
- Independent consumers: none.
- Supported evidence: locked Windows source-development package tests, real
  binary stdin/stdout boundary coverage, API docs, release build, and a local
  publish dry run.
- Missing evidence: installed Windows/Linux toolchains, package license
  metadata/files, registry publication, and an independent consumer.

This package is not a general stream abstraction and is not eligible for
`std::` promotion yet.
