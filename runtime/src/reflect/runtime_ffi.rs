use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

pub const SENGOO_FFI_STATUS_OK: i32 = 0;
pub const SENGOO_FFI_ERR_INVALID_ARGUMENT: i32 = -2001;
pub const SENGOO_FFI_ERR_INVALID_HANDLE: i32 = -2002;
pub const SENGOO_FFI_ERR_SYMBOL_NOT_FOUND: i32 = -2003;
pub const SENGOO_FFI_ERR_CALL_FAILED: i32 = -2004;
pub const SENGOO_FFI_ERR_PARSE: i32 = -2005;
pub const SENGOO_FFI_ERR_BUFFER: i32 = -2006;
pub const SENGOO_FFI_ERR_INTERNAL: i32 = -2099;

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

#[derive(Clone, Debug)]
struct FfiObject {
    lib_handle: u64,
    raw_ptr: i64,
    destructor_symbol: Option<String>,
}

#[derive(Clone, Debug)]
struct FfiCallbackBinding {
    lib_handle: u64,
    symbol: String,
    arity: usize,
}

#[derive(Clone, Debug, Default)]
struct FfiBuffer {
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
enum LuaTerm {
    Literal(i64),
    Param(String),
}

#[derive(Clone, Debug)]
enum LuaExpr {
    Single(LuaTerm),
    Binary {
        lhs: LuaTerm,
        op: char,
        rhs: LuaTerm,
    },
}

#[derive(Clone, Debug)]
struct LuaFunction {
    params: Vec<String>,
    expr: LuaExpr,
}

#[derive(Clone, Debug, Default)]
struct LuaState {
    loaded_chunk: Option<Vec<(String, LuaFunction)>>,
    functions: HashMap<String, LuaFunction>,
}

static NEXT_FFI_HANDLE: AtomicU64 = AtomicU64::new(1);
static C_LIBS: OnceLock<Mutex<HashMap<u64, CApiLibrary>>> = OnceLock::new();
static LUA_STATES: OnceLock<Mutex<HashMap<u64, LuaState>>> = OnceLock::new();
static FFI_OBJECTS: OnceLock<Mutex<HashMap<u64, FfiObject>>> = OnceLock::new();
static NEXT_FFI_CALLBACK_ID: AtomicU64 = AtomicU64::new(1);
static FFI_CALLBACKS: OnceLock<Mutex<HashMap<u64, FfiCallbackBinding>>> = OnceLock::new();
static FFI_BUFFERS: OnceLock<Mutex<HashMap<u64, FfiBuffer>>> = OnceLock::new();
static FFI_LAST_ERROR: OnceLock<Mutex<FfiErrorState>> = OnceLock::new();

fn c_libs() -> &'static Mutex<HashMap<u64, CApiLibrary>> {
    C_LIBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lua_states() -> &'static Mutex<HashMap<u64, LuaState>> {
    LUA_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ffi_objects() -> &'static Mutex<HashMap<u64, FfiObject>> {
    FFI_OBJECTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ffi_callbacks() -> &'static Mutex<HashMap<u64, FfiCallbackBinding>> {
    FFI_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ffi_buffers() -> &'static Mutex<HashMap<u64, FfiBuffer>> {
    FFI_BUFFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ffi_last_error() -> &'static Mutex<FfiErrorState> {
    FFI_LAST_ERROR.get_or_init(|| Mutex::new(FfiErrorState::default()))
}

fn next_handle() -> u64 {
    NEXT_FFI_HANDLE.fetch_add(1, Ordering::Relaxed)
}

fn next_callback_id() -> u64 {
    NEXT_FFI_CALLBACK_ID.fetch_add(1, Ordering::Relaxed)
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

fn lua_parse_term(raw: &str) -> LuaTerm {
    let token = raw.trim();
    if let Ok(value) = token.parse::<i64>() {
        LuaTerm::Literal(value)
    } else {
        LuaTerm::Param(token.to_string())
    }
}

fn lua_parse_expr(raw: &str) -> Result<LuaExpr, i32> {
    let expr = raw.trim().trim_end_matches(';').trim();
    let parts = expr.split_whitespace().collect::<Vec<_>>();
    if parts.len() == 1 {
        return Ok(LuaExpr::Single(lua_parse_term(parts[0])));
    }
    if parts.len() == 3 {
        let op_token = parts[1].trim();
        if op_token.len() != 1 {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                format!("unsupported Lua operator '{op_token}'"),
            ));
        }
        let op = op_token
            .chars()
            .next()
            .ok_or_else(|| set_error(SENGOO_FFI_ERR_PARSE, "Lua expression missing operator"))?;
        if !matches!(op, '+' | '-' | '*' | '/') {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                format!("unsupported Lua operator '{op}'"),
            ));
        }
        return Ok(LuaExpr::Binary {
            lhs: lua_parse_term(parts[0]),
            op,
            rhs: lua_parse_term(parts[2]),
        });
    }
    Err(set_error(
        SENGOO_FFI_ERR_PARSE,
        format!("unsupported Lua expression '{expr}'"),
    ))
}

fn lua_parse_chunk(chunk: &str) -> Result<Vec<(String, LuaFunction)>, i32> {
    let mut source = chunk;
    let mut functions = Vec::<(String, LuaFunction)>::new();

    loop {
        let Some(function_idx) = source.find("function ") else {
            break;
        };
        source = &source[function_idx + "function ".len()..];

        let Some(end_idx) = source.find("end") else {
            return Err(set_error(SENGOO_FFI_ERR_PARSE, "Lua chunk missing 'end'"));
        };
        let block = source[..end_idx].trim();
        source = &source[end_idx + "end".len()..];

        let Some((header, body)) = block.split_once("return") else {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                "Lua function missing return statement",
            ));
        };
        let header = header.trim();
        let body = body.trim();

        let Some(open_idx) = header.find('(') else {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                "Lua function header missing '('",
            ));
        };
        let Some(close_idx) = header.rfind(')') else {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                "Lua function header missing ')'",
            ));
        };
        if close_idx <= open_idx {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                "Lua function header has invalid parameter section",
            ));
        }

        let name = header[..open_idx].trim().to_string();
        if name.is_empty() {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                "Lua function name is empty",
            ));
        }

        let params_raw = &header[open_idx + 1..close_idx];
        let params = if params_raw.trim().is_empty() {
            Vec::new()
        } else {
            params_raw
                .split(',')
                .map(|item| item.trim().to_string())
                .collect::<Vec<_>>()
        };
        if params.iter().any(|param| param.is_empty()) {
            return Err(set_error(
                SENGOO_FFI_ERR_PARSE,
                format!("Lua function '{name}' contains empty parameter name"),
            ));
        }

        let expr = lua_parse_expr(body)?;
        functions.push((name, LuaFunction { params, expr }));
    }

    if functions.is_empty() {
        return Err(set_error(
            SENGOO_FFI_ERR_PARSE,
            "Lua chunk does not define any function",
        ));
    }
    Ok(functions)
}

fn lua_eval_term(term: &LuaTerm, env: &HashMap<String, i64>) -> Result<i64, i32> {
    match term {
        LuaTerm::Literal(v) => Ok(*v),
        LuaTerm::Param(name) => env.get(name).copied().ok_or_else(|| {
            set_error(
                SENGOO_FFI_ERR_CALL_FAILED,
                format!("Lua call missing parameter '{name}'"),
            )
        }),
    }
}

fn lua_eval_expr(expr: &LuaExpr, env: &HashMap<String, i64>) -> Result<i64, i32> {
    match expr {
        LuaExpr::Single(term) => lua_eval_term(term, env),
        LuaExpr::Binary { lhs, op, rhs } => {
            let lhs = lua_eval_term(lhs, env)?;
            let rhs = lua_eval_term(rhs, env)?;
            match op {
                '+' => Ok(lhs + rhs),
                '-' => Ok(lhs - rhs),
                '*' => Ok(lhs * rhs),
                '/' => {
                    if rhs == 0 {
                        Err(set_error(
                            SENGOO_FFI_ERR_CALL_FAILED,
                            "Lua division by zero",
                        ))
                    } else {
                        Ok(lhs / rhs)
                    }
                }
                other => Err(set_error(
                    SENGOO_FFI_ERR_CALL_FAILED,
                    format!("unsupported Lua operator '{other}'"),
                )),
            }
        }
    }
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

#[no_mangle]
pub extern "C" fn sengoo_ffi_buffer_new(capacity: usize) -> u64 {
    clear_error();
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

#[no_mangle]
pub extern "C" fn sengoo_lua_open() -> u64 {
    clear_error();
    let handle = next_handle();
    let mut states = match lua_states().lock() {
        Ok(table) => table,
        Err(_) => {
            set_error(SENGOO_FFI_ERR_INTERNAL, "lua state table poisoned");
            return 0;
        }
    };
    states.insert(handle, LuaState::default());
    handle
}

#[no_mangle]
pub extern "C" fn sengoo_lua_close(handle: u64) -> i32 {
    clear_error();
    let mut states = match lua_states().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "lua state table poisoned"),
    };
    if states.remove(&handle).is_some() {
        SENGOO_FFI_STATUS_OK
    } else {
        set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("lua state handle {handle} not found"),
        )
    }
}

#[no_mangle]
pub extern "C" fn sengoo_lua_load(handle: u64, chunk: *const u8) -> i32 {
    clear_error();
    let chunk = match parse_c_string(chunk) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let parsed = match lua_parse_chunk(&chunk) {
        Ok(defs) => defs,
        Err(code) => return code,
    };

    let mut states = match lua_states().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "lua state table poisoned"),
    };
    let Some(state) = states.get_mut(&handle) else {
        return set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("lua state handle {handle} not found"),
        );
    };
    state.loaded_chunk = Some(parsed);
    SENGOO_FFI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_lua_exec(handle: u64, chunk: *const u8) -> i32 {
    clear_error();
    let mut states = match lua_states().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "lua state table poisoned"),
    };
    let Some(state) = states.get_mut(&handle) else {
        return set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("lua state handle {handle} not found"),
        );
    };

    let defs = if chunk.is_null() {
        match &state.loaded_chunk {
            Some(defs) => defs.clone(),
            None => {
                return set_error(
                    SENGOO_FFI_ERR_INVALID_ARGUMENT,
                    "no loaded lua chunk to exec",
                )
            }
        }
    } else {
        let chunk = match parse_c_string(chunk) {
            Ok(value) => value,
            Err(code) => return code,
        };
        match lua_parse_chunk(&chunk) {
            Ok(defs) => defs,
            Err(code) => return code,
        }
    };

    for (name, function) in defs {
        state.functions.insert(name, function);
    }
    SENGOO_FFI_STATUS_OK
}

#[no_mangle]
pub extern "C" fn sengoo_lua_call_i64(
    handle: u64,
    func_name: *const u8,
    argc: usize,
    argv: *const i64,
    out_value: *mut i64,
) -> i32 {
    clear_error();
    if out_value.is_null() {
        return set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "out_value pointer is null");
    }
    let func_name = match parse_c_string(func_name) {
        Ok(value) => value,
        Err(code) => return code,
    };
    let args = match ffi_read_i64_args(argc, argv) {
        Ok(args) => args,
        Err(code) => return code,
    };

    let states = match lua_states().lock() {
        Ok(table) => table,
        Err(_) => return set_error(SENGOO_FFI_ERR_INTERNAL, "lua state table poisoned"),
    };
    let Some(state) = states.get(&handle) else {
        return set_error(
            SENGOO_FFI_ERR_INVALID_HANDLE,
            format!("lua state handle {handle} not found"),
        );
    };
    let Some(function) = state.functions.get(&func_name) else {
        return set_error(
            SENGOO_FFI_ERR_SYMBOL_NOT_FOUND,
            format!("lua function '{func_name}' not found"),
        );
    };
    if function.params.len() != args.len() {
        return set_error(
            SENGOO_FFI_ERR_INVALID_ARGUMENT,
            format!(
                "lua function '{}' expects {} args, got {}",
                func_name,
                function.params.len(),
                args.len()
            ),
        );
    }

    let mut env = HashMap::new();
    for (idx, name) in function.params.iter().enumerate() {
        env.insert(name.clone(), args[idx]);
    }
    let value = match lua_eval_expr(&function.expr, &env) {
        Ok(value) => value,
        Err(code) => return code,
    };
    unsafe {
        *out_value = value;
    }
    SENGOO_FFI_STATUS_OK
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
}
