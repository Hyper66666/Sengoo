# Debugging native Sengoo artifacts

Internal quickstart for debugging programs built with `sgc build` or `sgc run --engine native`.

## Build with debug symbols

Use a low optimization level so breakpoints map cleanly to source:

```powershell
sgc build path/to/main.sg -O 0 --debug-info
```

The native object and executable land under `build/` next to the source file (for example `build/main.exe` on Windows).
Windows-hosted MSVC debug builds also emit `build/main.pdb`; the compiler
selects CodeView metadata for that target and the linker writes full private
symbols. Cross-host Windows linking does not yet promise PDB production.
Without `--debug-info` / `-g`, Sengoo keeps the default IR free of debug
metadata and uses a separate artifact-cache dimension for debug builds.

## Windows (Visual Studio / WinDbg)

1. Build with `-O 0 --debug-info` as above.
2. Open the generated executable in Visual Studio (**Debug → Open Debug → File**) or launch WinDbg.
3. Set breakpoints on exported runtime helpers (for example `sengoo_assert_failure_v1`) when investigating assertion transport.
4. Pass program arguments through `sgc run` to reproduce CLI behavior, or run the executable directly from `build/`.

The Windows reference-host CDB proof is recorded in
[`debugging-native-windows-cdb.transcript`](debugging-native-windows-cdb.transcript).
It binds a Sengoo file/line breakpoint, steps from line 2 to line 3, reads the
`value` parameter as `21` and `doubled` local as `42`, then continues to normal
program completion. A Linux LLDB transcript remains a separate release-host
gate; Windows evidence is not used as a substitute for it.

### Windows VS Code launch configuration

For source-level native debugging from VS Code, install the Microsoft C/C++
extension and use `cppvsdbg` against an executable built with debug info:

```json
{
  "type": "cppvsdbg",
  "request": "launch",
  "name": "Debug Sengoo native executable (Windows)",
  "program": "${fileDirname}\\build\\${fileBasenameNoExtension}.exe",
  "cwd": "${fileDirname}",
  "preLaunchTask": "sengoo-build-debug"
}
```

Pair it with this task:

```json
{
  "label": "sengoo-build-debug",
  "type": "shell",
  "command": "sgc",
  "args": ["build", "${file}", "-O", "0", "--debug-info"],
  "problemMatcher": []
}
```

## Linux / macOS (lldb)

```bash
sgc build examples/01_hello.sg -O 0 --debug-info
lldb build/01_hello
(lldb) run
```

### Linux / macOS VS Code launch configuration

For a VS Code source-level debug launch, install a CodeLLDB-compatible extension
and point it at the native executable built by `sgc build --debug-info`:

```json
{
  "type": "lldb",
  "request": "launch",
  "name": "Debug Sengoo native executable (lldb)",
  "program": "${fileDirname}/build/${fileBasenameNoExtension}",
  "cwd": "${fileDirname}",
  "preLaunchTask": "sengoo-build-debug"
}
```

Pair it with this task:

```json
{
  "label": "sengoo-build-debug",
  "type": "shell",
  "command": "sgc",
  "args": ["build", "${file}", "-O", "0", "--debug-info"],
  "problemMatcher": []
}
```

The bundled `vscode-sengoo` extension also contributes a lightweight
`type: "sengoo"` debug entry for `sgc run` / build-and-run. Use that for quick
program execution from F5; use the `cppvsdbg`/`lldb` configurations above when
you need native breakpoints, stepping, and variable inspection.

Native debugger integration is covered by
`tools/sgc/tests/debugger_native.rs`. The test builds a minimal Sengoo program
with `sgc build -O 0 --debug-info --force-rebuild`, sets a breakpoint in
`debug_probe`, steps over the local initialization, and checks that the
debugger reports parameter `value` as `21` and local `doubled` as `42`.
The Unix LLDB lane also builds a composite probe, reads a struct, enum, owned
`String`, and `Vec<i64>` with their live members, steps into an ordinary
function call, steps over a closure invocation, and verifies the source
backtrace. Core-conformance CI runs this lane in fail-closed mode and uploads
the raw scalar and composite transcripts as
`debugger-native-lldb-transcripts`.

The driver uses LLDB in batch mode on Linux/macOS and a generated CDB command
file on Windows. Run it directly with:

```bash
cargo test -p sgc --test debugger_native -- --nocapture
```

The command generators and transcript parser always run as unit tests. The
native debugger session prints an explicit `SKIP debugger_native::...` reason
when the platform debugger or clang is absent; it never substitutes a metadata
inspection for the missing debugger. If the tools are present, build,
breakpoint, stepping, or value-inspection failures fail the test. The existing
`SENGOO_REQUIRE_NATIVE_DEBUGGER=1` is reserved for release-host automation and
turns every missing-tool skip into a failure. `SENGOO_DEBUGGER_TRANSCRIPT_DIR`
selects the directory for raw batch transcripts. The existing
`llvm-dwarfdump` tests in `tools/sgc/src/tests.rs` remain the broader portable
coverage for source files, entry lines, parameter/local DIEs, and core-language
surfaces. Object-level regressions cover named struct members, tuple index
members, owned `String`, monomorphized `Vec_i64`, and the lowered enum ABI.
Composite locals must retain both a `DW_AT_location` and a type reference;
members retain their base types and aligned byte offsets. Enum metadata exposes
the reliable runtime representation (`discriminant: i64` followed by bounded
`payload: u8[N]` storage). Source enum and variant names are not yet present in
MIR, so the debugger currently labels that composite type as `enum` rather than
claiming variant-aware inspection.

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
