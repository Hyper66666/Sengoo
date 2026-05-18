## 1. Driver ABI Audit

- [x] 1.1 Read `runtime/src/reflect/runtime_db.rs`; capture extern symbols, signatures, ownership, status codes in `runtime/src/reflect/runtime_db.md`.
- [x] 1.2 Same for `runtime_ffi.rs` → `runtime_ffi.md`.
- [x] 1.3 Same for `runtime_lua54.rs` → `runtime_lua54.md`. Note Cargo feature gate.
- [x] 1.4 Same for `runtime_proto.rs` → `runtime_proto.md`.
- [x] 1.5 Same for `runtime_net_bench.rs` and the publicly reachable surface of `runtime/src/net.rs` → `runtime_net.md`.
- [x] 1.6 Cross-check each ABI note against the corresponding test module in the runtime crate to confirm signatures.

## 2. tools/stdlib/db.sg

- [x] 2.1 Create `tools/stdlib/db.sg` with `extern "C" { ... }` block matching the audit.
- [x] 2.2 Define `struct Db { handle: i64 }`.
- [x] 2.3 Implement methods: `open(path)`, `close(self)`, `ping(self)`, `exec(self, sql)`, `query(self, sql)`.
- [x] 2.4 Return `Result<T, i64>` for fallible operations using `tools/stdlib/result.sg`.
- [x] 2.5 Add `examples/reflection/db_open_query.sg` smoke demo.

## 3. tools/stdlib/lua54.sg

- [x] 3.1 Create `tools/stdlib/lua54.sg` with extern block.
- [x] 3.2 Define `struct Lua54 { handle: i64 }` and resource methods.
- [x] 3.3 Add `examples/reflection/lua54_eval.sg`; conditionally skip if `runtime_lua54` Cargo feature is off.
- [x] 3.4 Document the feature-gate requirement in `tools/stdlib/README.md`.

## 4. tools/stdlib/proto.sg

- [x] 4.1 Create `tools/stdlib/proto.sg` with extern block.
- [x] 4.2 Surface only the already-implemented message types; do not invent new schemas in this change.
- [x] 4.3 Add `examples/reflection/proto_encode_decode.sg` round-trip demo.

## 5. tools/stdlib/net.sg

- [x] 5.1 Create `tools/stdlib/net.sg` with extern block covering NetRuntime-backed TCP/UDP/HTTP/WS surface.
- [x] 5.2 Define `struct TcpStream`, `struct UdpSocket`, `struct HttpClient`, `struct WsClient` each holding the opaque handle.
- [x] 5.3 Add `examples/reflection/net_tcp_echo.sg` demo using a localhost echo server fixture.

## 6. tools/stdlib/ffi.sg

- [x] 6.1 Create `tools/stdlib/ffi.sg` with extern block (largest surface).
- [x] 6.2 Define `struct CLib`, `struct CppObject`, `struct CallbackToken`, `struct Buffer` wrappers.
- [x] 6.3 Add `examples/reflection/ffi_load_call.sg` demo loading a small native library and calling one symbol.

## 7. Examples Smoke Tests

- [x] 7.1 Add `cargo test -p sgc examples_smoke_reflection_*` test cases that compile each example with the matching stdlib wrapper modules. Full `sgc run` is deferred until the Rust reflection runtime symbols are linkable from the source-level examples.
- [x] 7.2 Keep lua54 as a compile smoke; runtime availability remains feature/dynamic-library gated.

## 8. Documentation

- [x] 8.1 Extend `tools/stdlib/README.md` with a "Reflection Wrappers" section listing each wrapper, its driver source, its example, and its resource lifecycle.
- [x] 8.2 Update README.md / README.zh-CN.md "Runtime Integration Stack" bullets to point at the new wrappers as the recommended user surface.

## 9. Verification

- [x] 9.1 `cargo test -p sengoo-runtime --lib` stays at 42/42.
- [x] 9.2 `cargo test -p sgc` runs all examples_smoke_reflection_* cases green (lua54 skipped if feature off).
- [x] 9.3 No new symbols exported from `runtime/src/reflect/`.
- [x] 9.4 No changes to `runtime/src/net.rs` extern C ABI.
