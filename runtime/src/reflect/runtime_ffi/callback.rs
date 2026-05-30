use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use super::{
    c_libs, clear_error, ffi_call_i64_with_library, parse_c_string, set_error,
    SENGOO_FFI_ERR_INTERNAL, SENGOO_FFI_ERR_INVALID_ARGUMENT, SENGOO_FFI_ERR_INVALID_HANDLE,
    SENGOO_FFI_STATUS_OK,
};

#[derive(Clone, Debug)]
struct FfiCallbackBinding {
    lib_handle: u64,
    symbol: String,
    arity: usize,
}

static NEXT_FFI_CALLBACK_ID: AtomicU64 = AtomicU64::new(1);
static FFI_CALLBACKS: OnceLock<Mutex<HashMap<u64, FfiCallbackBinding>>> = OnceLock::new();

fn ffi_callbacks() -> &'static Mutex<HashMap<u64, FfiCallbackBinding>> {
    FFI_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_callback_id() -> u64 {
    NEXT_FFI_CALLBACK_ID.fetch_add(1, Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_callback_bind_i64(
    lib_handle: u64,
    symbol: *const u8,
    arity: usize,
) -> u64 {
    clear_error();
    if arity > 6 {
        set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            "callback arity > 6 is not supported",
        );
        return 0;
    }
    let symbol = match parse_c_string(symbol) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let libs = match c_libs().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned");
            return 0;
        }
    };
    if !libs.contains_key(&lib_handle) {
        set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi c library handle {lib_handle} not found"),
        );
        return 0;
    }
    drop(libs);

    let id = next_callback_id();
    let binding = FfiCallbackBinding {
        lib_handle,
        symbol,
        arity,
    };
    let mut callbacks = match ffi_callbacks().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_FFI_ERR_INTERNAL, "ffi callback table poisoned");
            return 0;
        }
    };
    callbacks.insert(id, binding);
    id
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_callback_unbind(callback_id: u64) -> i32 {
    clear_error();
    let mut callbacks = match ffi_callbacks().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi callback table poisoned"),
    };
    if callbacks.remove(&callback_id).is_some() {
        SENGOO_FFI_STATUS_OK
    } else {
        set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi callback id {callback_id} not found"),
        )
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_callback_dispatch_i64(
    callback_id: u64,
    a0: i64,
    a1: i64,
    a2: i64,
    a3: i64,
    a4: i64,
    a5: i64,
) -> i64 {
    clear_error();
    let binding = {
        let callbacks = match ffi_callbacks().lock() {
            Ok(table) => table,
            Err(_) => {
                return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi callback table poisoned") as i64
            }
        };
        match callbacks.get(&callback_id) {
            Some(binding) => binding.clone(),
            None => {
                return set_error(
                    SENGOO_FFI_ERR_INVALID_HANDLE,
                    format!("ffi callback id {callback_id} not found"),
                ) as i64;
            }
        }
    };

    let full = [a0, a1, a2, a3, a4, a5];
    let args = &full[..binding.arity.min(full.len())];
    let value = {
        let libs = match c_libs().lock() {
            Ok(table) => table,
            Err(_) => {
                return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned") as i64
            }
        };
        let Some(lib) = libs.get(&binding.lib_handle) else {
            return set_error(
                SENGOO_FFI_ERR_INVALID_HANDLE,
                format!("ffi c library handle {} not found", binding.lib_handle),
            ) as i64;
        };
        match ffi_call_i64_with_library(lib, &binding.symbol, args) {
            Ok(value) => value,
            Err(code) => return code as i64,
        }
    };
    value
}
