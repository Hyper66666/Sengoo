# Sengoo Runtime Database MVP

This document describes the runtime database MVP added in branch `feat/db-ffi-a`.

## Scope

- Connection lifecycle: `open / ping / close`
- Minimal SQL path:
  - `exec`: `CREATE TABLE / INSERT INTO / DELETE FROM`
  - `query`: `SELECT ... FROM ... [WHERE ...]`
- Parameter passing:
  - Named parameter (`:name`) with JSON object input
  - Positional parameter (`?`) with JSON array input
- Result handle and cell access APIs
- Structured error code + error message channel

## C ABI

### Error channel

- `i32 sengoo_db_last_error_code()`
- `i64 sengoo_db_last_error_len()`
- `i64 sengoo_db_last_error_copy(u8* buffer, usize capacity)`
- `i32 sengoo_db_last_error_clear()`

### Connection lifecycle

- `u64 sengoo_db_open(const u8* conn_str)`
- `i32 sengoo_db_ping(u64 handle)`
- `i32 sengoo_db_close(u64 handle)`

### Statement execution

- `i64 sengoo_db_exec(u64 handle, const u8* sql, const u8* params_json)`
- `u64 sengoo_db_query(u64 handle, const u8* sql, const u8* params_json)`

### Result access

- `i32 sengoo_db_result_close(u64 result_handle)`
- `i64 sengoo_db_result_row_count(u64 result_handle)`
- `i64 sengoo_db_result_col_count(u64 result_handle)`
- `i64 sengoo_db_result_col_name_len(u64 result_handle, usize col_idx)`
- `i64 sengoo_db_result_col_name_copy(u64 result_handle, usize col_idx, u8* buffer, usize capacity)`
- `i64 sengoo_db_result_cell_len(u64 result_handle, usize row_idx, usize col_idx)`
- `i64 sengoo_db_result_cell_copy(u64 result_handle, usize row_idx, usize col_idx, u8* buffer, usize capacity)`

## Status codes

- `0`: success
- `< 0`: failure

Key DB error codes:

- `-1001`: invalid argument
- `-1002`: invalid handle
- `-1003`: parse error
- `-1004`: table/column not found
- `-1005`: execution error
- `-1099`: internal error

## Notes

- This MVP is an in-process memory table engine to unblock runtime integration.
- The API shape is stable enough for later external-driver replacement.

