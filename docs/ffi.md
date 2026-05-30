# Sengoo FFI MVP (M3)

This document describes the current FFI MVP surface.

## Supported Syntax

## External declarations

```sg
extern "C" {
    pub fn c_add(a: i64, b: i64) -> i64;
    pub fn c_strlen(value: &str) -> i64;
    pub unsafe fn read_buffer(ptr: *mut u8, len: usize) -> i64;
}
```

## Exported functions

```sg
#[no_mangle]
pub extern "C" fn plain_export(x: i64) -> i64 {
    x
}

#[export_name = "sengoo_add_export"]
pub extern "C" fn sengoo_add(a: i64, b: i64) -> i64 {
    a + b
}
```

## Link attribute

```sg
#[link(name = "crypto")]
extern "C" {
    pub fn sha256(data: *const u8, len: usize, out: *mut u8);
}
```

## Compile-time Checks

- ABI allowlist (MVP): `"C"`, `"cdecl"`, `"system"`
- Unsupported ABI reports compile error
- Non-FFI-safe types in extern signatures report compile error
- Raw pointer signatures require explicit `unsafe` boundary

## FFI-safe MVP type set

- `()`
- integer types (`i8/i16/i32/i64/i128/isize/u8/u16/u32/u64/u128/usize`)
- `f32`, `f64`
- `bool`, `char`
- immutable `&str` parameters, lowered as null-terminated `i8*` C strings
- raw pointers (`*const T`, `*mut T`) where `T` is recursively FFI-safe

Rejected examples:

- references other than immutable `&str` (`&T`, `&mut T`)
- bare `str`, slices, tuples, ADTs, function values

## End-to-end Repro

Use the example scripts in:

- `examples/ffi/README.md`

The folder includes both directions:

- Sengoo -> C
- C -> Sengoo (exported symbol path)

## Runtime Bridge (MVP)

Runtime-level FFI and Lua bridges are documented in:

- `docs/runtime-ffi-lua.md`
- `docs/database-runtime.md`
- `docs/runtime-network-bench.md`
- `docs/runtime-protobuf-ffi.md`

These runtime APIs provide:

- C library open/call/close path
- Lua load/exec/call path (subset + Lua 5.4 native bridge PoC)
- Loopback network benchmark path with p50/p95/p99 metrics
- Protobuf wire encode/decode FFI path for integration validation
- C++ reuse wrapper primitives (object lifecycle + callback relay + payload buffer handles)
- Structured error code + message diagnostics for local integration loops
