# Sengoo Runtime FFI + Lua Bridge MVP

This document describes the runtime C FFI bridge and Lua bridge MVP in branch `feat/db-ffi-a`.

## Scope

- C library path:
  - open
  - call symbol (`i64` return, arity 0..=4)
  - close
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
- `-2099`: internal error

## Unsafe boundary notes

- `sengoo_ffi_c_call_i64` performs native symbol lookup and raw pointer casting.
- Native function ABI is treated as `extern "C"` and currently supports i64 return with max 4 i64 args.
- Caller must ensure symbol signature matches the requested call shape.
- `sengoo_lua54_*` uses runtime-loaded Lua 5.4 symbols (`luaL_loadstring`, `lua_pcallk/lua_pcall`, etc.).
- If Lua 5.4 dynamic library is unavailable, `sengoo_lua54_open` reports load diagnostics through the dedicated Lua54 error channel.
