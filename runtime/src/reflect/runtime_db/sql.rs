use serde_json::Value;

use super::state::set_error;
use super::status::*;

pub(super) fn normalize_identifier(raw: &str) -> String {
    raw.trim()
        .trim_matches('`')
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

pub(super) fn find_keyword_case_insensitive(haystack: &str, keyword: &str) -> Option<usize> {
    haystack
        .to_ascii_uppercase()
        .find(&keyword.to_ascii_uppercase())
}

pub(super) fn parse_literal(raw: &str) -> Value {
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

pub(super) fn resolve_param_token(token: &str, params: &Value) -> Result<Value, i32> {
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

pub(super) fn parse_where_clause(where_clause: &str, params: &Value) -> Result<(String, Value), i32> {
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

pub(super) fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Number(v) => v.to_string(),
        Value::String(v) => v.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "<invalid-json>".to_string()),
    }
}
