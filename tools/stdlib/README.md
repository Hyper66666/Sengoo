# Sengoo Standard Library Sources

The MVP standard library is split into small source modules so compiler tests,
runtime wrappers, and examples can depend on only the surfaces they need.

- `option.sg`: generic `Option<T>` plus i64 constructors and unwrap/map helpers.
- `result.sg`: generic `Result<T, E>` plus i64 constructors and map/projection helpers.
- `collections.sg`: runtime-backed `Vec<T>`, `HashMap<K, V>`, iterators, and i64-specialized collection mutators.
- `string.sg`: Sengoo-side wrappers over built-in string lowering: `str_len`, `str_eq`, and `str_concat`.
- `math.sg`: pure-Sengoo integer helpers: `abs_i64`, `min_i64`, `max_i64`, and `pow_i64`.
- `error.sg`: pure-Sengoo assertion helpers. `assert_eq` variants for floating-point and strings are deferred until matching panic/reporting helpers exist.
- `runtime.c`: C runtime support used by stdlib/runtime smoke paths.

## Reflection Wrappers

The reflection wrapper modules are thin Sengoo-side surfaces over existing
runtime drivers. They do not add or change runtime symbols.

- `db.sg`: wraps `runtime/src/reflect/runtime_db.rs`. Lifecycle: `db_open_raw` returns `Db`, then call `Db.close`; query results use `DbResult.close`. Example: `examples/reflection/db_open_query.sg`.
- `ffi.sg`: wraps `runtime/src/reflect/runtime_ffi.rs`. Lifecycle: `ffi_open_raw` returns `CLib`, callbacks use `CallbackToken.unbind`, buffers use `Buffer.free`. Example: `examples/reflection/ffi_load_call.sg`.
- `lua54.sg`: wraps `runtime/src/reflect/runtime_lua54.rs`. Lifecycle: `lua54_open_raw` returns `Lua54`, then call `Lua54.close`. Native Lua 5.4 availability is runtime/feature-gated, so examples may exercise the diagnostic path when Lua is unavailable. Example: `examples/reflection/lua54_eval.sg`.
- `proto.sg`: wraps `runtime/src/reflect/runtime_proto.rs` for the currently implemented `ProtoUserEvent` encode/decode shape. Caller-provided buffers remain borrowed. Example: `examples/reflection/proto_encode_decode.sg`.
- `net.sg`: wraps the public `runtime/src/net.rs` TCP/UDP/HTTP/WS surface and `runtime/src/reflect/runtime_net_bench.rs`. Lifecycle: every nonzero handle is closed by its matching `close` method/function. Example: `examples/reflection/net_tcp_echo.sg`.

Current source-level limitation: Sengoo FFI does not yet expose safe `&str` or
byte-slice parameter types. Wrapper methods therefore use raw `i64` pointer
values for C string and buffer pointers until typed FFI pointer/string support
lands.
