use serde_json::Value;

use super::state::set_error;
use super::status::*;

pub(super) fn parse_c_string(ptr: *const u8) -> Result<String, i32> {
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

pub(super) fn parse_optional_json(ptr: *const u8) -> Result<Value, i32> {
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

pub(super) fn copy_bytes_to_buffer(bytes: &[u8], buffer: *mut u8, capacity: usize) -> i64 {
    if buffer.is_null() {
        return set_error(SENGOO_DB_ERR_INVALID_ARGUMENT, "null output buffer") as i64;
    }
    let copy_len = bytes.len().min(capacity);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, copy_len);
    }
    copy_len as i64
}
