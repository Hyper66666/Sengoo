use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub const SENGOO_DB_STATUS_OK: i32 = 0;
pub const SENGOO_DB_ERR_INVALID_ARGUMENT: i32 = -1001;
pub const SENGOO_DB_ERR_INVALID_HANDLE: i32 = -1002;
pub const SENGOO_DB_ERR_PARSE: i32 = -1003;
pub const SENGOO_DB_ERR_NOT_FOUND: i32 = -1004;
pub const SENGOO_DB_ERR_EXECUTION: i32 = -1005;
pub const SENGOO_DB_ERR_INTERNAL: i32 = -1099;

#[derive(Clone, Debug, Default)]
struct DbConnection {
    tables: HashMap<String, DbTable>,
}

#[derive(Clone, Debug)]
struct DbTable {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

#[derive(Clone, Debug, Default)]
struct DbQueryResult {
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
}

#[derive(Clone, Debug)]
struct DbErrorState {
    code: i32,
    message: String,
}

impl Default for DbErrorState {
    fn default() -> Self {
        Self {
            code: SENGOO_DB_STATUS_OK,
            message: String::new(),
        }
    }
}

static NEXT_DB_HANDLE: AtomicU64 = AtomicU64::new(1);
static DB_CONNECTIONS: OnceLock<Mutex<HashMap<u64, DbConnection>>> = OnceLock::new();
static DB_RESULTS: OnceLock<Mutex<HashMap<u64, DbQueryResult>>> = OnceLock::new();
static DB_LAST_ERROR: OnceLock<Mutex<DbErrorState>> = OnceLock::new();

fn db_connections() -> &'static Mutex<HashMap<u64, DbConnection>> {
    DB_CONNECTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn db_results() -> &'static Mutex<HashMap<u64, DbQueryResult>> {
    DB_RESULTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn db_last_error() -> &'static Mutex<DbErrorState> {
    DB_LAST_ERROR.get_or_init(|| Mutex::new(DbErrorState::default()))
}

fn next_handle() -> u64 {
    NEXT_DB_HANDLE.fetch_add(1, Ordering::Relaxed)
}

fn clear_error() {
    if let Ok(mut state) = db_last_error().lock() {
        state.code = SENGOO_DB_STATUS_OK;
        state.message.clear();
    }
}

fn set_error(code: i32, message: impl Into<String>) -> i32 {
    if let Ok(mut state) = db_last_error().lock() {
        state.code = code;
        state.message = message.into();
    }
    code
}

fn parse_c_string(ptr: *const u8) -> Result<String, i32> {
    if ptr.is_null() {
        return Err(set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            "null C string pointer",
        ));
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 512 * 1024 {
                return Err(set_error(
                    SENGOO_DB_ERR_INVALID_ARGUMENT,
                    "C string too long",
                ));
            }
        }
        let bytes = std::slice::from_raw_parts(ptr, len);
        std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| set_error(SENGOO_DB_ERR_INVALID_ARGUMENT, "invalid UTF-8 string"))
    }
}

fn parse_optional_json(ptr: *const u8) -> Result<Value, i32> {
    if ptr.is_null() {
        return Ok(Value::Null);
    }
    let raw = parse_c_string(ptr)?;
    if raw.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&raw).map_err(|err| {
        set_error(
            SENGOO_DB_ERR_INVALID_ARGUMENT,
            format!("invalid params JSON: {err}"),
        )
    })
}

fn copy_bytes_to_buffer(bytes: &[u8], buffer: *mut u8, capacity: usize) -> i64 {
    if buffer.is_null() {
        return set_error(SENGOO_DB_ERR_INVALID_ARGUMENT, "null output buffer") as i64;
    }
    let copy_len = bytes.len().min(capacity);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, copy_len);
    }
    copy_len as i64
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

fn find_keyword_case_insensitive(haystack: &str, keyword: &str) -> Option<usize> {
    haystack
        .to_ascii_uppercase()
        .find(&keyword.to_ascii_uppercase())
}

fn parse_literal(raw: &str) -> Value {
    let trimmed = raw.trim().trim_end_matches(';').trim();
    if trimmed.eq_ignore_ascii_case("null") {
        return Value::Null;
    }
    if trimmed.eq_ignore_ascii_case("true") {
        return Value::Bool(true);
    }
    if trimmed.eq_ignore_ascii_case("false") {
        return Value::Bool(false);
    }
    if (trimmed.starts_with('\"') && trimmed.ends_with('\"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        return Value::String(trimmed[1..trimmed.len() - 1].to_string());
    }
    if let Ok(v) = trimmed.parse::<i64>() {
        return Value::from(v);
    }
    if let Ok(v) = trimmed.parse::<f64>() {
        return serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null);
    }
    Value::String(trimmed.to_string())
}

fn resolve_param_token(token: &str, params: &Value) -> Result<Value, i32> {
    let token = token.trim().trim_end_matches(';').trim();
    if let Some(key) = token.strip_prefix(':') {
        let key = key.trim();
        let Some(obj) = params.as_object() else {
            return Err(set_error(
                SENGOO_DB_ERR_INVALID_ARGUMENT,
                "named SQL parameter requires object JSON",
            ));
        };
        obj.get(key).cloned().ok_or_else(|| {
            set_error(
                SENGOO_DB_ERR_INVALID_ARGUMENT,
                format!("missing SQL parameter :{key}"),
            )
        })
    } else if token == "?" {
        let Some(arr) = params.as_array() else {
            return Err(set_error(
                SENGOO_DB_ERR_INVALID_ARGUMENT,
                "positional SQL parameter requires array JSON",
            ));
        };
        arr.first().cloned().ok_or_else(|| {
            set_error(
                SENGOO_DB_ERR_INVALID_ARGUMENT,
                "missing positional SQL parameter",
            )
        })
    } else {
        Ok(parse_literal(token))
    }
}

fn parse_where_clause(where_clause: &str, params: &Value) -> Result<(String, Value), i32> {
    let Some((lhs, rhs)) = where_clause.split_once('=') else {
        return Err(set_error(
            SENGOO_DB_ERR_PARSE,
            "WHERE clause must contain '='",
        ));
    };
    let column = normalize_identifier(lhs);
    if column.is_empty() {
        return Err(set_error(
            SENGOO_DB_ERR_PARSE,
            "WHERE clause column is empty",
        ));
    }
    let expected = resolve_param_token(rhs, params)?;
    Ok((column, expected))
}

fn exec_create_table(conn: &mut DbConnection, sql: &str) -> Result<usize, i32> {
    let prefix = "CREATE TABLE";
    let rest = sql[prefix.len()..].trim();
    let Some(open_idx) = rest.find('(') else {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "CREATE TABLE missing '('"));
    };
    let Some(close_idx) = rest.rfind(')') else {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "CREATE TABLE missing ')'"));
    };
    if close_idx <= open_idx {
        return Err(set_error(
            SENGOO_DB_ERR_PARSE,
            "CREATE TABLE invalid column section",
        ));
    }
    let table_name = normalize_identifier(&rest[..open_idx]);
    if table_name.is_empty() {
        return Err(set_error(
            SENGOO_DB_ERR_PARSE,
            "CREATE TABLE missing table name",
        ));
    }
    if conn.tables.contains_key(&table_name) {
        return Err(set_error(
            SENGOO_DB_ERR_EXECUTION,
            format!("table '{table_name}' already exists"),
        ));
    }
    let columns_raw = &rest[open_idx + 1..close_idx];
    let mut columns = Vec::new();
    for chunk in columns_raw.split(',') {
        let ident = normalize_identifier(chunk.split_whitespace().next().unwrap_or_default());
        if !ident.is_empty() {
            columns.push(ident);
        }
    }
    if columns.is_empty() {
        return Err(set_error(
            SENGOO_DB_ERR_PARSE,
            "CREATE TABLE requires at least one column",
        ));
    }
    conn.tables.insert(
        table_name,
        DbTable {
            columns,
            rows: Vec::new(),
        },
    );
    Ok(0)
}

fn resolve_insert_columns(table: &DbTable, sql_left: &str) -> Result<Vec<String>, i32> {
    let left = sql_left.trim();
    if let Some(open_idx) = left.find('(') {
        let Some(close_idx) = left.rfind(')') else {
            return Err(set_error(
                SENGOO_DB_ERR_PARSE,
                "INSERT column section missing ')'",
            ));
        };
        if close_idx <= open_idx {
            return Err(set_error(
                SENGOO_DB_ERR_PARSE,
                "INSERT invalid column section",
            ));
        }
        let cols_raw = &left[open_idx + 1..close_idx];
        let mut cols = Vec::new();
        for col in cols_raw.split(',') {
            let ident = normalize_identifier(col);
            if !ident.is_empty() {
                cols.push(ident);
            }
        }
        if cols.is_empty() {
            return Err(set_error(
                SENGOO_DB_ERR_PARSE,
                "INSERT column list is empty",
            ));
        }
        return Ok(cols);
    }
    Ok(table.columns.clone())
}

fn exec_insert(conn: &mut DbConnection, sql: &str, params: &Value) -> Result<usize, i32> {
    let prefix = "INSERT INTO";
    let rest = sql[prefix.len()..].trim();
    let Some(values_pos) = find_keyword_case_insensitive(rest, " VALUES") else {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "INSERT must contain VALUES"));
    };
    let left = rest[..values_pos].trim();

    let table_name = if let Some(open_idx) = left.find('(') {
        normalize_identifier(&left[..open_idx])
    } else {
        normalize_identifier(left)
    };
    if table_name.is_empty() {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "INSERT missing table name"));
    }

    let Some(table) = conn.tables.get_mut(&table_name) else {
        return Err(set_error(
            SENGOO_DB_ERR_NOT_FOUND,
            format!("table '{table_name}' not found"),
        ));
    };

    let insert_columns = resolve_insert_columns(table, left)?;
    let mut row = vec![Value::Null; table.columns.len()];

    match params {
        Value::Object(map) => {
            for col in &insert_columns {
                let Some(col_idx) = table.columns.iter().position(|name| name == col) else {
                    return Err(set_error(
                        SENGOO_DB_ERR_NOT_FOUND,
                        format!("column '{col}' not found"),
                    ));
                };
                if let Some(value) = map.get(col) {
                    row[col_idx] = value.clone();
                }
            }
        }
        Value::Array(values) => {
            if values.len() < insert_columns.len() {
                return Err(set_error(
                    SENGOO_DB_ERR_INVALID_ARGUMENT,
                    "insert array params shorter than column list",
                ));
            }
            for (idx, col) in insert_columns.iter().enumerate() {
                let Some(col_idx) = table.columns.iter().position(|name| name == col) else {
                    return Err(set_error(
                        SENGOO_DB_ERR_NOT_FOUND,
                        format!("column '{col}' not found"),
                    ));
                };
                row[col_idx] = values[idx].clone();
            }
        }
        Value::Null => {
            return Err(set_error(
                SENGOO_DB_ERR_INVALID_ARGUMENT,
                "INSERT requires JSON params object or array",
            ));
        }
        _ => {
            return Err(set_error(
                SENGOO_DB_ERR_INVALID_ARGUMENT,
                "INSERT params must be object or array",
            ));
        }
    }

    table.rows.push(row);
    Ok(1)
}

fn build_select_result(
    table: &DbTable,
    selected_columns: &[String],
    where_filter: Option<(String, Value)>,
) -> Result<DbQueryResult, i32> {
    let mut projection = Vec::new();
    for col in selected_columns {
        let Some(idx) = table.columns.iter().position(|c| c == col) else {
            return Err(set_error(
                SENGOO_DB_ERR_NOT_FOUND,
                format!("column '{col}' not found"),
            ));
        };
        projection.push(idx);
    }

    let filter = if let Some((col, value)) = where_filter {
        let Some(col_idx) = table.columns.iter().position(|c| c == &col) else {
            return Err(set_error(
                SENGOO_DB_ERR_NOT_FOUND,
                format!("column '{col}' not found"),
            ));
        };
        Some((col_idx, value))
    } else {
        None
    };

    let mut rows = Vec::new();
    for row in &table.rows {
        if let Some((idx, expected)) = &filter {
            if row.get(*idx) != Some(expected) {
                continue;
            }
        }
        let mut projected = Vec::with_capacity(projection.len());
        for idx in &projection {
            projected.push(row[*idx].clone());
        }
        rows.push(projected);
    }

    Ok(DbQueryResult {
        columns: selected_columns.to_vec(),
        rows,
    })
}

fn run_select(conn: &DbConnection, sql: &str, params: &Value) -> Result<DbQueryResult, i32> {
    let prefix = "SELECT";
    let rest = sql[prefix.len()..].trim();
    let Some(from_pos) = find_keyword_case_insensitive(rest, " FROM ") else {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "SELECT must contain FROM"));
    };
    let cols_part = rest[..from_pos].trim();
    let after_from = rest[from_pos + " FROM ".len()..].trim();

    let (table_name_raw, where_part) =
        if let Some(where_pos) = find_keyword_case_insensitive(after_from, " WHERE ") {
            (
                after_from[..where_pos].trim(),
                Some(after_from[where_pos + " WHERE ".len()..].trim()),
            )
        } else {
            (after_from.trim_end_matches(';').trim(), None)
        };
    let table_name = normalize_identifier(table_name_raw);
    if table_name.is_empty() {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "SELECT missing table name"));
    }
    let Some(table) = conn.tables.get(&table_name) else {
        return Err(set_error(
            SENGOO_DB_ERR_NOT_FOUND,
            format!("table '{table_name}' not found"),
        ));
    };

    let selected_columns = if cols_part == "*" {
        table.columns.clone()
    } else {
        let mut cols = Vec::new();
        for col in cols_part.split(',') {
            let ident = normalize_identifier(col);
            if !ident.is_empty() {
                cols.push(ident);
            }
        }
        if cols.is_empty() {
            return Err(set_error(
                SENGOO_DB_ERR_PARSE,
                "SELECT column list is empty",
            ));
        }
        cols
    };

    let where_filter = where_part
        .map(|clause| parse_where_clause(clause, params))
        .transpose()?;
    build_select_result(table, &selected_columns, where_filter)
}

fn exec_delete(conn: &mut DbConnection, sql: &str, params: &Value) -> Result<usize, i32> {
    let prefix = "DELETE FROM";
    let rest = sql[prefix.len()..].trim();
    let (table_name_raw, where_part) =
        if let Some(where_pos) = find_keyword_case_insensitive(rest, " WHERE ") {
            (
                rest[..where_pos].trim(),
                Some(rest[where_pos + " WHERE ".len()..].trim()),
            )
        } else {
            (rest.trim_end_matches(';').trim(), None)
        };
    let table_name = normalize_identifier(table_name_raw);
    if table_name.is_empty() {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "DELETE missing table name"));
    }
    let Some(table) = conn.tables.get_mut(&table_name) else {
        return Err(set_error(
            SENGOO_DB_ERR_NOT_FOUND,
            format!("table '{table_name}' not found"),
        ));
    };
    let before = table.rows.len();
    if let Some(clause) = where_part {
        let (col, expected) = parse_where_clause(clause, params)?;
        let Some(col_idx) = table.columns.iter().position(|c| c == &col) else {
            return Err(set_error(
                SENGOO_DB_ERR_NOT_FOUND,
                format!("column '{col}' not found"),
            ));
        };
        table.rows.retain(|row| row.get(col_idx) != Some(&expected));
    } else {
        table.rows.clear();
    }
    Ok(before.saturating_sub(table.rows.len()))
}

fn execute_statement(conn: &mut DbConnection, sql: &str, params: &Value) -> Result<usize, i32> {
    let normalized = sql.trim();
    if normalized.is_empty() {
        return Err(set_error(SENGOO_DB_ERR_PARSE, "empty SQL"));
    }
    let upper = normalized.to_ascii_uppercase();
    if upper.starts_with("CREATE TABLE") {
        exec_create_table(conn, normalized)
    } else if upper.starts_with("INSERT INTO") {
        exec_insert(conn, normalized, params)
    } else if upper.starts_with("DELETE FROM") {
        exec_delete(conn, normalized, params)
    } else {
        Err(set_error(
            SENGOO_DB_ERR_PARSE,
            "unsupported SQL for exec; supported: CREATE TABLE / INSERT INTO / DELETE FROM",
        ))
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "<invalid-json>".to_string()),
    }
}

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
    use std::sync::MutexGuard;

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
