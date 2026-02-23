use std::sync::{Mutex, OnceLock};

pub const SENGOO_PROTO_OK: i32 = 0;
pub const SENGOO_PROTO_ERR_INVALID_ARGUMENT: i32 = -2801;
pub const SENGOO_PROTO_ERR_PARSE: i32 = -2802;
pub const SENGOO_PROTO_ERR_TRUNCATED: i32 = -2803;
pub const SENGOO_PROTO_ERR_INTERNAL: i32 = -2899;

#[derive(Clone, Debug)]
struct ProtoErrorState {
    code: i32,
    message: String,
}

impl Default for ProtoErrorState {
    fn default() -> Self {
        Self {
            code: SENGOO_PROTO_OK,
            message: String::new(),
        }
    }
}

static PROTO_LAST_ERROR: OnceLock<Mutex<ProtoErrorState>> = OnceLock::new();

fn proto_last_error() -> &'static Mutex<ProtoErrorState> {
    PROTO_LAST_ERROR.get_or_init(|| Mutex::new(ProtoErrorState::default()))
}

fn clear_error() {
    if let Ok(mut state) = proto_last_error().lock() {
        state.code = SENGOO_PROTO_OK;
        state.message.clear();
    }
}

fn set_error(code: i32, message: impl Into<String>) -> i32 {
    if let Ok(mut state) = proto_last_error().lock() {
        state.code = code;
        state.message = message.into();
    }
    code
}

fn copy_bytes_to_buffer(bytes: &[u8], buffer: *mut u8, capacity: usize) -> i64 {
    if buffer.is_null() {
        return set_error(SENGOO_PROTO_ERR_INVALID_ARGUMENT, "null output buffer") as i64;
    }
    let copy_len = bytes.len().min(capacity);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, copy_len);
    }
    copy_len as i64
}

fn parse_c_string(ptr: *const u8) -> Result<String, i32> {
    if ptr.is_null() {
        return Err(set_error(
            SENGOO_PROTO_ERR_INVALID_ARGUMENT,
            "null C string pointer",
        ));
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 8 * 1024 * 1024 {
                return Err(set_error(
                    SENGOO_PROTO_ERR_INVALID_ARGUMENT,
                    "string too long",
                ));
            }
        }
        let bytes = std::slice::from_raw_parts(ptr, len);
        std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| set_error(SENGOO_PROTO_ERR_INVALID_ARGUMENT, "invalid UTF-8 string"))
    }
}

fn encode_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
            out.push(byte);
        } else {
            out.push(byte);
            break;
        }
    }
}

fn decode_varint(input: &[u8], index: &mut usize) -> Result<u64, i32> {
    let mut shift = 0u32;
    let mut value = 0u64;
    while *index < input.len() && shift <= 63 {
        let byte = input[*index];
        *index += 1;
        value |= ((byte & 0x7F) as u64) << shift;
        if (byte & 0x80) == 0 {
            return Ok(value);
        }
        shift += 7;
    }
    if *index >= input.len() {
        Err(set_error(
            SENGOO_PROTO_ERR_TRUNCATED,
            "unexpected EOF while decoding varint",
        ))
    } else {
        Err(set_error(SENGOO_PROTO_ERR_PARSE, "invalid varint encoding"))
    }
}

fn encode_user_event_wire(id: u32, name: &str, ts: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + name.len());
    // field 1: id (varint)
    encode_varint((1 << 3 | 0) as u64, &mut out);
    encode_varint(id as u64, &mut out);
    // field 2: name (length-delimited)
    encode_varint((2 << 3 | 2) as u64, &mut out);
    encode_varint(name.len() as u64, &mut out);
    out.extend_from_slice(name.as_bytes());
    // field 3: ts (varint)
    encode_varint((3 << 3 | 0) as u64, &mut out);
    encode_varint(ts, &mut out);
    out
}

fn decode_user_event_wire(input: &[u8]) -> Result<(u32, String, u64), i32> {
    let mut idx = 0usize;
    let mut id = None::<u32>;
    let mut name = None::<String>;
    let mut ts = None::<u64>;

    while idx < input.len() {
        let key = decode_varint(input, &mut idx)?;
        let field_no = key >> 3;
        let wire_type = (key & 0x07) as u8;
        match (field_no, wire_type) {
            (1, 0) => {
                let value = decode_varint(input, &mut idx)?;
                id = Some(value as u32);
            }
            (2, 2) => {
                let len = decode_varint(input, &mut idx)? as usize;
                if idx + len > input.len() {
                    return Err(set_error(
                        SENGOO_PROTO_ERR_TRUNCATED,
                        "string field exceeds input length",
                    ));
                }
                let bytes = &input[idx..idx + len];
                idx += len;
                let value = std::str::from_utf8(bytes)
                    .map_err(|_| set_error(SENGOO_PROTO_ERR_PARSE, "name field is not UTF-8"))?;
                name = Some(value.to_string());
            }
            (3, 0) => {
                let value = decode_varint(input, &mut idx)?;
                ts = Some(value);
            }
            (_, 0) => {
                let _ = decode_varint(input, &mut idx)?;
            }
            (_, 2) => {
                let len = decode_varint(input, &mut idx)? as usize;
                if idx + len > input.len() {
                    return Err(set_error(
                        SENGOO_PROTO_ERR_TRUNCATED,
                        "length-delimited field exceeds input length",
                    ));
                }
                idx += len;
            }
            _ => {
                return Err(set_error(
                    SENGOO_PROTO_ERR_PARSE,
                    format!("unsupported wire type {wire_type} for field {field_no}"),
                ));
            }
        }
    }

    let id = id.ok_or_else(|| set_error(SENGOO_PROTO_ERR_PARSE, "missing required field id"))?;
    let name =
        name.ok_or_else(|| set_error(SENGOO_PROTO_ERR_PARSE, "missing required field name"))?;
    let ts = ts.ok_or_else(|| set_error(SENGOO_PROTO_ERR_PARSE, "missing required field ts"))?;
    Ok((id, name, ts))
}

#[no_mangle]
pub extern "C" fn sengoo_proto_last_error_code() -> i32 {
    proto_last_error()
        .lock()
        .map(|state| state.code)
        .unwrap_or(SENGOO_PROTO_ERR_INTERNAL)
}

#[no_mangle]
pub extern "C" fn sengoo_proto_last_error_len() -> i64 {
    proto_last_error()
        .lock()
        .map(|state| state.message.len() as i64)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn sengoo_proto_last_error_copy(buffer: *mut u8, capacity: usize) -> i64 {
    let message = proto_last_error()
        .lock()
        .map(|state| state.message.clone())
        .unwrap_or_default();
    copy_bytes_to_buffer(message.as_bytes(), buffer, capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_proto_last_error_clear() -> i32 {
    clear_error();
    SENGOO_PROTO_OK
}

#[no_mangle]
pub extern "C" fn sengoo_proto_user_event_encode(
    id: u32,
    name: *const u8,
    ts: u64,
    out_buffer: *mut u8,
    out_capacity: usize,
) -> i64 {
    clear_error();
    let name = match parse_c_string(name) {
        Ok(name) => name,
        Err(code) => return code as i64,
    };
    let bytes = encode_user_event_wire(id, &name, ts);
    if out_capacity < bytes.len() {
        return set_error(
            SENGOO_PROTO_ERR_INVALID_ARGUMENT,
            format!(
                "output buffer too small: need {}, got {}",
                bytes.len(),
                out_capacity
            ),
        ) as i64;
    }
    copy_bytes_to_buffer(&bytes, out_buffer, out_capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_proto_user_event_decode(
    input_ptr: *const u8,
    input_len: usize,
    out_id: *mut u32,
    out_name_buffer: *mut u8,
    out_name_capacity: usize,
    out_ts: *mut u64,
) -> i64 {
    clear_error();
    if input_ptr.is_null() || input_len == 0 {
        return set_error(
            SENGOO_PROTO_ERR_INVALID_ARGUMENT,
            "input buffer is null or empty",
        ) as i64;
    }
    if out_id.is_null() || out_ts.is_null() || out_name_buffer.is_null() {
        return set_error(SENGOO_PROTO_ERR_INVALID_ARGUMENT, "output pointer is null") as i64;
    }
    let input = unsafe { std::slice::from_raw_parts(input_ptr, input_len) };
    let (id, name, ts) = match decode_user_event_wire(input) {
        Ok(value) => value,
        Err(code) => return code as i64,
    };

    let name_bytes = name.as_bytes();
    if out_name_capacity < name_bytes.len() {
        return set_error(
            SENGOO_PROTO_ERR_INVALID_ARGUMENT,
            format!(
                "name output buffer too small: need {}, got {}",
                name_bytes.len(),
                out_name_capacity
            ),
        ) as i64;
    }

    unsafe {
        *out_id = id;
        *out_ts = ts;
        std::ptr::copy_nonoverlapping(name_bytes.as_ptr(), out_name_buffer, name_bytes.len());
    }
    name_bytes.len() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c_str(s: &str) -> Vec<u8> {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    #[test]
    fn protobuf_encode_matches_golden_wire_bytes() {
        let mut output = vec![0u8; 64];
        let name = c_str("alice");
        let written = sengoo_proto_user_event_encode(
            150,
            name.as_ptr(),
            9001,
            output.as_mut_ptr(),
            output.len(),
        );
        assert!(written > 0);
        let got = &output[..written as usize];
        let expected: [u8; 13] = [
            0x08, 0x96, 0x01, 0x12, 0x05, b'a', b'l', b'i', b'c', b'e', 0x18, 0xA9, 0x46,
        ];
        assert_eq!(got, expected);
    }

    #[test]
    fn protobuf_encode_decode_roundtrip() {
        let mut output = vec![0u8; 64];
        let name = c_str("bob");
        let written =
            sengoo_proto_user_event_encode(7, name.as_ptr(), 42, output.as_mut_ptr(), output.len());
        assert!(written > 0);

        let mut out_id = 0u32;
        let mut out_ts = 0u64;
        let mut out_name = vec![0u8; 16];
        let name_len = sengoo_proto_user_event_decode(
            output.as_ptr(),
            written as usize,
            &mut out_id as *mut u32,
            out_name.as_mut_ptr(),
            out_name.len(),
            &mut out_ts as *mut u64,
        );
        assert_eq!(out_id, 7);
        assert_eq!(out_ts, 42);
        assert_eq!(&out_name[..name_len as usize], b"bob");
    }

    #[test]
    fn protobuf_decode_truncated_returns_error() {
        let truncated: [u8; 4] = [0x08, 0x96, 0x01, 0x12];
        let mut out_id = 0u32;
        let mut out_ts = 0u64;
        let mut out_name = vec![0u8; 8];
        let rc = sengoo_proto_user_event_decode(
            truncated.as_ptr(),
            truncated.len(),
            &mut out_id as *mut u32,
            out_name.as_mut_ptr(),
            out_name.len(),
            &mut out_ts as *mut u64,
        );
        assert_eq!(rc, SENGOO_PROTO_ERR_TRUNCATED as i64);
        assert_eq!(sengoo_proto_last_error_code(), SENGOO_PROTO_ERR_TRUNCATED);
        assert!(sengoo_proto_last_error_len() > 0);
    }
}
