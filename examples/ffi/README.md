# FFI Examples

This folder demonstrates both directions of the C FFI boundary:

- Sengoo -> C: [`sengoo_calls_c.sg`](sengoo_calls_c.sg) calls `c_add`
  from [`c_add.c`](c_add.c).
- C -> Sengoo: [`c_calls_sengoo.c`](c_calls_sengoo.c) calls the exported
  `sengoo_add_export` symbol from [`sengoo_exports.sg`](sengoo_exports.sg).

## Requirements

- `cargo`
- `clang`
- `make`

## Build And Run

```bash
make -C examples/ffi call-c
make -C examples/ffi c-calls-sengoo
```

Expected output for both targets:

```text
42
```

Clean generated files:

```bash
make -C examples/ffi clean
```
