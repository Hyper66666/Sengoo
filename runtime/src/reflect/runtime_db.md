# runtime_db ABI Note

Driver: `runtime/src/reflect/runtime_db/` (directory module since 2026-05-20)

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

The Sengoo wrapper exposes `&str` helpers for connection strings, SQL, and
params JSON. Error, column-name, and cell copy helpers accept managed `Buffer`
handles, with `_raw` variants left available for explicit pointer/capacity
handoff.

Module layout (2026-05-20, post-split):

- `runtime_db/mod.rs` (431 LoC): the 16 `#[no_mangle] pub extern "C" fn sengoo_db_*` symbols listed above, plus the integration test module. No business logic.
- `runtime_db/status.rs` (7 LoC): the 7 `pub const SENGOO_DB_*` status codes listed above. Re-exported at the module root via `pub use status::*;` so consumer paths are unchanged.
- `runtime_db/state.rs` (74 LoC): private storage types (`DbConnection`, `DbTable`, `DbQueryResult`, `DbErrorState`), four `OnceLock` statics holding the global tables, plus `pub(super)` helpers `db_connections`, `db_results`, `db_last_error`, `next_handle`, `clear_error`, `set_error`. All scoped to the directory module — nothing leaks to outer `reflect` or to other crates.
- `runtime_db/ffi_utils.rs` (56 LoC): C-pointer helpers `parse_c_string`, `parse_optional_json`, `copy_bytes_to_buffer`.
- `runtime_db/sql.rs` (107 LoC): SQL fragment parsers `normalize_identifier`, `find_keyword_case_insensitive`, `parse_literal`, `resolve_param_token`, `parse_where_clause`, plus the `value_to_string` formatter used by the result-cell extern C exports.
- `runtime_db/exec.rs` (327 LoC): statement execution helpers `exec_create_table`, `resolve_insert_columns`, `exec_insert`, `build_select_result`, `run_select`, `exec_delete`, `execute_statement`.

The split preserves byte-for-byte the extern C symbol surface, the three integration tests, and every emitted error message string. See `openspec/changes/archive/2026-05-XX-large-file-splits-runtime-db/` for the full change record and the reusable Split SOP captured in `tasks.md` §9.
