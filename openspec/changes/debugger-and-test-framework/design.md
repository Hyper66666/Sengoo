## Context

The test framework, structured assertion transport, parametrization, and runtime
coverage are implemented. The remaining release blocker is debugger evidence:
object metadata exists, but statement stepping and live composite inspection
must be demonstrated by an installed debugger on supported hosts.

## Decisions

### Decision 1: Object metadata is necessary but not sufficient

`llvm-dwarfdump` validates metadata structure. A task closes only when LLDB or
CDB can stop, step, and read the expected source value from a native executable.

### Decision 2: Debugger evidence is host-tagged

Windows uses CDB/cppvsdbg evidence; Linux/macOS uses LLDB. Missing tools produce
an explicit skip for development convenience but do not satisfy the release
host gate.

### Decision 3: Optimize for stable O0 debugging first

Correct `-O0 --debug-info` stepping and inspection is required. Optimized debug
quality is documented as limited and is not an archive blocker for this change.

### Decision 4: Inspect representative scalar and composite layouts

The native transcript suite reads parameters and locals for scalar, struct,
enum, owned String, and `Vec<i64>`/generic Vec once available. It also steps
across calls and closures and checks the backtrace source names.

### Decision 5: Release-host debugger evidence is fail closed

Local development keeps an explicit skip when LLDB/CDB or clang is absent.
Release-host CI sets `SENGOO_REQUIRE_NATIVE_DEBUGGER=1`, so a missing tool,
failed debug build, unresolved breakpoint, incorrect live value, or absent
transcript fails the gate. The same run persists scalar and composite LLDB
transcripts as a retained Actions artifact; object metadata tests cannot
substitute for that artifact.

## Archive gate

- statement line tables rather than entry-only rows;
- live names/types/values for required scalar and composite locals;
- at least Windows plus one Unix-family release-host transcript;
- VS Code launch flow documented against the released toolchain;
- existing test/coverage JSON compatibility remains green.
