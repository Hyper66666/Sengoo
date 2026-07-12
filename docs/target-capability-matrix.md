# Sengoo Target Capability Matrix

This matrix defines the intended target split for `sgc build --target` work.
Only `native` is implemented today. `wasm` and `bytecode` are planned targets
under `openspec/changes/wasm-and-bytecode-backends`.

| Capability | native | wasm | bytecode |
| --- | --- | --- | --- |
| Core scalar arithmetic/control flow | Supported | Planned | Planned |
| Structs/enums/match | Supported | Planned | Planned |
| Ownership + Drop | Supported subset | Planned subset | Planned |
| Generics/monomorphization | Supported subset | Planned | Planned |
| `String` | Supported | Planned via linear memory | Planned VM heap object |
| `Vec`/maps transitional stdlib | Supported subset | Planned subset | Planned subset |
| Filesystem | Supported subset | WASI sandbox only | Host bridge only |
| Env/args/time | Supported subset | WASI sandbox only | Host bridge only |
| Process spawning | Supported subset | Unsupported | Host policy dependent |
| Network | Supported subset | Unsupported by default | Host policy dependent |
| Async timers | Supported subset | Planned | Planned |
| Async sockets/files | Supported subset | Unsupported by default | Host policy dependent |
| C FFI | Supported subset | Unsupported except imports explicitly modeled by WASM host | Unsupported |
| Python interop | Supported host feature | Unsupported | Unsupported |
| Reflection sidecars | Supported opt-in | Planned metadata only | Planned metadata only |
| Debug info | Native DWARF subset | Planned DWARF/name section subset | VM trace metadata planned |

## Native

Native remains the default target. It emits LLVM IR/object code and links with
the runtime C source bundle through the configured native toolchain.

## WASM

The first WASM target should use a WASI-compatible host subset:

- allow pure language features, stdlib text/math/collections, args/env/time
  where WASI exposes them;
- gate filesystem APIs behind WASI rights;
- reject process, arbitrary native dynamic linking, Python interop, and default
  network APIs with stable unsupported-target diagnostics;
- verify downloaded/host-provided modules through normal package hashes.

## Bytecode

The bytecode target should be clang-free and deterministic. It should run core
conformance programs through an interpreter with an explicit value/heap model
that matches owned values and `Drop`. Host APIs should be routed through a
small bridge table so unsupported domains fail predictably.

## CLI Contract

Planned spelling:

```bash
sgc build --target native path/to/main.sg
sgc build --target wasm path/to/main.sg
sgc build --target bytecode path/to/main.sg
```

Until `wasm` and `bytecode` are implemented, the CLI should reject those target
values or mark them experimental instead of silently falling back to native.
