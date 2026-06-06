# Sengoo Examples

Small, runnable programs that show the language surface without hiding
the important syntax in a larger app.

Run a single standalone example with:

```bash
sgc run examples/01_hello.sg
```

Some FFI examples need `clang` and are driven by `make`; see
[`ffi/README.md`](ffi/README.md).

## Categories

| Category | What it demonstrates |
|---|---|
| [`async/`](async/) | `async def`, `sleep`, `spawn`, `select`, and task lifecycle APIs |
| [`generics/`](generics/) | Generic structs, generic impl methods, `Option<T>`, and `Result<T, E>` |
| [`realworld/`](realworld/) | Package-shaped fixtures for the locked `sgpm` check/test/fmt/doc/build loop |
| [`stdlib/`](stdlib/) | Source-level standard library imports and everyday helper APIs |
| [`traits/`](traits/) | Trait methods, concrete impls, and generic trait method instantiation |
| [`ffi/`](ffi/) | Sengoo calling C and C calling exported Sengoo symbols |
| [`reflection/`](reflection/) | Runtime wrapper demos for DB, Lua, proto, net, and FFI reflection paths |

## Basic Syntax

| File | Expected result |
|---|---:|
| [`01_hello.sg`](01_hello.sg) | `42` |
| [`02_arithmetic.sg`](02_arithmetic.sg) | `30` |
| [`03_variables.sg`](03_variables.sg) | `300` |
| [`04_array.sg`](04_array.sg) | `20` |
| [`05_loop.sg`](05_loop.sg) | `15` |
| [`06_lambda.sg`](06_lambda.sg) | `15` |
| [`07_if.sg`](07_if.sg) | `1` |
| [`08_struct.sg`](08_struct.sg) | `3` |
| [`09_method_call.sg`](09_method_call.sg) | `43` |

## Smoke Coverage

`cargo test -p sgc examples_smoke_` compiles and runs the curated examples.
The FFI smoke tests skip gracefully when a compatible C toolchain is not
available.
