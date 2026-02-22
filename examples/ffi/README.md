# FFI Examples (M3 MVP)

This folder demonstrates both directions of C FFI:

- Sengoo -> C (`sengoo_calls_c.sg` + `c_add.c`)
- C -> Sengoo export (`sengoo_exports.sg` + `c_calls_sengoo.c`)

## 1. Sengoo Calls C

Build LLVM IR from Sengoo source:

```bash
cargo run -q -p sengoo-compiler --bin emit_ir -- examples/ffi/sengoo_calls_c.sg examples/build/ffi_sengoo_calls_c.ll
```

Link Sengoo IR with C implementation:

```bash
clang -Wno-override-module examples/build/ffi_sengoo_calls_c.ll examples/ffi/c_add.c tools/stdlib/runtime.c -o examples/build/ffi_sengoo_calls_c.exe
```

Run:

```bash
examples/build/ffi_sengoo_calls_c.exe
```

Expected output:

```text
42
```

## 2. C Calls Sengoo Export

Build LLVM IR for exported Sengoo symbol:

```bash
cargo run -q -p sengoo-compiler --bin emit_ir -- examples/ffi/sengoo_exports.sg examples/build/ffi_sengoo_exports.ll
```

Link C main with Sengoo IR:

```bash
clang -Wno-override-module examples/ffi/c_calls_sengoo.c examples/build/ffi_sengoo_exports.ll -o examples/build/ffi_c_calls_sengoo.exe
```

Run:

```bash
examples/build/ffi_c_calls_sengoo.exe
```

Expected output:

```text
42
```
