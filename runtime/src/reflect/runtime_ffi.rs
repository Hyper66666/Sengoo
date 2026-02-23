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
static FFI_LAST_ERROR: OnceLock<Mutex<FfiErrorState>> = OnceLock::new();

fn c_libs() -> &'static Mutex<HashMap<u64, CApiLibrary>> {
    C_LIBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lua_states() -> &'static Mutex<HashMap<u64, LuaState>> {
    LUA_STATES.get_or_init(|| Mutex::new(HashMap::new()))
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
        _ => Err(set_error(
            SENGOO_FFI_ERR_SYMBOL_NOT_FOUND,
            format!("builtin symbol '{symbol}' not found"),
        )),
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

    let value = match &lib.kind {
        CLibKind::Builtin => ffi_invoke_builtin(&symbol, &args),
        CLibKind::Native(native_handle) => unsafe {
            ffi_invoke_native_i64(*native_handle as *mut c_void, &symbol, &args)
        },
    };
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
}
