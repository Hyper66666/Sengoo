use std::ffi::{c_char, c_int, c_void, CString};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub const SENGOO_LUA54_STATUS_OK: i32 = 0;
pub const SENGOO_LUA54_ERR_INVALID_ARGUMENT: i32 = -2401;
pub const SENGOO_LUA54_ERR_INVALID_HANDLE: i32 = -2402;
pub const SENGOO_LUA54_ERR_LIBRARY_LOAD: i32 = -2403;
pub const SENGOO_LUA54_ERR_SYMBOL_LOAD: i32 = -2404;
pub const SENGOO_LUA54_ERR_COMPILE: i32 = -2405;
pub const SENGOO_LUA54_ERR_RUNTIME: i32 = -2406;
pub const SENGOO_LUA54_ERR_TYPE: i32 = -2407;
pub const SENGOO_LUA54_ERR_INTERNAL: i32 = -2499;

const LUA_OK: i32 = 0;
const LUA_TNIL: i32 = 0;

type LuaLNewState = unsafe extern "C" fn() -> *mut c_void;
type LuaLOpenLibs = unsafe extern "C" fn(*mut c_void);
type LuaClose = unsafe extern "C" fn(*mut c_void);
type LuaLLoadString = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type LuaPCallK =
    unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, isize, *const c_void) -> c_int;
type LuaPCall = unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int) -> c_int;
type LuaGetGlobal = unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int;
type LuaPushInteger = unsafe extern "C" fn(*mut c_void, i64);
type LuaToIntegerX = unsafe extern "C" fn(*mut c_void, c_int, *mut c_int) -> i64;
type LuaToInteger = unsafe extern "C" fn(*mut c_void, c_int) -> i64;
type LuaSetTop = unsafe extern "C" fn(*mut c_void, c_int);
type LuaToLString = unsafe extern "C" fn(*mut c_void, c_int, *mut usize) -> *const c_char;

#[derive(Clone, Debug)]
struct Lua54ErrorState {
    code: i32,
    message: String,
}

impl Default for Lua54ErrorState {
    fn default() -> Self {
        Self {
            code: SENGOO_LUA54_STATUS_OK,
            message: String::new(),
        }
    }
}

#[derive(Clone, Copy)]
enum LuaPCallFn {
    K(LuaPCallK),
    Plain(LuaPCall),
}

impl LuaPCallFn {
    unsafe fn call(self, lua_state: *mut c_void, nargs: i32, nresults: i32, errfunc: i32) -> i32 {
        match self {
            LuaPCallFn::K(f) => unsafe {
                f(lua_state, nargs, nresults, errfunc, 0, std::ptr::null())
            },
            LuaPCallFn::Plain(f) => unsafe { f(lua_state, nargs, nresults, errfunc) },
        }
    }
}

#[derive(Clone, Copy)]
enum LuaToIntegerFn {
    X(LuaToIntegerX),
    Plain(LuaToInteger),
}

impl LuaToIntegerFn {
    unsafe fn call(self, lua_state: *mut c_void, idx: i32, isnum_out: *mut c_int) -> i64 {
        match self {
            LuaToIntegerFn::X(f) => unsafe { f(lua_state, idx, isnum_out) },
            LuaToIntegerFn::Plain(f) => unsafe {
                if !isnum_out.is_null() {
                    *isnum_out = 1;
                }
                f(lua_state, idx)
            },
        }
    }
}

#[derive(Clone, Copy)]
struct Lua54Api {
    new_state: LuaLNewState,
    open_libs: LuaLOpenLibs,
    close: LuaClose,
    load_string: LuaLLoadString,
    pcall: LuaPCallFn,
    get_global: LuaGetGlobal,
    push_integer: LuaPushInteger,
    to_integer: LuaToIntegerFn,
    set_top: LuaSetTop,
    to_lstring: LuaToLString,
}

struct Lua54Instance {
    lib_handle: usize,
    lua_state: usize,
    api: Lua54Api,
}

static NEXT_LUA54_HANDLE: AtomicU64 = AtomicU64::new(1);
static LUA54_INSTANCES: OnceLock<Mutex<std::collections::HashMap<u64, Lua54Instance>>> =
    OnceLock::new();
static LUA54_LAST_ERROR: OnceLock<Mutex<Lua54ErrorState>> = OnceLock::new();

fn lua54_instances() -> &'static Mutex<std::collections::HashMap<u64, Lua54Instance>> {
    LUA54_INSTANCES.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn lua54_last_error() -> &'static Mutex<Lua54ErrorState> {
    LUA54_LAST_ERROR.get_or_init(|| Mutex::new(Lua54ErrorState::default()))
}

fn next_handle() -> u64 {
    NEXT_LUA54_HANDLE.fetch_add(1, Ordering::Relaxed)
}

fn clear_error() {
    if let Ok(mut state) = lua54_last_error().lock() {
        state.code = SENGOO_LUA54_STATUS_OK;
        state.message.clear();
    }
}

fn set_error(code: i32, message: impl Into<String>) -> i32 {
    if let Ok(mut state) = lua54_last_error().lock() {
        state.code = code;
        state.message = message.into();
    }
    code
}

fn copy_bytes_to_buffer(bytes: &[u8], buffer: *mut u8, capacity: usize) -> i64 {
    if buffer.is_null() {
        return set_error(SENGOO_LUA54_ERR_INVALID_ARGUMENT, "null output buffer") as i64;
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
            SENGOO_LUA54_ERR_INVALID_ARGUMENT,
            "null C string pointer",
        ));
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
            if len > 512 * 1024 {
                return Err(set_error(
                    SENGOO_LUA54_ERR_INVALID_ARGUMENT,
                    "C string too long",
                ));
            }
        }
        let bytes = std::slice::from_raw_parts(ptr, len);
        std::str::from_utf8(bytes)
            .map(|s| s.to_string())
            .map_err(|_| set_error(SENGOO_LUA54_ERR_INVALID_ARGUMENT, "invalid UTF-8 string"))
    }
}

unsafe fn load_symbol<T>(lib_handle: *mut c_void, name: &str) -> Result<T, i32>
where
    T: Copy,
{
    let raw = unsafe { super::native_loader::get(lib_handle, name) }.map_err(|reason| {
        set_error(
            SENGOO_LUA54_ERR_SYMBOL_LOAD,
            format!("failed to load Lua symbol '{name}': {reason}"),
        )
    })?;
    Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&raw) })
}

unsafe fn load_lua54_api(lib_handle: *mut c_void) -> Result<Lua54Api, i32> {
    let new_state: LuaLNewState = unsafe { load_symbol(lib_handle, "luaL_newstate") }?;
    let open_libs: LuaLOpenLibs = unsafe { load_symbol(lib_handle, "luaL_openlibs") }?;
    let close: LuaClose = unsafe { load_symbol(lib_handle, "lua_close") }?;
    let load_string: LuaLLoadString = unsafe { load_symbol(lib_handle, "luaL_loadstring") }?;
    let pcall = match unsafe { load_symbol::<LuaPCallK>(lib_handle, "lua_pcallk") } {
        Ok(f) => LuaPCallFn::K(f),
        Err(_) => LuaPCallFn::Plain(unsafe { load_symbol(lib_handle, "lua_pcall") }?),
    };
    let get_global: LuaGetGlobal = unsafe { load_symbol(lib_handle, "lua_getglobal") }?;
    let push_integer: LuaPushInteger = unsafe { load_symbol(lib_handle, "lua_pushinteger") }?;
    let to_integer = match unsafe { load_symbol::<LuaToIntegerX>(lib_handle, "lua_tointegerx") } {
        Ok(f) => LuaToIntegerFn::X(f),
        Err(_) => LuaToIntegerFn::Plain(unsafe { load_symbol(lib_handle, "lua_tointeger") }?),
    };
    let set_top: LuaSetTop = unsafe { load_symbol(lib_handle, "lua_settop") }?;
    let to_lstring: LuaToLString = unsafe { load_symbol(lib_handle, "lua_tolstring") }?;
    Ok(Lua54Api {
        new_state,
        open_libs,
        close,
        load_string,
        pcall,
        get_global,
        push_integer,
        to_integer,
        set_top,
        to_lstring,
    })
}

#[cfg(target_os = "windows")]
fn default_lua54_library_candidates() -> &'static [&'static str] {
    &["lua54.dll", "lua5.4.dll"]
}

#[cfg(target_os = "linux")]
fn default_lua54_library_candidates() -> &'static [&'static str] {
    &["liblua5.4.so", "liblua54.so", "liblua.so.5.4"]
}

#[cfg(target_os = "macos")]
fn default_lua54_library_candidates() -> &'static [&'static str] {
    &["liblua5.4.dylib", "liblua54.dylib", "liblua.dylib"]
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn default_lua54_library_candidates() -> &'static [&'static str] {
    &["lua54"]
}

unsafe fn open_lua_library(path: Option<&str>) -> Result<*mut c_void, i32> {
    let mut attempts = Vec::new();

    if let Some(path) = path {
        attempts.push(path.to_string());
    } else {
        attempts.extend(
            default_lua54_library_candidates()
                .iter()
                .map(|item| item.to_string()),
        );
    }

    let mut last_reason = String::new();
    for candidate in attempts {
        match unsafe { super::native_loader::open(Path::new(&candidate)) } {
            Ok(handle) => return Ok(handle),
            Err(reason) => {
                last_reason = format!("{} => {}", candidate, reason);
            }
        }
    }

    Err(set_error(
        SENGOO_LUA54_ERR_LIBRARY_LOAD,
        format!("failed to open Lua 5.4 library ({last_reason})"),
    ))
}

unsafe fn lua_error_message(lua_state: *mut c_void, api: Lua54Api) -> String {
    let mut len = 0usize;
    let ptr = unsafe { (api.to_lstring)(lua_state, -1, &mut len as *mut usize) };
    if ptr.is_null() {
        return "unknown lua error".to_string();
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
    String::from_utf8_lossy(bytes).to_string()
}

fn with_lua54_instance_mut<F>(handle: u64, f: F) -> i32
where
    F: FnOnce(&mut Lua54Instance) -> i32,
{
    let mut table = match lua54_instances().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_LUA54_ERR_INTERNAL, "lua54 instance table poisoned"),
    };
    let Some(instance) = table.get_mut(&handle) else {
        return set_error(
            SENGOO_LUA54_ERR_INVALID_HANDLE,
            format!("lua54 handle {handle} not found"),
        );
    };
    f(instance)
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_last_error_code() -> i32 {
    lua54_last_error()
        .lock()
        .map(|state| state.code)
        .unwrap_or(SENGOO_LUA54_ERR_INTERNAL)
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_last_error_len() -> i64 {
    lua54_last_error()
        .lock()
        .map(|state| state.message.len() as i64)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_last_error_copy(buffer: *mut u8, capacity: usize) -> i64 {
    let message = lua54_last_error()
        .lock()
        .map(|state| state.message.clone())
        .unwrap_or_default();
    copy_bytes_to_buffer(message.as_bytes(), buffer, capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_last_error_clear() -> i32 {
    clear_error();
    SENGOO_LUA54_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_open(path: *const u8) -> u64 {
    clear_error();
    let requested_path = if path.is_null() {
        None
    } else {
        let value = match parse_c_string(path) {
            Ok(value) => value,
            Err(_) => return 0,
        };
        if value.trim().is_empty() {
            None
        } else {
            Some(value)
        }
    };

    let lib_handle = match unsafe { open_lua_library(requested_path.as_deref()) } {
        Ok(handle) => handle,
        Err(_) => return 0,
    };
    let api = match unsafe { load_lua54_api(lib_handle) } {
        Ok(api) => api,
        Err(_) => {
            let _ = unsafe { super::native_loader::close(lib_handle) };
            return 0;
        }
    };

    let lua_state = unsafe { (api.new_state)() };
    if lua_state.is_null() {
        let _ = unsafe { super::native_loader::close(lib_handle) };
        set_error(SENGOO_LUA54_ERR_RUNTIME, "luaL_newstate returned null");
        return 0;
    }
    unsafe {
        (api.open_libs)(lua_state);
    }

    let handle = next_handle();
    let instance = Lua54Instance {
        lib_handle: lib_handle as usize,
        lua_state: lua_state as usize,
        api,
    };
    let mut table = match lua54_instances().lock() {
        Ok(table) => table,
        Err(_) => {
            unsafe {
                (api.close)(lua_state);
                let _ = super::native_loader::close(lib_handle);
            }
            set_error(SENGOO_LUA54_ERR_INTERNAL, "lua54 instance table poisoned");
            return 0;
        }
    };
    table.insert(handle, instance);
    handle
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_close(handle: u64) -> i32 {
    clear_error();
    let instance = {
        let mut table = match lua54_instances().lock() {
            Ok(table) => table,
            Err(_) => return set_error(SENGOO_LUA54_ERR_INTERNAL, "lua54 instance table poisoned"),
        };
        match table.remove(&handle) {
            Some(instance) => instance,
            None => {
                return set_error(
                    SENGOO_LUA54_ERR_INVALID_HANDLE,
                    format!("lua54 handle {handle} not found"),
                );
            }
        }
    };

    unsafe {
        (instance.api.close)(instance.lua_state as *mut c_void);
        match super::native_loader::close(instance.lib_handle as *mut c_void) {
            Ok(()) => SENGOO_LUA54_STATUS_OK,
            Err(reason) => set_error(
                SENGOO_LUA54_ERR_LIBRARY_LOAD,
                format!("failed to close Lua library: {reason}"),
            ),
        }
    }
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_exec(handle: u64, chunk: *const u8) -> i32 {
    clear_error();
    let chunk = match parse_c_string(chunk) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let c_chunk = match CString::new(chunk) {
        Ok(value) => value,
        Err(_) => {
            return set_error(
                SENGOO_LUA54_ERR_INVALID_ARGUMENT,
                "Lua chunk contains interior NUL byte",
            )
        }
    };

    with_lua54_instance_mut(handle, |instance| {
        let lua_state = instance.lua_state as *mut c_void;
        let api = instance.api;
        let compile_rc = unsafe { (api.load_string)(lua_state, c_chunk.as_ptr()) };
        if compile_rc != LUA_OK {
            let message = unsafe { lua_error_message(lua_state, api) };
            unsafe {
                (api.set_top)(lua_state, 0);
            }
            return set_error(SENGOO_LUA54_ERR_COMPILE, message);
        }
        let run_rc = unsafe { api.pcall.call(lua_state, 0, 0, 0) };
        if run_rc != LUA_OK {
            let message = unsafe { lua_error_message(lua_state, api) };
            unsafe {
                (api.set_top)(lua_state, 0);
            }
            return set_error(SENGOO_LUA54_ERR_RUNTIME, message);
        }
        unsafe {
            (api.set_top)(lua_state, 0);
        }
        SENGOO_LUA54_STATUS_OK
    })
}

#[no_mangle]
pub extern "C" fn sengoo_lua54_call_i64(
    handle: u64,
    func_name: *const u8,
    argc: usize,
    argv: *const i64,
    out_value: *mut i64,
) -> i32 {
    clear_error();
    if out_value.is_null() {
        return set_error(
            SENGOO_LUA54_ERR_INVALID_ARGUMENT,
            "out_value pointer is null",
        );
    }
    let func_name = match parse_c_string(func_name) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let c_func_name = match CString::new(func_name) {
        Ok(value) => value,
        Err(_) => {
            return set_error(
                SENGOO_LUA54_ERR_INVALID_ARGUMENT,
                "function name contains interior NUL byte",
            )
        }
    };
    let args = if argc == 0 {
        Vec::new()
    } else if argv.is_null() {
        return set_error(
            SENGOO_LUA54_ERR_INVALID_ARGUMENT,
            "argv pointer is null while argc > 0",
        );
    } else {
        let slice = unsafe { std::slice::from_raw_parts(argv, argc) };
        slice.to_vec()
    };

    with_lua54_instance_mut(handle, |instance| {
        let lua_state = instance.lua_state as *mut c_void;
        let api = instance.api;
        let ty = unsafe { (api.get_global)(lua_state, c_func_name.as_ptr()) };
        if ty == LUA_TNIL {
            unsafe {
                (api.set_top)(lua_state, 0);
            }
            return set_error(
                SENGOO_LUA54_ERR_RUNTIME,
                "Lua function not found in global scope",
            );
        }
        for value in &args {
            unsafe {
                (api.push_integer)(lua_state, *value);
            }
        }
        let run_rc = unsafe { api.pcall.call(lua_state, args.len() as i32, 1, 0) };
        if run_rc != LUA_OK {
            let message = unsafe { lua_error_message(lua_state, api) };
            unsafe {
                (api.set_top)(lua_state, 0);
            }
            return set_error(SENGOO_LUA54_ERR_RUNTIME, message);
        }
        let mut is_num: c_int = 0;
        let value = unsafe {
            api.to_integer
                .call(lua_state, -1, &mut is_num as *mut c_int)
        };
        if is_num == 0 {
            unsafe {
                (api.set_top)(lua_state, 0);
            }
            return set_error(SENGOO_LUA54_ERR_TYPE, "Lua return value is not integer");
        }
        unsafe {
            *out_value = value;
            (api.set_top)(lua_state, 0);
        }
        SENGOO_LUA54_STATUS_OK
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn lua54_test_guard() -> MutexGuard<'static, ()> {
        static LUA54_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LUA54_TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn c_str(s: &str) -> Vec<u8> {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    #[test]
    fn lua54_open_missing_library_reports_error() {
        let _guard = lua54_test_guard();
        let path = c_str("missing-lua54-runtime-bridge.dll");
        let handle = sengoo_lua54_open(path.as_ptr());
        assert_eq!(handle, 0);
        assert_eq!(
            sengoo_lua54_last_error_code(),
            SENGOO_LUA54_ERR_LIBRARY_LOAD
        );
        assert!(sengoo_lua54_last_error_len() > 0);
    }

    #[test]
    fn lua54_exec_and_call_i64_when_library_available() {
        let _guard = lua54_test_guard();
        let handle = sengoo_lua54_open(std::ptr::null());
        if handle == 0 {
            // CI and dev environments may not ship Lua 5.4 dynamic library.
            return;
        }

        let chunk = c_str("function add(a,b) return a + b end");
        assert_eq!(
            sengoo_lua54_exec(handle, chunk.as_ptr()),
            SENGOO_LUA54_STATUS_OK
        );

        let name = c_str("add");
        let args = [2_i64, 5_i64];
        let mut out = 0_i64;
        assert_eq!(
            sengoo_lua54_call_i64(
                handle,
                name.as_ptr(),
                args.len(),
                args.as_ptr(),
                &mut out as *mut i64
            ),
            SENGOO_LUA54_STATUS_OK
        );
        assert_eq!(out, 7);

        assert_eq!(sengoo_lua54_close(handle), SENGOO_LUA54_STATUS_OK);
    }

    #[test]
    fn lua54_call_missing_function_reports_runtime_error() {
        let _guard = lua54_test_guard();
        let handle = sengoo_lua54_open(std::ptr::null());
        if handle == 0 {
            return;
        }

        let name = c_str("missing_func");
        let mut out = 0_i64;
        let rc = sengoo_lua54_call_i64(handle, name.as_ptr(), 0, std::ptr::null(), &mut out);
        assert_eq!(rc, SENGOO_LUA54_ERR_RUNTIME);
        assert!(sengoo_lua54_last_error_len() > 0);
        assert_eq!(sengoo_lua54_close(handle), SENGOO_LUA54_STATUS_OK);
    }
}
