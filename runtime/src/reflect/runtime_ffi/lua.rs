use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::{
    clear_error, ffi_read_i64_args, next_handle, parse_c_string, set_error,
    SENGOO_FFI_ERR_CALL_FAILED, SENGOO_FFI_ERR_INTERNAL, SENGOO_FFI_ERR_INVALID_ARGUMENT,
    SENGOO_FFI_ERR_INVALID_HANDLE, SENGOO_FFI_ERR_PARSE, SENGOO_FFI_ERR_SYMBOL_NOT_FOUND,
    SENGOO_FFI_STATUS_OK,
};

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

static LUA_STATES: OnceLock<Mutex<HashMap<u64, LuaState>>> = OnceLock::new();

fn lua_states() -> &'static Mutex<HashMap<u64, LuaState>> {
    LUA_STATES.get_or_init(|| Mutex::new(HashMap::new()))
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
