## Context

The recent stdlib usability slices made filesystem scripts more realistic, but
there is still no standard way for a Sengoo program to participate in Unix-like
pipelines or write diagnostics to stderr. The language already has managed
`Buffer` handles, so stdin can follow the same output/input byte-copy convention
used by files, paths, args, process, and reflection wrappers.

## Goals / Non-Goals

**Goals:**

- Add a minimal synchronous `std::io` module.
- Keep stdin reads Buffer-backed until an owned-string ABI exists.
- Let programs write exact `&str` bytes to stdout or stderr.
- Keep the behavior portable and easy to test in `sgc` runtime smoke tests.

**Non-Goals:**

- No async integration or scheduler-facing I/O.
- No terminal modes, prompts, coloring, TTY checks, or stream handles.
- No file descriptor passing.
- No rich errno mapping.

## Decisions

### Decision 1: Prefix functions with `io_`

The module name is `std::io`, but source-level functions keep an `io_` prefix to
match existing stdlib naming and avoid collision with built-ins such as
`print(...)`.

### Decision 2: Stdin reads copy into managed Buffers

`io_stdin_read(buffer)` reads up to the buffer capacity from stdin. `io_stdin_read_line(buffer)`
reads up to the buffer capacity or through one newline, whichever comes first.
Both return the byte count on success, including `0` for EOF.

### Decision 3: Writes do not append newlines

`io_stdout_write` and `io_stderr_write` write exactly the bytes in the `&str`.
Callers can include `\n` explicitly or use existing `print(...)` for newline
stdout output.

### Decision 4: Flushes are fallible

`io_stdout_flush` and `io_stderr_flush` return `Result<bool, i64>`, preserving the
current convention that runtime I/O failure is surfaced through `Result`.

## Risks / Trade-offs

- **Risk:** Buffer-backed stdin is less ergonomic than returning strings.  
  **Mitigation:** keep examples direct and revisit when Sengoo has an owned
  string/byte-slice ABI.
- **Risk:** stderr writes make example smoke tests more complex.  
  **Mitigation:** add focused runtime coverage that asserts stderr separately.
- **Risk:** Users may expect async I/O because Sengoo has async features.  
  **Mitigation:** document this module as synchronous only and defer scheduler
  integration to a later spec.

## Migration Plan

1. Add compiler, `sgc`, and `sglsp` tests for `std::io` import visibility.
2. Add runtime coverage for stdin reads, stdout writes, stderr writes, and flushes.
3. Implement `tools/stdlib/io.sg`.
4. Add C runtime helpers using `stdin`, `stdout`, and `stderr`.
5. Wire `io` into `sgc` and `sglsp`.
6. Add `examples/stdlib/13_io.sg` and docs.
7. Run focused tests and the standard verification gate.
