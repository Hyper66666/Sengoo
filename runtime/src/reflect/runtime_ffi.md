# runtime_ffi ABI Note

Driver: `runtime/src/reflect/runtime_ffi.rs`

Status codes:

- `0`: success
- `-2001`: invalid argument
- `-2002`: invalid handle
- `-2003`: symbol not found
- `-2004`: call failed
- `-2005`: parse error
- `-2006`: buffer error
- `-2099`: internal error

Extern symbols:

- `sengoo_ffi_last_error_code() -> i32`
- `sengoo_ffi_last_error_len() -> i64`
- `sengoo_ffi_last_error_copy(buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_ffi_last_error_clear() -> i32`
- `sengoo_ffi_c_open(path: *const u8) -> u64`
- `sengoo_ffi_c_close(handle: u64) -> i32`
- `sengoo_ffi_c_call_i64(handle: u64, symbol: *const u8, argc: usize, argv: *const i64, out_value: *mut i64) -> i32`
- `sengoo_ffi_object_create(lib_handle: u64, constructor_symbol: *const u8, argc: usize, argv: *const i64, destructor_symbol: *const u8) -> u64`
- `sengoo_ffi_object_raw_ptr(object_handle: u64) -> i64`
- `sengoo_ffi_object_call_i64(object_handle: u64, method_symbol: *const u8, argc: usize, argv: *const i64, out_value: *mut i64) -> i32`
- `sengoo_ffi_object_destroy(object_handle: u64) -> i32`
- `sengoo_ffi_callback_bind_i64(lib_handle: u64, symbol: *const u8, arity: usize) -> u64`
- `sengoo_ffi_callback_dispatch_i64(callback_id: u64, a0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64`
- `sengoo_ffi_callback_unbind(callback_id: u64) -> i32`
- `sengoo_ffi_buffer_new(capacity: usize) -> u64`
- `sengoo_ffi_buffer_from_bytes(data: *const u8, len: usize) -> u64`
- `sengoo_ffi_buffer_len(buffer_handle: u64) -> i64`
- `sengoo_ffi_buffer_ptr(buffer_handle: u64) -> i64`
- `sengoo_ffi_buffer_copy_out(buffer_handle: u64, out_buffer: *mut u8, out_capacity: usize) -> i64`
- `sengoo_ffi_buffer_copy_in(buffer_handle: u64, src_ptr: *const u8, src_len: usize) -> i32`
- `sengoo_ffi_buffer_free(buffer_handle: u64) -> i32`

Ownership:

- `sengoo_ffi_c_open` returns a library handle closed by `sengoo_ffi_c_close`.
- Object handles are destroyed by `sengoo_ffi_object_destroy`.
- Callback handles are released by `sengoo_ffi_callback_unbind`.
- Buffer handles are released by `sengoo_ffi_buffer_free`.

Sengoo wrapper note:

The MVP wrapper intentionally exposes raw pointer parameters as `i64` until source-level pointer/string FFI types are stabilized.
