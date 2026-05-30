mod exec;
mod ffi_utils;
mod sql;
mod state;
mod status;
use exec::*;
use ffi_utils::*;
use sql::*;
use state::*;
pub use status::*;

#[no_mangle]
pub extern "C" fn sengoo_db_last_error_code() -> i32 {
    db_last_error()
        .lock()
        .map(|state| state.code)
        .unwrap_or(SENGOO_DB_ERR_INTERNAL)
}

#[no_mangle]
pub extern "C" fn sengoo_db_last_error_len() -> i64 {
    db_last_error()
        .lock()
        .map(|state| state.message.len() as i64)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn sengoo_db_last_error_copy(buffer: *mut u8, capacity: usize) -> i64 {
    let message = db_last_error()
        .lock()
        .map(|state| state.message.clone())
        .unwrap_or_default();
    copy_bytes_to_buffer(message.as_bytes(), buffer, capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_db_last_error_clear() -> i32 {
    clear_error();
    SENGOO_DB_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_db_open(conn_str: *const u8) -> u64 {
    clear_error();
    let conn_str = match parse_c_string(conn_str) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    if conn_str.trim().is_empty() {
        set_error(SENGOO_DB_ERR_INVALID_ARGUMENT, "connection string is empty");
        return 0;
    }
    let handle = next_handle();
    let mut table = match db_connections().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_DB_ERR_INTERNAL, "db connection table poisoned");
            return 0;
        }
    };
    table.insert(handle, DbConnection::default());
    handle
}

#[no_mangle]
pub extern "C" fn sengoo_db_close(handle: u64) -> i32 {
    clear_error();
    let mut table = match db_connections().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db connection table poisoned"),
    };
    if table.remove(&handle).is_some() {
        SENGOO_DB_STATUS_OK
    } else {
        set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db handle {handle} not found"),
        )
    }
}

#[no_mangle]
pub extern "C" fn sengoo_db_ping(handle: u64) -> i32 {
    clear_error();
    let table = match db_connections().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db connection table poisoned"),
    };
    if table.contains_key(&handle) {
        SENGOO_DB_STATUS_OK
    } else {
        set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db handle {handle} not found"),
        )
    }
}

#[no_mangle]
pub extern "C" fn sengoo_db_exec(handle: u64, sql: *const u8, params_json: *const u8) -> i64 {
    clear_error();
    let sql = match parse_c_string(sql) {
        Ok(value) => value,
        Err(code) => return code as i64,
    };
    let params = match parse_optional_json(params_json) {
        Ok(value) => value,
        Err(code) => return code as i64,
    };

    let mut table = match db_connections().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db connection table poisoned") as i64,
    };
    let Some(conn) = table.get_mut(&handle) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db handle {handle} not found"),
        ) as i64;
    };

    match execute_statement(conn, &sql, &params) {
        Ok(affected) => affected as i64,
        Err(code) => code as i64,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_db_query(handle: u64, sql: *const u8, params_json: *const u8) -> u64 {
    clear_error();
    let sql = match parse_c_string(sql) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let params = match parse_optional_json(params_json) {
        Ok(value) => value,
        Err(_) => return 0,
    };

    let table = match db_connections().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_DB_ERR_INTERNAL, "db connection table poisoned");
            return 0;
        }
    };
    let Some(conn) = table.get(&handle) else {
        set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db handle {handle} not found"),
        );
        return 0;
    };
    let normalized = sql.trim();
    if !normalized.to_ascii_uppercase().starts_with("SELECT") {
        set_error(SENGOO_DB_ERR_PARSE, "query only supports SELECT statements");
        return 0;
    }
    let result = match run_select(conn, normalized, &params) {
        Ok(result) => result,
        Err(_) => return 0,
    };
    drop(table);

    let result_handle = next_handle();
    let mut results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned");
            return 0;
        }
    };
    results.insert(result_handle, result);
    result_handle
}

#[no_mangle]
pub extern "C" fn sengoo_db_result_close(result_handle: u64) -> i32 {
    clear_error();
    let mut results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned"),
    };
    if results.remove(&result_handle).is_some() {
        SENGOO_DB_STATUS_OK
    } else {
        set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db result handle {result_handle} not found"),
        )
    }
}

#[no_mangle]
pub extern "C" fn sengoo_db_result_row_count(result_handle: u64) -> i64 {
    clear_error();
    let results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned") as i64,
    };
    match results.get(&result_handle) {
        Some(result) => result.rows.len() as i64,
        None => set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db result handle {result_handle} not found"),
        ) as i64,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_db_result_col_count(result_handle: u64) -> i64 {
    clear_error();
    let results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned") as i64,
    };
    match results.get(&result_handle) {
        Some(result) => result.columns.len() as i64,
        None => set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db result handle {result_handle} not found"),
        ) as i64,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_db_result_col_name_len(result_handle: u64, col_idx: usize) -> i64 {
    clear_error();
    let results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned") as i64,
    };
    let Some(result) = results.get(&result_handle) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db result handle {result_handle} not found"),
        ) as i64;
    };
    match result.columns.get(col_idx) {
        Some(name) => name.len() as i64,
        None => set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            format!("column index {col_idx} out of bounds"),
        ) as i64,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_db_result_col_name_copy(
    result_handle: u64,
    col_idx: usize,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    clear_error();
    let results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned") as i64,
    };
    let Some(result) = results.get(&result_handle) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db result handle {result_handle} not found"),
        ) as i64;
    };
    let Some(name) = result.columns.get(col_idx) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            format!("column index {col_idx} out of bounds"),
        ) as i64;
    };
    copy_bytes_to_buffer(name.as_bytes(), buffer, capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_db_result_cell_len(
    result_handle: u64,
    row_idx: usize,
    col_idx: usize,
) -> i64 {
    clear_error();
    let results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned") as i64,
    };
    let Some(result) = results.get(&result_handle) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db result handle {result_handle} not found"),
        ) as i64;
    };
    let Some(row) = result.rows.get(row_idx) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            format!("row index {row_idx} out of bounds"),
        ) as i64;
    };
    let Some(value) = row.get(col_idx) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            format!("column index {col_idx} out of bounds"),
        ) as i64;
    };
    value_to_string(value).len() as i64
}

#[no_mangle]
pub extern "C" fn sengoo_db_result_cell_copy(
    result_handle: u64,
    row_idx: usize,
    col_idx: usize,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    clear_error();
    let results = match db_results().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_DB_ERR_INTERNAL, "db result table poisoned") as i64,
    };
    let Some(result) = results.get(&result_handle) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_HANDLE,
            format!("db result handle {result_handle} not found"),
        ) as i64;
    };
    let Some(row) = result.rows.get(row_idx) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            format!("row index {row_idx} out of bounds"),
        ) as i64;
    };
    let Some(value) = row.get(col_idx) else {
        return set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            format!("column index {col_idx} out of bounds"),
        ) as i64;
    };
    let text = value_to_string(value);
    copy_bytes_to_buffer(text.as_bytes(), buffer, capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test lock poisoned")
    }

    fn c_str(s: &str) -> Vec<u8> {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    fn read_cell(result: u64, row: usize, col: usize) -> String {
        let len = sengoo_db_result_cell_len(result, row, col);
        assert!(len >= 0);
        let mut buf = vec![0u8; len as usize];
        let copied = sengoo_db_result_cell_copy(result, row, col, buf.as_mut_ptr(), buf.len());
        assert!(copied >= 0);
        String::from_utf8_lossy(&buf[..copied as usize]).to_string()
    }

    #[test]
    fn db_open_ping_close_and_error_mapping() {
        let _guard = test_lock();
        let conn = c_str("memory://mvp");
        let handle = sengoo_db_open(conn.as_ptr());
        assert!(handle != 0);
        assert_eq!(sengoo_db_ping(handle), SENGOO_DB_STATUS_OK);
        assert_eq!(sengoo_db_close(handle), SENGOO_DB_STATUS_OK);

        let code = sengoo_db_ping(handle);
        assert_eq!(code, SENGOO_DB_ERR_INVALID_HANDLE);
        assert_eq!(sengoo_db_last_error_code(), SENGOO_DB_ERR_INVALID_HANDLE);
        assert!(sengoo_db_last_error_len() > 0);
    }

    #[test]
    fn db_exec_query_with_params_smoke() {
        let _guard = test_lock();
        let conn = c_str("memory://users");
        let handle = sengoo_db_open(conn.as_ptr());
        assert!(handle != 0);

        let create = c_str("CREATE TABLE users (id, name)");
        assert_eq!(
            sengoo_db_exec(handle, create.as_ptr(), std::ptr::null()),
            0,
            "create table should return affected=0"
        );

        let insert = c_str("INSERT INTO users (id, name) VALUES (:id, :name)");
        let params = c_str("{\"id\":1,\"name\":\"alice\"}");
        assert_eq!(sengoo_db_exec(handle, insert.as_ptr(), params.as_ptr()), 1);

        let select = c_str("SELECT id, name FROM users WHERE id = :target");
        let query_params = c_str("{\"target\":1}");
        let result = sengoo_db_query(handle, select.as_ptr(), query_params.as_ptr());
        assert!(result != 0);
        assert_eq!(sengoo_db_result_row_count(result), 1);
        assert_eq!(sengoo_db_result_col_count(result), 2);
        assert_eq!(read_cell(result, 0, 0), "1");
        assert_eq!(read_cell(result, 0, 1), "alice");
        assert_eq!(sengoo_db_result_close(result), SENGOO_DB_STATUS_OK);
        assert_eq!(sengoo_db_close(handle), SENGOO_DB_STATUS_OK);
    }

    #[test]
    fn db_invalid_sql_returns_parse_error() {
        let _guard = test_lock();
        let conn = c_str("memory://bad-sql");
        let handle = sengoo_db_open(conn.as_ptr());
        assert!(handle != 0);

        let bad = c_str("UPSERT INTO users VALUES (1)");
        let rc = sengoo_db_exec(handle, bad.as_ptr(), std::ptr::null());
        assert_eq!(rc, SENGOO_DB_ERR_PARSE as i64);
        assert_eq!(sengoo_db_last_error_code(), SENGOO_DB_ERR_PARSE);

        assert_eq!(sengoo_db_close(handle), SENGOO_DB_STATUS_OK);
    }
}
