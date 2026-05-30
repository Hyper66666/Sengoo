use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::{
    c_libs, clear_error, ffi_call_i64_with_library, ffi_direct_i64_args, ffi_read_i64_args,
    next_handle, parse_c_string, set_error, SENGOO_FFI_ERR_CALL_FAILED, SENGOO_FFI_ERR_INTERNAL,
    SENGOO_FFI_ERR_INVALID_ARGUMENT, SENGOO_FFI_ERR_INVALID_HANDLE, SENGOO_FFI_STATUS_OK,
};

#[derive(Clone, Debug)]
struct FfiObject {
    lib_handle: u64,
    raw_ptr: i64,
    destructor_symbol: Option<String>,
}

static FFI_OBJECTS: OnceLock<Mutex<HashMap<u64, FfiObject>>> = OnceLock::new();

fn ffi_objects() -> &'static Mutex<HashMap<u64, FfiObject>> {
    FFI_OBJECTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_object_create(
    lib_handle: u64,
    constructor_symbol: *const u8,
    argc: usize,
    argv: *const i64,
    destructor_symbol: *const u8,
) -> u64 {
    clear_error();
    let constructor_symbol = match parse_c_string(constructor_symbol) {
        Ok(value) => value,
        Err(_) => return 0,
    };
    let args = match ffi_read_i64_args(argc, argv) {
        Ok(args) => args,
        Err(_) => return 0,
    };
    let destructor_symbol = if destructor_symbol.is_null() {
        None
    } else {
        match parse_c_string(destructor_symbol) {
            Ok(value) => Some(value),
            Err(_) => return 0,
        }
    };

    let raw_ptr = {
        let libs = match c_libs().lock() {
            Ok(table) => table,
            Err(_) => {
                set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned");
                return 0;
            }
        };
        let Some(lib) = libs.get(&lib_handle) else {
            set_error(
                SENGOO_FFI_ERR_INVALID_HANDLE,
                format!("ffi c library handle {lib_handle} not found"),
            );
            return 0;
        };
        match ffi_call_i64_with_library(lib, &constructor_symbol, &args) {
            Ok(value) => value,
            Err(_) => return 0,
        }
    };
    if raw_ptr == 0 {
        set_error(
            SENGOO_FFI_ERR_CALL_FAILED,
            format!("constructor '{}' returned null pointer", constructor_symbol),
        );
        return 0;
    }

    let object_handle = next_handle();
    let object = FfiObject {
        lib_handle,
        raw_ptr,
        destructor_symbol,
    };
    let mut objects = match ffi_objects().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_FFI_ERR_INTERNAL, "ffi object table poisoned");
            return 0;
        }
    };
    objects.insert(object_handle, object);
    object_handle
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_object_create_value(
    lib_handle: u64,
    constructor_symbol: *const u8,
    argc: usize,
    a0: i64,
    a1: i64,
    a2: i64,
    a3: i64,
    destructor_symbol: *const u8,
) -> u64 {
    clear_error();
    let full = [a0, a1, a2, a3];
    let args = match ffi_direct_i64_args(argc, &full) {
        Ok(args) => args,
        Err(_) => return 0,
    };
    sengoo_ffi_object_create(
        lib_handle,
        constructor_symbol,
        args.len(),
        args.as_ptr(),
        destructor_symbol,
    )
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_object_raw_ptr(object_handle: u64) -> i64 {
    clear_error();
    let objects = match ffi_objects().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi object table poisoned") as i64,
    };
    match objects.get(&object_handle) {
        Some(object) => object.raw_ptr,
        None => set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi object handle {object_handle} not found"),
        ) as i64,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_object_call_i64(
    object_handle: u64,
    method_symbol: *const u8,
    argc: usize,
    argv: *const i64,
    out_value: *mut i64,
) -> i32 {
    clear_error();
    if out_value.is_null() {
        return set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "out_value pointer is null");
    }
    let method_symbol = match parse_c_string(method_symbol) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let args = match ffi_read_i64_args(argc, argv) {
        Ok(args) => args,
        Err(code) => return code,
    };

    let (lib_handle, raw_ptr) = {
        let objects = match ffi_objects().lock() {
            Ok(table) => table,
            Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi object table poisoned"),
        };
        let Some(object) = objects.get(&object_handle) else {
            return set_error(
                SENGOO_FFI_ERR_INVALID_HANDLE,
                format!("ffi object handle {object_handle} not found"),
            );
        };
        (object.lib_handle, object.raw_ptr)
    };

    let mut call_args = Vec::with_capacity(args.len() + 1);
    call_args.push(raw_ptr);
    call_args.extend_from_slice(&args);

    let value = {
        let libs = match c_libs().lock() {
            Ok(table) => table,
            Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned"),
        };
        let Some(lib) = libs.get(&lib_handle) else {
            return set_error(
                SENGOO_FFI_ERR_INVALID_HANDLE,
                format!("ffi c library handle {lib_handle} not found"),
            );
        };
        match ffi_call_i64_with_library(lib, &method_symbol, &call_args) {
            Ok(value) => value,
            Err(code) => return code,
        }
    };

    unsafe {
        *out_value = value;
    }
    SENGOO_FFI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_object_call_i64_value(
    object_handle: u64,
    method_symbol: *const u8,
    argc: usize,
    a0: i64,
    a1: i64,
    a2: i64,
) -> i64 {
    clear_error();
    let full = [a0, a1, a2];
    let args = match ffi_direct_i64_args(argc, &full) {
        Ok(args) => args,
        Err(_) => return 0,
    };
    let mut out = 0_i64;
    if sengoo_ffi_object_call_i64(
        object_handle,
        method_symbol,
        args.len(),
        args.as_ptr(),
        &mut out,
    ) == 0
    {
        out
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_object_destroy(object_handle: u64) -> i32 {
    clear_error();
    let object = {
        let mut objects = match ffi_objects().lock() {
            Ok(table) => table,
            Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi object table poisoned"),
        };
        match objects.remove(&object_handle) {
            Some(object) => object,
            None => {
                return set_error(
                    SENGOO_FFI_ERR_INVALID_HANDLE,
                    format!("ffi object handle {object_handle} not found"),
                );
            }
        }
    };

    if let Some(dtor) = object.destructor_symbol {
        let args = [object.raw_ptr];
        let rc = {
            let libs = match c_libs().lock() {
                Ok(table) => table,
                Err(_) => {
                    return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned")
                }
            };
            let Some(lib) = libs.get(&object.lib_handle) else {
                return set_error(
                    SENGOO_FFI_ERR_INVALID_HANDLE,
                    format!("ffi c library handle {} not found", object.lib_handle),
                );
            };
            ffi_call_i64_with_library(lib, &dtor, &args)
        };
        if rc.is_err() {
            return set_error(
                SENGOO_FFI_ERR_CALL_FAILED,
                format!("destructor '{}' invocation failed", dtor),
            );
        }
    }
    SENGOO_FFI_STATUS_OK
}
