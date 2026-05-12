# runtime_lua54 ABI Note

Driver: `runtime/src/reflect/runtime_lua54.rs`

Feature gate:

- Native Lua 5.4 loading depends on the runtime Lua54 dynamic-library path being available at runtime.
- Tests and examples should treat missing Lua 5.4 as a skip/diagnostic path, not a hard wrapper failure.

Status codes:

- `0`: success
- `-2401`: invalid argument
- `-2402`: invalid handle
- `-2403`: library load failure
- `-2404`: symbol load failure
- `-2405`: compile error
- `-2406`: runtime error
- `-2407`: type error
- `-2499`: internal error

Extern symbols:

- `sengoo_lua54_last_error_code() -> i32`
- `sengoo_lua54_last_error_len() -> i64`
- `sengoo_lua54_last_error_copy(buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_lua54_last_error_clear() -> i32`
- `sengoo_lua54_open(path: *const u8) -> u64`
- `sengoo_lua54_close(handle: u64) -> i32`
- `sengoo_lua54_exec(handle: u64, chunk: *const u8) -> i32`
- `sengoo_lua54_call_i64(handle: u64, func_name: *const u8, argc: usize, argv: *const i64, out_value: *mut i64) -> i32`

Ownership:

- `sengoo_lua54_open` returns an interpreter handle closed by `sengoo_lua54_close`.
- Script/function name pointers are borrowed.

Sengoo wrapper note:

The MVP wrapper exposes raw pointer parameters as `i64` and keeps the dynamic-library path explicit.
