# SG self-hosting program

## Why

Sengoo's compiler and development tools are implemented in Rust. Production
native builds emit LLVM text and invoke clang, and part of the runtime is Rust.
Writing the toolchain in SG will exercise the language on its own development
workload and permit a Windows native build path independent of those tools.

The starting checkpoint is
`cc70247ec5bd9531bea751a08288980da40ecc68`, published to `origin/main` before
self-hosting implementation. That checkpoint preserves the Agent SDK Windows
development preview and its compiler ownership corrections. Its existing
evidence does not qualify a self-hosted compiler or a new stable release.

## What changes

Implement `sgc`, `sgpm`, `sgfmt` and `sglsp` in SG, sharing one source, syntax,
diagnostic and semantic core. Add an SG x64 machine-code generator and an SG
COFF/PE linker, including Windows native debug information. Migrate runtime
algorithms to SG while retaining a minimal C platform layer and locked native
SDK inputs. Preserve the documented edition 2026 and Windows Stable and
Supported-subset contracts.

This umbrella owns sequencing, requirement ownership and integration evidence.
Child changes own implementation requirements. `add-selfhost-foundations`
owns the first seed lock, explicit stage0 entry and SG source/span/symbol/node
arena foundation. Later child changes must be registered before their
implementation requirements are treated as complete.

## Impact and limits

The first completion target is Windows x64. Linux and macOS remain on the
reference implementation until separately ported and qualified. Experimental
WASM, bytecode, Cranelift and graphics retain their independent scope.

Stage0 may use the frozen Rust compiler, clang and source runtime. Final native
self-rebuilding must use SG tools, SG sources and the locked native SDK without
Rust, clang/LLVM, an external assembler or an external linker. Rebuilding the
C/third-party SDK from its published sources is a separate procedure.

A first subset compiler that compiles itself is an intermediate milestone.
Default replacement requires all four tools, supported Windows compatibility,
real native debugging, reproducibility and the performance gates in design.md.
