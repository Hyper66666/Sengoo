use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::{
    clear_error, copy_bytes_to_buffer, next_handle, set_error, SENGOO_FFI_ERR_BUFFER,
    SENGOO_FFI_ERR_INTERNAL, SENGOO_FFI_ERR_INVALID_ARGUMENT, SENGOO_FFI_ERR_INVALID_HANDLE,
    SENGOO_FFI_STATUS_OK, SENGOO_RUNTIME_MAX_BUFFER_BYTES,
};

#[derive(Clone, Debug, Default)]
struct FfiBuffer {
    bytes: Vec<u8>,
}

static FFI_BUFFERS: OnceLock<Mutex<HashMap<u64, FfiBuffer>>> = OnceLock::new();

fn ffi_buffers() -> &'static Mutex<HashMap<u64, FfiBuffer>> {
    FFI_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_new(capacity: usize) -> u64 {
    clear_error();
    if capacity > SENGOO_RUNTIME_MAX_BUFFER_BYTES {
        set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            "buffer capacity exceeds runtime limit",
        );
        return 0;
    }
    let handle = next_handle();
    let buffer = FfiBuffer {
        bytes: vec![0u8; capacity],
    };
    let mut table = match ffi_buffers().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_FFI_ERR_INTERNAL, "ffi buffer table poisoned");
            return 0;
        }
    };
    table.insert(handle, buffer);
    handle
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_from_bytes(data: *const u8, len: usize) -> u64 {
    clear_error();
    if len > 0 && data.is_null() {
        set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            "data is null while len > 0",
        );
        return 0;
    }
    let bytes = if len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(data, len) }.to_vec()
    };
    let handle = next_handle();
    let mut table = match ffi_buffers().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_FFI_ERR_INTERNAL, "ffi buffer table poisoned");
            return 0;
        }
    };
    table.insert(handle, FfiBuffer { bytes });
    handle
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_len(buffer_handle: u64) -> i64 {
    clear_error();
    let table = match ffi_buffers().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi buffer table poisoned") as i64,
    };
    match table.get(&buffer_handle) {
        Some(buffer) => buffer.bytes.len() as i64,
        None => set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi buffer handle {buffer_handle} not found"),
        ) as i64,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_copy_out(
    buffer_handle: u64,
    out_buffer: *mut u8,
    out_capacity: usize,
) -> i64 {
    clear_error();
    let table = match ffi_buffers().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi buffer table poisoned") as i64,
    };
    let Some(buffer) = table.get(&buffer_handle) else {
        return set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi buffer handle {buffer_handle} not found"),
        ) as i64;
    };
    if out_capacity < buffer.bytes.len() {
        return set_error(
            SENGOO_FFI_ERR_BUFFER,
            format!(
                "output capacity too small: need {}, got {}",
                buffer.bytes.len(),
                out_capacity
            ),
        ) as i64;
    }
    copy_bytes_to_buffer(&buffer.bytes, out_buffer, out_capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_copy_in(
    buffer_handle: u64,
    src_ptr: *const u8,
    src_len: usize,
) -> i32 {
    clear_error();
    if src_len > 0 && src_ptr.is_null() {
        return set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            "src_ptr is null while src_len > 0",
        );
    }
    let src = if src_len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(src_ptr, src_len) }
    };
    let mut table = match ffi_buffers().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi buffer table poisoned"),
    };
    let Some(buffer) = table.get_mut(&buffer_handle) else {
        return set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi buffer handle {buffer_handle} not found"),
        );
    };
    buffer.bytes.clear();
    buffer.bytes.extend_from_slice(src);
    SENGOO_FFI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_ptr(buffer_handle: u64) -> i64 {
    clear_error();
    let table = match ffi_buffers().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi buffer table poisoned") as i64,
    };
    let Some(buffer) = table.get(&buffer_handle) else {
        return set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi buffer handle {buffer_handle} not found"),
        ) as i64;
    };
    buffer.bytes.as_ptr() as i64
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_free(buffer_handle: u64) -> i32 {
    clear_error();
    let mut table = match ffi_buffers().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi buffer table poisoned"),
    };
    if table.remove(&buffer_handle).is_some() {
        SENGOO_FFI_STATUS_OK
    } else {
        set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi buffer handle {buffer_handle} not found"),
        )
    }
}
