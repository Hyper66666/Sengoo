# Sengoo Standard Library Sources

The MVP standard library is split into small source modules so compiler tests,
runtime wrappers, and examples can depend on only the surfaces they need.

- `option.sg`: generic `Option<T>`, generic constructors (`option_some`,
  `option_none_with`), i64 convenience constructors, and unwrap/map helpers.
- `result.sg`: generic `Result<T, E>`, generic constructors (`result_ok_with`,
  `result_err_with`), i64 convenience constructors, and map/projection helpers.
- `collections.sg`: runtime-backed `Vec<T>`, `HashMap<K, V>`, iterators, and i64/bool collection mutators.
- `string.sg`: Sengoo-side wrappers over built-in string lowering: `str_len`, `str_eq`, `str_ne`, empty checks, append, and repeat.
- `math.sg`: pure-Sengoo integer helpers: `abs_i64`, `min_i64`, `max_i64`, `sign_i64`, `clamp_i64`, `gcd_i64`, `lcm_i64`, and `pow_i64`.
- `error.sg`: pure-Sengoo assertion helpers for boolean, i64, string, and f64 checks.
- `db.sg`, `ffi.sg`, `lua54.sg`, `net.sg`, `proto.sg`: Sengoo-side wrappers over the runtime reflection drivers.
- `runtime.c`: C runtime support used by stdlib/runtime smoke paths.

## Source Imports

`sgc check`, `sgc build`, and `sgc run` understand source-level stdlib imports
and preload the requested module before compiling:

```sg
import std::collections;

def main() -> i64 {
    let values = vec_new_i64();
    values.push(41);
    values.get(0).unwrap_or(0) + 1
}
```

For modules that use `Option<T>` or `Result<T, E>`, `sgc` also preloads the
current source dependencies (`option.sg` and `result.sg`) automatically.
Reflection modules can declare their own source dependencies as well. `import
std::db`, `import std::lua54`, `import std::net`, and `import std::proto`
preload `ffi.sg` so managed `Buffer` helpers are available for output payloads.

## Reflection Wrappers

The reflection wrapper modules are thin Sengoo-side surfaces over existing
runtime drivers. A shared `sengoo_stdlib_str_ptr` helper bridges Sengoo `&str`
values to the existing raw-pointer driver calls.

- `db.sg`: wraps `runtime/src/reflect/runtime_db.rs`. Lifecycle: `db_open`/`db_open_raw` returns `Db`, then call `Db.close`; query results use `DbResult.close`. Error, column-name, and cell copy helpers accept managed `Buffer` handles. Example: `examples/reflection/db_open_query.sg`.
- `ffi.sg`: wraps `runtime/src/reflect/runtime_ffi.rs`. Lifecycle: `ffi_open`/`ffi_open_raw` returns `CLib`, callbacks use `CallbackToken.unbind`, buffers use `Buffer.free`. Error copy and buffer-to-buffer copy helpers accept managed `Buffer` handles. Fixed-arity `call_i64_0` through `call_i64_4` helpers cover common C calls without raw argument/result pointers; object constructors and methods have matching helpers. Example: `examples/reflection/ffi_load_call.sg`.
- `lua54.sg`: wraps `runtime/src/reflect/runtime_lua54.rs`. Lifecycle: `lua54_open`/`lua54_open_raw` returns `Lua54`, then call `Lua54.close`. Error copy helpers accept managed `Buffer` handles, and `call_i64_0` through `call_i64_4` cover common calls without raw pointer slots. Native Lua 5.4 availability is runtime/feature-gated, so examples may exercise the diagnostic path when Lua is unavailable. Example: `examples/reflection/lua54_eval.sg`.
- `proto.sg`: wraps `runtime/src/reflect/runtime_proto.rs` for the currently implemented `ProtoUserEvent` encode/decode shape. `proto_user_event` accepts a normal `&str` name, `proto_user_event_encode` writes into a managed `Buffer`, and `proto_user_event_decode(buffer, input_len)` returns a managed `ProtoDecodedUserEvent` handle with field readers plus `close`. Raw decode/output helpers remain available for explicit pointer handoff. Example: `examples/reflection/proto_encode_decode.sg`.
- `net.sg`: wraps the public `runtime/src/net.rs` TCP/UDP/HTTP client/server/WS surface and `runtime/src/reflect/runtime_net_bench.rs`. Safe `&str` helpers cover hosts, URLs, text payloads, server routes, and required-header middleware; managed `Buffer` helpers cover receive/body/error/bench output; `_raw` helpers remain for explicit pointer/buffer handoff. Lifecycle: every nonzero handle is closed by its matching `close` method/function. Examples: `examples/reflection/net_tcp_echo.sg`, `examples/reflection/net_http_server.sg`.

Current source-level limitation: Sengoo FFI now accepts immutable `&str` C-string
parameters, and the reflection wrappers expose normal string helpers for common
paths. Managed `Buffer` handles cover FFI byte payloads, DB/Lua/FFI diagnostics,
DB result copies, protobuf encode output, and network receive/body/error/bench
output. Protobuf decoded fields are available through runtime-owned handles,
and common fixed-arity FFI/Lua calls no longer require raw pointer slots.
Dynamic-arity FFI/Lua calls still use raw `i64` pointer values until typed
slice/buffer/out-parameter support lands.
