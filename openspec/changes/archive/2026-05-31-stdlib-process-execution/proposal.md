## Why

Sengoo scripts can inspect their process context and read user arguments, but
they still cannot run another program without leaving the language. Small build
helpers, generators, and automation scripts need a portable process-execution
primitive that does not hide shell parsing or injection risk.

## What Changes

- Add synchronous shell-free child-process helpers to `std::process`.
- Support zero through three explicit string arguments while Sengoo lacks a
  general `Vec<&str>` or string-slice ABI.
- Return the child exit code for normal termination and an error-shaped
  `Result` when startup, waiting, or termination handling fails.
- Inherit the current process standard streams so the first slice stays small
  and predictable.
- Add compiler, `sgc`, `sglsp`, runtime, documentation, and runnable-example
  coverage.
- Explicitly defer arbitrary argv, shell convenience helpers, stdio capture,
  pipes, cwd/environment overrides, background handles, timeouts, signals, and
  async execution.

## Impact

- Affected spec: `stdlib-mainstream-usability`
- Affected code: `tools/stdlib/process.sg`, `tools/stdlib/runtime.c`,
  compiler/`sgc`/`sglsp` stdlib tests, and `examples/stdlib`
- Dependencies: none
- Syntax changes: none
