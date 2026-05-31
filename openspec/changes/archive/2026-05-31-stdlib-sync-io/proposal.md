## Why

Sengoo can now handle files, paths, directories, process metadata, and command
arguments, but command-line tools still cannot read standard input or write to
standard error through the standard library. That leaves common pipeline-style
programs awkward compared with mainstream scripting languages.

## What Changes

- Add a `std::io` source module for minimal synchronous process I/O.
- Provide Buffer-backed stdin reads:
  - `io_stdin_read(buffer: Buffer) -> Result<i64, i64>`
  - `io_stdin_read_line(buffer: Buffer) -> Result<i64, i64>`
- Provide string writes and flushes:
  - `io_stdout_write(data: &str) -> Result<i64, i64>`
  - `io_stderr_write(data: &str) -> Result<i64, i64>`
  - `io_stdout_flush() -> Result<bool, i64>`
  - `io_stderr_flush() -> Result<bool, i64>`
- Wire `std::io` through `sgc`, `sglsp`, docs, and runnable examples.

## Non-Goals

- No async I/O, nonblocking I/O, terminal raw mode, prompts, or TTY detection.
- No file descriptor/socket abstraction.
- No owned-string stdin return ABI.
- No automatic newline behavior beyond whatever bytes the caller writes.
- No new third-party dependencies or source-language syntax.

## Impact

- Affected code:
  - `tools/stdlib/io.sg`
  - `tools/stdlib/runtime.c`
  - `tools/sgc/src/stdlib_imports.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `compiler/src/tests/stdlib_surface_tests.rs`
  - `tools/sgc/src/tests.rs`
  - `examples/stdlib/*`
  - `tools/stdlib/README.md`
- Existing `print(...)` behavior and current stdlib modules must remain
  backward-compatible.
- Verification follows the stdlib pattern: focused red/green tests,
  `cargo fmt --check`, compiler/sgc/sglsp tests, OpenSpec validation, and
  `git diff --check`.
