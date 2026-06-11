# Debugging native Sengoo artifacts

Internal quickstart for debugging programs built with `sgc build` or `sgc run --engine native`.

## Build with debug symbols

Use a low optimization level so breakpoints map cleanly to source:

```powershell
sgc build path/to/main.sg -O 0
```

The native object and executable land under `build/` next to the source file (for example `build/main.exe` on Windows).

## Windows (Visual Studio / WinDbg)

1. Build with `-O 0` as above.
2. Open the generated executable in Visual Studio (**Debug → Open Debug → File**) or launch WinDbg.
3. Set breakpoints on exported runtime helpers (for example `sengoo_assert_failure_v1`) when investigating assertion transport.
4. Pass program arguments through `sgc run` to reproduce CLI behavior, or run the executable directly from `build/`.

## Linux / macOS (lldb)

```bash
sgc build examples/01_hello.sg -O 0
lldb build/01_hello
(lldb) run
```

For a failing test with structured assertions:

```bash
sgc test --exact tests/smoke.sg
# Inspect SENGOO_ASSERT_REPORT handling in tools/sgc/src/commands/test.rs when extending the runner.
```

## Useful environment variables

| Variable | Purpose |
| --- | --- |
| `RUST_BACKTRACE=1` | Prints a Rust backtrace when the embedded runtime panics during toolchain development |
| `SENGOO_ASSERT_REPORT` | Set by `sgc test` to a runner-owned JSON envelope path (do not set manually unless reproducing assertion transport) |

## When native linking fails

Confirm `clang` is on `PATH`. Native `sgc build` and native `sgc run` require
LLVM/clang 15+ for the opaque-pointer backend contract; clang 19 matches the
core conformance CI. On Windows, install LLVM and ensure `clang.exe` is
discoverable. The `sgc` command prints the selected linker during `run`/`build`.
