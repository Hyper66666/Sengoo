# Portable Targets

Sengoo currently has three build target families:

| Target | Artifact | Host toolchain | Current capability |
| --- | --- | --- | --- |
| `native` | platform executable | clang/LLVM or cached native artifacts | Full supported stdlib/runtime surface |
| `bytecode` | `.sgbc` | none at run time | Scalar MIR subset with internal function calls, branches, loops, phi nodes, and integer/boolean arithmetic |
| `wasm` | `.wasm` | WebAssembly runtime | Scalar MIR subset emitted as a core WebAssembly module exporting `main` |

The portable backends are deliberately conservative. They reject unsupported
MIR, FFI, and stdlib calls with diagnostics that point back to this document
rather than silently falling back to native code or miscompiling.

## Supported Source Shape

The first portable slice supports programs that lower to scalar MIR:

- `i64`, `bool`, `char`, unit, references/pointers represented as integer-like
  handles, and plain return values.
- Internal function calls, recursion, `if`/`else`, loops represented in MIR,
  `switch`, `goto`, and SSA `phi`.
- Unary and binary integer/boolean operations supported by MIR.

`sgc build --target bytecode input.sg -o app.sgbc` writes a versioned `SGB1`
bytecode file. `sgc run --target bytecode input.sg` compiles and interprets the
program without invoking clang, LLVM, or a native linker.

`sgc build --target wasm input.sg -o app.wasm` writes a core WebAssembly module
that exports `main`. WebAssembly `i64` results appear as BigInt values in
JavaScript hosts.

## Unsupported Areas

These features are not portable yet and remain native-only:

- FFI and host stdlib calls, including file, process, network, string-buffer,
  JSON, database, and reflection helpers.
- Heap-backed values, aggregate layout, arrays, enum payloads, closures, owned
  `String`, generic collections, and automatic `Drop` in the VM heap.
- WASI filesystem, environment, stdin/stdout, and clock imports.
- Async functions, futures, reactor I/O, and thread-pool execution.
- Program arguments for `sgc run --target bytecode`.

When a program touches one of these surfaces, the portable backend should fail
at compile time with a diagnostic naming the unsupported call or MIR construct.

## Forward Compatibility

The bytecode header is:

- magic: `SGB1`
- version: little-endian `u16`, currently `1`

Version `1` is for scalar MIR only. Future versions can add heap objects,
drop-glue opcodes, and host import tables without changing the native target.
