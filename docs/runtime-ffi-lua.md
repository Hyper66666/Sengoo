# Sengoo Runtime FFI + Lua Bridge MVP

This document describes the runtime C/C++ FFI wrapper and Lua bridge in branch `feat/db-ffi-a`.

## Scope

- C library path:
  - open
  - call symbol (`i64` return, arity 0..=4)
  - close
- C++ reuse wrapper path (via C shim):
  - object create
  - object method call
  - object destroy
  - callback bind/dispatch/unbind
  - byte buffer handle for payload/string/protobuf bridge
- Lua bridge path (lightweight subset):
  - state open/close
  - load chunk
  - exec loaded/inline chunk
  - call function (`i64` return)
- Lua 5.4 bridge PoC path (native dynamic library):
  - open/close Lua 5.4 runtime
  - exec real Lua chunk
  - call Lua global function (`i64` return)
- Structured error diagnostics (code + message)

## C ABI

### Shared FFI error channel

- `i32 sengoo_ffi_last_error_code()`
- `i64 sengoo_ffi_last_error_len()`
- `i64 sengoo_ffi_last_error_copy(u8* buffer, usize capacity)`
- `i32 sengoo_ffi_last_error_clear()`

### C library bridge

- `u64 sengoo_ffi_c_open(const u8* path)`
- `i32 sengoo_ffi_c_close(u64 handle)`
- `i32 sengoo_ffi_c_call_i64(u64 handle, const u8* symbol, usize argc, const i64* argv, i64* out_value)`

### C++ wrapper primitives (C shim friendly)

- `u64 sengoo_ffi_object_create(u64 lib_handle, const u8* constructor_symbol, usize argc, const i64* argv, const u8* destructor_symbol)`
- `i64 sengoo_ffi_object_raw_ptr(u64 object_handle)`
- `i32 sengoo_ffi_object_call_i64(u64 object_handle, const u8* method_symbol, usize argc, const i64* argv, i64* out_value)`
- `i32 sengoo_ffi_object_destroy(u64 object_handle)`

- `u64 sengoo_ffi_callback_bind_i64(u64 lib_handle, const u8* symbol, usize arity)`
- `i64 sengoo_ffi_callback_dispatch_i64(u64 callback_id, i64 a0, i64 a1, i64 a2, i64 a3, i64 a4, i64 a5)`
- `i32 sengoo_ffi_callback_unbind(u64 callback_id)`

- `u64 sengoo_ffi_buffer_new(usize capacity)`
- `u64 sengoo_ffi_buffer_from_bytes(const u8* data, usize len)`
- `i64 sengoo_ffi_buffer_len(u64 buffer_handle)`
- `i64 sengoo_ffi_buffer_ptr(u64 buffer_handle)`
- `i64 sengoo_ffi_buffer_copy_out(u64 buffer_handle, u8* out_buffer, usize out_capacity)`
- `i32 sengoo_ffi_buffer_copy_in(u64 buffer_handle, const u8* src_ptr, usize src_len)`
- `i32 sengoo_ffi_buffer_free(u64 buffer_handle)`

Special test/baseline path:

- `self://builtin`
  - `sengoo_ffi_builtin_add2(a, b) -> i64`
  - `sengoo_ffi_builtin_mul3(a, b, c) -> i64`

### Lua bridge

- `u64 sengoo_lua_open()`
- `i32 sengoo_lua_close(u64 handle)`
- `i32 sengoo_lua_load(u64 handle, const u8* chunk)`
- `i32 sengoo_lua_exec(u64 handle, const u8* chunk)`  
  If `chunk == null`, executes previously loaded chunk.
- `i32 sengoo_lua_call_i64(u64 handle, const u8* func_name, usize argc, const i64* argv, i64* out_value)`

### Lua 5.4 native bridge PoC

- `u64 sengoo_lua54_open(const u8* path)`
  - `path == null` or empty: try platform default library names
  - explicit `path`: load exactly that dynamic library
- `i32 sengoo_lua54_close(u64 handle)`
- `i32 sengoo_lua54_exec(u64 handle, const u8* chunk)`
- `i32 sengoo_lua54_call_i64(u64 handle, const u8* func_name, usize argc, const i64* argv, i64* out_value)`
- `i32 sengoo_lua54_last_error_code()`
- `i64 sengoo_lua54_last_error_len()`
- `i64 sengoo_lua54_last_error_copy(u8* buffer, usize capacity)`
- `i32 sengoo_lua54_last_error_clear()`

Lua 5.4 bridge key codes:

- `-2401`: invalid argument
- `-2402`: invalid handle
- `-2403`: library load failed
- `-2404`: symbol load failed
- `-2405`: compile error
- `-2406`: runtime error
- `-2407`: return type mismatch
- `-2499`: internal error

## FFI/Lua error codes

- `0`: success
- `< 0`: failure

Key codes:

- `-2001`: invalid argument
- `-2002`: invalid handle
- `-2003`: symbol not found
- `-2004`: call failure
- `-2005`: parse failure
- `-2006`: buffer failure
- `-2099`: internal error

## Unsafe boundary notes

- `sengoo_ffi_c_call_i64` performs native symbol lookup and raw pointer casting.
- Native function ABI is treated as `extern "C"` and currently supports i64 return with max 4 i64 args.
- Caller must ensure symbol signature matches the requested call shape.
- `sengoo_lua54_*` uses runtime-loaded Lua 5.4 symbols (`luaL_loadstring`, `lua_pcallk/lua_pcall`, etc.).
- If Lua 5.4 dynamic library is unavailable, `sengoo_lua54_open` reports load diagnostics through the dedicated Lua54 error channel.

## Practical C++ reuse pattern

1. Provide a C shim over C++ classes:
   - `Counter* counter_new(int64_t init)`
   - `int64_t counter_add(Counter* ptr, int64_t delta)`
   - `void counter_drop(Counter* ptr)`
2. Use `sengoo_ffi_object_create` with constructor and destructor symbols.
3. Use `sengoo_ffi_object_call_i64` for methods.
4. Use `sengoo_ffi_buffer_*` for byte payload handoff (protobuf/string/blob).
