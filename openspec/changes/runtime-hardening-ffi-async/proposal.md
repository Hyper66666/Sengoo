## Why

Sengoo has accumulated runtime capabilities across async execution, stdlib C
bridges, networking, process helpers, reflection/native linking, and FFI. The
mainstream production gap is reliability: unsupported paths, optional platform
features, panics, resource limits, and handle lifecycle failures must be
explicit and testable instead of surfacing as crashes or unresolved symbols.

## Proposal

- Stabilize async/concurrency runtime semantics for scheduling, cancellation,
  timeouts, task status, error propagation, and cleanup.
- Make dynamic FFI either actually supported on a host platform or explicitly
  unsupported through stable status errors.
- Document and test platform behavior for filesystem, process, networking,
  path encoding, signals, termination, and permissions.
- Add runtime panic/backtrace/debug context and handle lifecycle validation.
- Add resource-limit and security-boundary tests for commands, paths, network,
  config, regex, JSON, compression, FFI, and large inputs.

## Impact

- Updates runtime C/Rust bridges, native toolchain linking, async runtime tests,
  FFI tests, stdlib runtime tests, docs, and diagnostics.
- Existing APIs remain source-compatible, but previously silent crashes or
  unresolved optional symbols must become explicit status failures.
- This is a P2 hardening lane and does not add broad new stdlib modules by
  itself.

## Non-Goals

- No new language-level async syntax beyond existing accepted syntax.
- No public network/TLS guarantee without platform support tests.
- No unsafe dynamic FFI fallback that lies about support.
- No background task/process API expansion without a separate OpenSpec.
