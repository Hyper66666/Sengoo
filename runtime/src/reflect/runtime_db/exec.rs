use serde_json::Value;

use super::sql::*;
use super::state::*;
use super::status::*;

pub(super) fn exec_create_table(conn: &mut DbConnection, sql: &str) -> Result<usize, i32> {
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

pub(super) fn resolve_insert_columns(table: &DbTable, sql_left: &str) -> Result<Vec<String>, i32> {
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

pub(super) fn exec_insert(
    conn: &mut DbConnection,
    sql: &str,
    params: &Value,
) -> Result<usize, i32> {
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

pub(super) fn build_select_result(
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

pub(super) fn run_select(
    conn: &DbConnection,
    sql: &str,
    params: &Value,
) -> Result<DbQueryResult, i32> {
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

pub(super) fn exec_delete(
    conn: &mut DbConnection,
    sql: &str,
    params: &Value,
) -> Result<usize, i32> {
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

pub(super) fn execute_statement(
    conn: &mut DbConnection,
    sql: &str,
    params: &Value,
) -> Result<usize, i32> {
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
