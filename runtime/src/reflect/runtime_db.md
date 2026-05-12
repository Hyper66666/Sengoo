# runtime_db ABI Note

Driver: `runtime/src/reflect/runtime_db.rs`

Status codes:

- `0`: success
- `-1001`: invalid argument
- `-1002`: invalid handle
- `-1003`: parse error
- `-1004`: not found
- `-1005`: execution error
- `-1099`: internal error

Extern symbols:

- `sengoo_db_last_error_code() -> i32`
- `sengoo_db_last_error_len() -> i64`
- `sengoo_db_last_error_copy(buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_db_last_error_clear() -> i32`
- `sengoo_db_open(conn_str: *const u8) -> u64`
- `sengoo_db_close(handle: u64) -> i32`
- `sengoo_db_ping(handle: u64) -> i32`
- `sengoo_db_exec(handle: u64, sql: *const u8, params_json: *const u8) -> i64`
- `sengoo_db_query(handle: u64, sql: *const u8, params_json: *const u8) -> u64`
- `sengoo_db_result_close(result_handle: u64) -> i32`
- `sengoo_db_result_row_count(result_handle: u64) -> i64`
- `sengoo_db_result_col_count(result_handle: u64) -> i64`
- `sengoo_db_result_col_name_len(result_handle: u64, col_idx: usize) -> i64`
- `sengoo_db_result_col_name_copy(result_handle: u64, col_idx: usize, buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_db_result_cell_len(result_handle: u64, row_idx: usize, col_idx: usize) -> i64`
- `sengoo_db_result_cell_copy(result_handle: u64, row_idx: usize, col_idx: usize, buffer: *mut u8, capacity: usize) -> i64`

Ownership:

- `sengoo_db_open` returns an opaque connection handle; close it with `sengoo_db_close`.
- `sengoo_db_query` returns an opaque result handle; close it with `sengoo_db_result_close`.
- String and buffer pointers are borrowed by the runtime.

Sengoo wrapper note:

The current source-level wrapper uses raw `i64` pointer values for borrowed C string and buffer pointers because Sengoo FFI currently rejects reference types such as `&str` as not FFI-safe.
