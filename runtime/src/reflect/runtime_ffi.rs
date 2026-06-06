use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

mod buffer;
mod callback;
mod lua;
mod object;

// Preserve the pre-split Rust paths alongside the stable no_mangle C ABI.
#[allow(unused_imports)]
pub use buffer::{
    sengoo_ffi_buffer_copy_in, sengoo_ffi_buffer_copy_out, sengoo_ffi_buffer_free,
    sengoo_ffi_buffer_from_bytes, sengoo_ffi_buffer_len, sengoo_ffi_buffer_new,
    sengoo_ffi_buffer_ptr,
};
#[allow(unused_imports)]
pub use callback::{
    sengoo_ffi_callback_bind_i64, sengoo_ffi_callback_dispatch_i64, sengoo_ffi_callback_unbind,
};
#[allow(unused_imports)]
pub use lua::{
    sengoo_lua_call_i64, sengoo_lua_close, sengoo_lua_exec, sengoo_lua_load, sengoo_lua_open,
};
#[allow(unused_imports)]
pub use object::{
    sengoo_ffi_object_call_i64, sengoo_ffi_object_call_i64_value, sengoo_ffi_object_create,
    sengoo_ffi_object_create_value, sengoo_ffi_object_destroy, sengoo_ffi_object_raw_ptr,
};

pub const SENGOO_FFI_STATUS_OK: i32 = 0;
pub const SENGOO_FFI_ERR_INVALID_ARGUMENT: i32 = -2001;
pub const SENGOO_FFI_ERR_INVALID_HANDLE: i32 = -2002;
pub const SENGOO_FFI_ERR_SYMBOL_NOT_FOUND: i32 = -2003;
pub const SENGOO_FFI_ERR_CALL_FAILED: i32 = -2004;
pub const SENGOO_FFI_ERR_PARSE: i32 = -2005;
pub const SENGOO_FFI_ERR_BUFFER: i32 = -2006;
#[allow(dead_code)]
pub const SENGOO_FFI_ERR_UNSUPPORTED: i32 = -2007;
pub const SENGOO_FFI_ERR_INTERNAL: i32 = -2099;

pub const SENGOO_RUNTIME_MAX_BUFFER_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
struct FfiErrorState {
    code: i32,
    message: String,
}

impl Default for FfiErrorState {
    fn default() -> Self {
        Self {
            code: SENGOO_FFI_STATUS_OK,
            message: String::new(),
        }
    }
}

#[derive(Debug)]
enum CLibKind {
    Native(usize),
    Builtin,
}

#[derive(Debug)]
struct CApiLibrary {
    kind: CLibKind,
}

static NEXT_FFI_HANDLE: AtomicU64 = AtomicU64::new(1);
static C_LIBS: OnceLock<Mutex<HashMap<u64, CApiLibrary>>> = OnceLock::new();
static FFI_LAST_ERROR: OnceLock<Mutex<FfiErrorState>> = OnceLock::new();

fn c_libs() -> &'static Mutex<HashMap<u64, CApiLibrary>> {
    C_LIBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ffi_last_error() -> &'static Mutex<FfiErrorState> {
    FFI_LAST_ERROR.get_or_init(|| Mutex::new(FfiErrorState::default()))
}

fn next_handle() -> u64 {
    NEXT_FFI_HANDLE.fetch_add(1, Ordering::Relaxed)
}

fn clear_error() {
    if let Ok(mut state) = ffi_last_error().lock() {
        state.code = SENGOO_FFI_STATUS_OK;
        state.message.clear();
    }
}

fn set_error(code: i32, message: impl Into<String>) -> i32 {
    if let Ok(mut state) = ffi_last_error().lock() {
        state.code = code;
        state.message = message.into();
    }
    code
}

fn parse_c_string(ptr: *const u8) -> Result<String, i32> {
    if ptr.is_null() {
        return Err(set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            "null C string pointer",
        ));
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 512 * 1024 {
                return Err(set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "C string too long",
                ));
            }
        }
        let bytes = std::slice::from_raw_parts(ptr, len);
        std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "invalid UTF-8 string"))
    }
}

fn copy_bytes_to_buffer(bytes: &[u8], buffer: *mut u8, capacity: usize) -> i64 {
    if buffer.is_null() {
        return set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "null output buffer") as i64;
    }
    let copy_len = bytes.len().min(capacity);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, copy_len);
    }
    copy_len as i64
}

fn ffi_read_i64_args(argc: usize, argv: *const i64) -> Result<Vec<i64>, i32> {
    if argc == 0 {
        return Ok(Vec::new());
    }
    if argv.is_null() {
        return Err(set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            "argv pointer is null while argc > 0",
        ));
    }
    let args = unsafe { std::slice::from_raw_parts(argv, argc) };
    Ok(args.to_vec())
}

fn ffi_direct_i64_args(argc: usize, args: &[i64]) -> Result<&[i64], i32> {
    if argc > args.len() {
        return Err(set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            format!(
                "direct i64 call argc {argc} exceeds supported arity {}",
                args.len()
            ),
        ));
    }
    Ok(&args[..argc])
}

fn ffi_invoke_builtin(symbol: &str, args: &[i64]) -> Result<i64, i32> {
    match symbol {
        "sengoo_ffi_builtin_add2" => {
            if args.len() != 2 {
                return Err(set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "builtin add2 requires 2 args",
                ));
            }
            Ok(sengoo_ffi_builtin_add2(args[0], args[1]))
        }
        "sengoo_ffi_builtin_mul3" => {
            if args.len() != 3 {
                return Err(set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "builtin mul3 requires 3 args",
                ));
            }
            Ok(sengoo_ffi_builtin_mul3(args[0], args[1], args[2]))
        }
        "sengoo_ffi_builtin_counter_new" => {
            if args.len() != 1 {
                return Err(set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "builtin counter_new requires 1 arg",
                ));
            }
            Ok(sengoo_ffi_builtin_counter_new(args[0]))
        }
        "sengoo_ffi_builtin_counter_add" => {
            if args.len() != 2 {
                return Err(set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "builtin counter_add requires 2 args",
                ));
            }
            Ok(sengoo_ffi_builtin_counter_add(args[0], args[1]))
        }
        "sengoo_ffi_builtin_counter_drop" => {
            if args.len() != 1 {
                return Err(set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "builtin counter_drop requires 1 arg",
                ));
            }
            Ok(sengoo_ffi_builtin_counter_drop(args[0]))
        }
        "sengoo_ffi_builtin_sum4" => {
            if args.len() != 4 {
                return Err(set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "builtin sum4 requires 4 args",
                ));
            }
            Ok(sengoo_ffi_builtin_sum4(args[0], args[1], args[2], args[3]))
        }
        _ => Err(set_error(
            SENGOO_FFI_ERR_SYMBOL_NOT_FOUND,
            format!("builtin symbol '{symbol}' not found"),
        )),
    }
}

fn ffi_call_i64_with_library(lib: &CApiLibrary, symbol: &str, args: &[i64]) -> Result<i64, i32> {
    match &lib.kind {
        CLibKind::Builtin => ffi_invoke_builtin(symbol, args),
        CLibKind::Native(native_handle) => unsafe {
            ffi_invoke_native_i64(*native_handle as *mut c_void, symbol, args)
        },
    }
}

unsafe fn ffi_invoke_native_i64(
    lib_handle: *mut c_void,
    symbol: &str,
    args: &[i64],
) -> Result<i64, i32> {
    let raw = unsafe { super::native_loader::get(lib_handle, symbol) }
        .map_err(|reason| set_error(SENGOO_FFI_ERR_SYMBOL_NOT_FOUND, reason))?;

    match args.len() {
        0 => {
            let f: unsafe extern "C" fn() -> i64 =
                unsafe { std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> i64>(raw) };
            Ok(unsafe { f() })
        }
        1 => {
            let f: unsafe extern "C" fn(i64) -> i64 = unsafe {
                std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64) -> i64>(raw)
            };
            Ok(unsafe { f(args[0]) })
        }
        2 => {
            let f: unsafe extern "C" fn(i64, i64) -> i64 = unsafe {
                std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64, i64) -> i64>(raw)
            };
            Ok(unsafe { f(args[0], args[1]) })
        }
        3 => {
            let f: unsafe extern "C" fn(i64, i64, i64) -> i64 = unsafe {
                std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64, i64, i64) -> i64>(raw)
            };
            Ok(unsafe { f(args[0], args[1], args[2]) })
        }
        4 => {
            let f: unsafe extern "C" fn(i64, i64, i64, i64) -> i64 = unsafe {
                std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64, i64, i64, i64) -> i64>(
                    raw,
                )
            };
            Ok(unsafe { f(args[0], args[1], args[2], args[3]) })
        }
        arity => Err(set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            format!("unsupported native call arity {arity}, supported 0..=4"),
        )),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_builtin_add2(a: i64, b: i64) -> i64 {
    a + b
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_builtin_mul3(a: i64, b: i64, c: i64) -> i64 {
    a * b * c
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_builtin_counter_new(initial: i64) -> i64 {
    let boxed = Box::new(initial);
    Box::into_raw(boxed) as i64
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_builtin_counter_add(ptr: i64, delta: i64) -> i64 {
    if ptr == 0 {
        return 0;
    }
    let counter = unsafe { &mut *(ptr as *mut i64) };
    *counter += delta;
    *counter
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_builtin_counter_drop(ptr: i64) -> i64 {
    if ptr == 0 {
        return 0;
    }
    unsafe {
        drop(Box::from_raw(ptr as *mut i64));
    }
    0
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_builtin_sum4(a: i64, b: i64, c: i64, d: i64) -> i64 {
    a + b + c + d
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_last_error_code() -> i32 {
    ffi_last_error()
        .lock()
        .map(|state| state.code)
        .unwrap_or(SENGOO_FFI_ERR_INTERNAL)
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_last_error_len() -> i64 {
    ffi_last_error()
        .lock()
        .map(|state| state.message.len() as i64)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_last_error_copy(buffer: *mut u8, capacity: usize) -> i64 {
    let message = ffi_last_error()
        .lock()
        .map(|state| state.message.clone())
        .unwrap_or_default();
    copy_bytes_to_buffer(message.as_bytes(), buffer, capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_last_error_clear() -> i32 {
    clear_error();
    SENGOO_FFI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_c_open(path: *const u8) -> u64 {
    clear_error();
    let path = match parse_c_string(path) {
        Ok(value) => value,
        Err(_) => return 0,
    };

    let kind = if path == "self://builtin" {
        CLibKind::Builtin
    } else {
        let handle = unsafe { super::native_loader::open(Path::new(&path)) }.map_err(|reason| {
            set_error(
                SENGOO_FFI_ERR_CALL_FAILED,
                format!("failed to load native library '{path}': {reason}"),
            )
        });
        match handle {
            Ok(handle) => CLibKind::Native(handle as usize),
            Err(_) => return 0,
        }
    };

    let handle = next_handle();
    let mut libs = match c_libs().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned");
            return 0;
        }
    };
    libs.insert(handle, CApiLibrary { kind });
    handle
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_c_close(handle: u64) -> i32 {
    clear_error();
    let lib = {
        let mut libs = match c_libs().lock() {
            Ok(table) => table,
            Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned"),
        };
        match libs.remove(&handle) {
            Some(lib) => lib,
            None => {
                return set_error(
                    SENGOO_FFI_ERR_INVALID_HANDLE,
                    format!("ffi c library handle {handle} not found"),
                );
            }
        }
    };

    match lib.kind {
        CLibKind::Builtin => SENGOO_FFI_STATUS_OK,
        CLibKind::Native(native_handle) => unsafe {
            match super::native_loader::close(native_handle as *mut c_void) {
                Ok(()) => SENGOO_FFI_STATUS_OK,
                Err(reason) => set_error(
                    SENGOO_FFI_ERR_CALL_FAILED,
                    format!("failed to close native library: {reason}"),
                ),
            }
        },
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_c_call_i64(
    handle: u64,
    symbol: *const u8,
    argc: usize,
    argv: *const i64,
    out_value: *mut i64,
) -> i32 {
    clear_error();
    if out_value.is_null() {
        return set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "out_value pointer is null");
    }
    let symbol = match parse_c_string(symbol) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let args = match ffi_read_i64_args(argc, argv) {
        Ok(args) => args,
        Err(code) => return code,
    };

    let libs = match c_libs().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "ffi c library table poisoned"),
    };
    let Some(lib) = libs.get(&handle) else {
        return set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("ffi c library handle {handle} not found"),
        );
    };

    let value = ffi_call_i64_with_library(lib, &symbol, &args);
    let value = match value {
        Ok(value) => value,
        Err(code) => return code,
    };

    unsafe {
        *out_value = value;
    }
    SENGOO_FFI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_ffi_c_call_i64_value(
    handle: u64,
    symbol: *const u8,
    argc: usize,
    a0: i64,
    a1: i64,
    a2: i64,
    a3: i64,
) -> i64 {
    clear_error();
    let full = [a0, a1, a2, a3];
    let args = match ffi_direct_i64_args(argc, &full) {
        Ok(args) => args,
        Err(_) => return 0,
    };
    let mut out = 0_i64;
    if sengoo_ffi_c_call_i64(handle, symbol, args.len(), args.as_ptr(), &mut out) == 0 {
        out
    } else {
        0
    }
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

    #[test]
    fn ffi_c_builtin_path_smoke() {
        let _guard = test_lock();
        let path = c_str("self://builtin");
        let lib = sengoo_ffi_c_open(path.as_ptr());
        assert!(lib != 0);

        let symbol_add = c_str("sengoo_ffi_builtin_add2");
        let args_add = [10_i64, 32_i64];
        let mut out_add = 0_i64;
        assert_eq!(
            sengoo_ffi_c_call_i64(
                lib,
                symbol_add.as_ptr(),
                args_add.len(),
                args_add.as_ptr(),
                &mut out_add as *mut i64
            ),
            SENGOO_FFI_STATUS_OK
        );
        assert_eq!(out_add, 42);

        let symbol_mul = c_str("sengoo_ffi_builtin_mul3");
        let args_mul = [2_i64, 3_i64, 4_i64];
        let mut out_mul = 0_i64;
        assert_eq!(
            sengoo_ffi_c_call_i64(
                lib,
                symbol_mul.as_ptr(),
                args_mul.len(),
                args_mul.as_ptr(),
                &mut out_mul as *mut i64
            ),
            SENGOO_FFI_STATUS_OK
        );
        assert_eq!(out_mul, 24);

        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_c_builtin_value_call_avoids_pointer_slots() {
        let _guard = test_lock();
        let path = c_str("self://builtin");
        let lib = sengoo_ffi_c_open(path.as_ptr());
        assert_ne!(lib, 0);

        let symbol = c_str("sengoo_ffi_builtin_add2");
        let value = sengoo_ffi_c_call_i64_value(lib, symbol.as_ptr(), 2, 10, 32, 0, 0);
        assert_eq!(value, 42);
        assert_eq!(sengoo_ffi_last_error_code(), SENGOO_FFI_STATUS_OK);

        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_c_error_paths() {
        let _guard = test_lock();
        let path = c_str("self://builtin");
        let lib = sengoo_ffi_c_open(path.as_ptr());
        assert!(lib != 0);

        let missing_symbol = c_str("unknown_symbol");
        let mut out = 0_i64;
        let rc = sengoo_ffi_c_call_i64(
            lib,
            missing_symbol.as_ptr(),
            0,
            std::ptr::null(),
            &mut out as *mut i64,
        );
        assert_eq!(rc, SENGOO_FFI_ERR_SYMBOL_NOT_FOUND);
        assert_eq!(
            sengoo_ffi_last_error_code(),
            SENGOO_FFI_ERR_SYMBOL_NOT_FOUND
        );

        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_STATUS_OK);
        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_ERR_INVALID_HANDLE);
    }

    #[test]
    fn lua_load_exec_call_smoke() {
        let _guard = test_lock();
        let lua = sengoo_lua_open();
        assert!(lua != 0);

        let script = c_str("function add(a,b)\n  return a + b\nend");
        assert_eq!(sengoo_lua_load(lua, script.as_ptr()), SENGOO_FFI_STATUS_OK);
        assert_eq!(sengoo_lua_exec(lua, std::ptr::null()), SENGOO_FFI_STATUS_OK);

        let func = c_str("add");
        let args = [2_i64, 5_i64];
        let mut out = 0_i64;
        assert_eq!(
            sengoo_lua_call_i64(
                lua,
                func.as_ptr(),
                args.len(),
                args.as_ptr(),
                &mut out as *mut i64
            ),
            SENGOO_FFI_STATUS_OK
        );
        assert_eq!(out, 7);

        assert_eq!(sengoo_lua_close(lua), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn lua_negative_paths() {
        let _guard = test_lock();
        let lua = sengoo_lua_open();
        assert!(lua != 0);

        let bad_script = c_str("function broken(a,b) return a ++ b end");
        let rc = sengoo_lua_exec(lua, bad_script.as_ptr());
        assert_eq!(rc, SENGOO_FFI_ERR_PARSE);

        let script = c_str("function sub(a,b) return a - b end");
        assert_eq!(sengoo_lua_exec(lua, script.as_ptr()), SENGOO_FFI_STATUS_OK);

        let missing = c_str("missing");
        let args = [1_i64];
        let mut out = 0_i64;
        let rc = sengoo_lua_call_i64(
            lua,
            missing.as_ptr(),
            args.len(),
            args.as_ptr(),
            &mut out as *mut i64,
        );
        assert_eq!(rc, SENGOO_FFI_ERR_SYMBOL_NOT_FOUND);

        assert_eq!(sengoo_lua_close(lua), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_object_lifecycle_with_builtin_counter() {
        let _guard = test_lock();
        let path = c_str("self://builtin");
        let lib = sengoo_ffi_c_open(path.as_ptr());
        assert!(lib != 0);

        let ctor = c_str("sengoo_ffi_builtin_counter_new");
        let dtor = c_str("sengoo_ffi_builtin_counter_drop");
        let init_args = [5_i64];
        let obj = sengoo_ffi_object_create(
            lib,
            ctor.as_ptr(),
            init_args.len(),
            init_args.as_ptr(),
            dtor.as_ptr(),
        );
        assert!(obj != 0);

        let method = c_str("sengoo_ffi_builtin_counter_add");
        let add_args = [7_i64];
        let mut out = 0_i64;
        assert_eq!(
            sengoo_ffi_object_call_i64(
                obj,
                method.as_ptr(),
                add_args.len(),
                add_args.as_ptr(),
                &mut out as *mut i64
            ),
            SENGOO_FFI_STATUS_OK
        );
        assert_eq!(out, 12);
        assert_ne!(sengoo_ffi_object_raw_ptr(obj), 0);
        assert_eq!(sengoo_ffi_object_destroy(obj), SENGOO_FFI_STATUS_OK);
        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_object_value_calls_avoid_pointer_slots() {
        let _guard = test_lock();
        let path = c_str("self://builtin");
        let lib = sengoo_ffi_c_open(path.as_ptr());
        assert_ne!(lib, 0);

        let ctor = c_str("sengoo_ffi_builtin_counter_new");
        let dtor = c_str("sengoo_ffi_builtin_counter_drop");
        let obj = sengoo_ffi_object_create_value(lib, ctor.as_ptr(), 1, 5, 0, 0, 0, dtor.as_ptr());
        assert_ne!(obj, 0);

        let method = c_str("sengoo_ffi_builtin_counter_add");
        let value = sengoo_ffi_object_call_i64_value(obj, method.as_ptr(), 1, 7, 0, 0);
        assert_eq!(value, 12);
        assert_eq!(sengoo_ffi_last_error_code(), SENGOO_FFI_STATUS_OK);

        assert_eq!(sengoo_ffi_object_destroy(obj), SENGOO_FFI_STATUS_OK);
        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_callback_dispatch_i64_smoke() {
        let _guard = test_lock();
        let path = c_str("self://builtin");
        let lib = sengoo_ffi_c_open(path.as_ptr());
        assert!(lib != 0);

        let symbol = c_str("sengoo_ffi_builtin_sum4");
        let callback = sengoo_ffi_callback_bind_i64(lib, symbol.as_ptr(), 4);
        assert!(callback != 0);
        let value = sengoo_ffi_callback_dispatch_i64(callback, 1, 2, 3, 4, 0, 0);
        assert_eq!(value, 10);

        assert_eq!(sengoo_ffi_callback_unbind(callback), SENGOO_FFI_STATUS_OK);
        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_buffer_interop_smoke() {
        let _guard = test_lock();
        let allocated = sengoo_ffi_buffer_new(8);
        assert_ne!(allocated, 0);
        assert_eq!(sengoo_ffi_buffer_len(allocated), 8);
        assert_eq!(sengoo_ffi_buffer_free(allocated), SENGOO_FFI_STATUS_OK);

        let input = b"protobuf-payload";
        let handle = sengoo_ffi_buffer_from_bytes(input.as_ptr(), input.len());
        assert!(handle != 0);
        assert_eq!(sengoo_ffi_buffer_len(handle), input.len() as i64);
        assert_ne!(sengoo_ffi_buffer_ptr(handle), 0);

        let mut out = vec![0u8; input.len()];
        let copied = sengoo_ffi_buffer_copy_out(handle, out.as_mut_ptr(), out.len());
        assert_eq!(copied, input.len() as i64);
        assert_eq!(out.as_slice(), input);

        let next = b"abc";
        assert_eq!(
            sengoo_ffi_buffer_copy_in(handle, next.as_ptr(), next.len()),
            SENGOO_FFI_STATUS_OK
        );
        assert_eq!(sengoo_ffi_buffer_len(handle), 3);
        assert_eq!(sengoo_ffi_buffer_free(handle), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_buffer_rejects_oversized_capacity() {
        let _guard = test_lock();
        let oversized = SENGOO_RUNTIME_MAX_BUFFER_BYTES + 1;
        let handle = sengoo_ffi_buffer_new(oversized);
        assert_eq!(handle, 0);
        assert_eq!(
            sengoo_ffi_last_error_code(),
            SENGOO_FFI_ERR_INVALID_ARGUMENT
        );
    }

    #[test]
    fn ffi_buffer_double_free_returns_invalid_handle() {
        let _guard = test_lock();
        let handle = sengoo_ffi_buffer_new(4);
        assert_ne!(handle, 0);
        assert_eq!(sengoo_ffi_buffer_free(handle), SENGOO_FFI_STATUS_OK);
        assert_eq!(
            sengoo_ffi_buffer_free(handle),
            SENGOO_FFI_ERR_INVALID_HANDLE
        );
    }

    #[test]
    fn ffi_buffer_use_after_free_returns_invalid_handle() {
        let _guard = test_lock();
        let handle = sengoo_ffi_buffer_new(4);
        assert_ne!(handle, 0);
        assert_eq!(sengoo_ffi_buffer_free(handle), SENGOO_FFI_STATUS_OK);
        assert_eq!(
            sengoo_ffi_buffer_len(handle),
            SENGOO_FFI_ERR_INVALID_HANDLE as i64
        );
    }

    #[test]
    fn ffi_c_call_rejects_unsupported_arity() {
        let _guard = test_lock();
        let path = c_str("self://builtin");
        let lib = sengoo_ffi_c_open(path.as_ptr());
        assert_ne!(lib, 0);

        let symbol = c_str("sengoo_ffi_builtin_add2");
        let args = [1_i64, 2_i64, 3_i64, 4_i64, 5_i64];
        let mut out = 0_i64;
        let rc = sengoo_ffi_c_call_i64(
            lib,
            symbol.as_ptr(),
            args.len(),
            args.as_ptr(),
            &mut out as *mut i64,
        );
        assert_eq!(rc, SENGOO_FFI_ERR_INVALID_ARGUMENT);

        assert_eq!(sengoo_ffi_c_close(lib), SENGOO_FFI_STATUS_OK);
    }

    #[test]
    fn ffi_callback_rejects_invalid_library_handle() {
        let _guard = test_lock();
        let symbol = c_str("sengoo_ffi_builtin_sum4");
        let callback = sengoo_ffi_callback_bind_i64(999_999, symbol.as_ptr(), 4);
        assert_eq!(callback, 0);
        assert_eq!(sengoo_ffi_last_error_code(), SENGOO_FFI_ERR_INVALID_HANDLE);
    }
}
