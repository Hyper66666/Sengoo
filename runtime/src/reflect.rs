//! Non-invasive reflection support.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
#[cfg(unix)]
use std::ffi::{c_char, c_int, CStr};
use std::ffi::{c_void, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use thiserror::Error;

mod runtime_db;
mod runtime_ffi;

pub const REFLECTION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectionSymbolMetadata {
    pub symbol: String,
    pub signature: String,
    #[serde(default)]
    pub native_symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectionModuleMetadata {
    pub module_id: String,
    #[serde(default)]
    pub symbols: Vec<ReflectionSymbolMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectionMetadata {
    #[serde(default = "default_reflection_schema_version")]
    pub schema_version: u32,
    pub compiler_version: String,
    #[serde(default)]
    pub compatible_compiler_versions: Vec<String>,
    pub root_module: String,
    #[serde(default)]
    pub modules: Vec<ReflectionModuleMetadata>,
}

fn default_reflection_schema_version() -> u32 {
    REFLECTION_SCHEMA_VERSION
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReflectValue {
    I64(i64),
    F64(f64),
    Bool(bool),
}

impl ReflectValue {
    fn type_name(&self) -> &'static str {
        match self {
            ReflectValue::I64(_) => "i64",
            ReflectValue::F64(_) => "f64",
            ReflectValue::Bool(_) => "bool",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PrimitiveType {
    I64,
    F64,
    Bool,
    Unit,
    Unsupported(String),
}

impl PrimitiveType {
    fn from_name(name: &str) -> Self {
        match name.trim() {
            "i64" => PrimitiveType::I64,
            "f64" => PrimitiveType::F64,
            "bool" => PrimitiveType::Bool,
            "unit" => PrimitiveType::Unit,
            other => PrimitiveType::Unsupported(other.to_string()),
        }
    }

    fn label(&self) -> String {
        match self {
            PrimitiveType::I64 => "i64".to_string(),
            PrimitiveType::F64 => "f64".to_string(),
            PrimitiveType::Bool => "bool".to_string(),
            PrimitiveType::Unit => "unit".to_string(),
            PrimitiveType::Unsupported(raw) => raw.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SignatureContract {
    params: Vec<PrimitiveType>,
    ret: PrimitiveType,
}

#[derive(Debug, Clone)]
struct SymbolEntry {
    native_symbol: Option<String>,
    contract: SignatureContract,
}

struct ReflectionRegistry {
    symbols: HashMap<String, SymbolEntry>,
    module_symbols: HashMap<String, Vec<ReflectionSymbolMetadata>>,
    module_symbol_index: HashMap<String, HashSet<String>>,
    handlers: RwLock<HashMap<String, ReflectHandler>>,
}

pub type ReflectHandler =
    Arc<dyn Fn(&[ReflectValue]) -> std::result::Result<ReflectValue, String> + Send + Sync>;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReflectionLoadError {
    #[error("failed to read reflection metadata: {0}")]
    Io(String),
    #[error("failed to parse reflection metadata: {0}")]
    Parse(String),
    #[error("reflection metadata schema mismatch: expected {expected} got {found}")]
    SchemaMismatch { expected: u32, found: u32 },
    #[error(
        "reflection metadata compiler compatibility mismatch: metadata={compiler_version}, runtime={runtime_version}"
    )]
    IncompatibleCompilerVersion {
        compiler_version: String,
        runtime_version: String,
    },
    #[error("reflection metadata invalid: {0}")]
    InvalidMetadata(String),
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ReflectInvokeError {
    #[error("reflection metadata load failed: {0}")]
    Load(#[from] ReflectionLoadError),
    #[error("module not found: {module}")]
    ModuleNotFound { module: String },
    #[error("symbol not found: {module}::{symbol}")]
    SymbolNotFound { module: String, symbol: String },
    #[error("symbol is not invocable: {module}::{symbol}")]
    SymbolNotInvocable { module: String, symbol: String },
    #[error("arity mismatch: expected {expected}, actual {actual}")]
    ArityMismatch { expected: usize, actual: usize },
    #[error("type mismatch at arg[{index}]: expected {expected}, actual {actual}")]
    TypeMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
    #[error("return type mismatch: expected {expected}, actual {actual}")]
    ReturnTypeMismatch { expected: String, actual: String },
    #[error("unsupported reflective signature for {module}::{symbol}: {reason}")]
    UnsupportedSignature {
        module: String,
        symbol: String,
        reason: String,
    },
    #[error("failed to load native reflection library {path}: {reason}")]
    NativeLibraryLoad { path: String, reason: String },
    #[error("failed to load native symbol {symbol} from {path}: {reason}")]
    NativeSymbolLoad {
        path: String,
        symbol: String,
        reason: String,
    },
    #[error("reflective invocation failed: {0}")]
    ExecutionFailed(String),
}

pub struct ReflectionRuntime {
    metadata_path: PathBuf,
    state: OnceLock<std::result::Result<Arc<ReflectionRegistry>, ReflectionLoadError>>,
}

impl ReflectionRuntime {
    pub fn new(metadata_path: impl Into<PathBuf>) -> Self {
        Self {
            metadata_path: metadata_path.into(),
            state: OnceLock::new(),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.state.get().is_some()
    }

    pub fn list_symbols(
        &self,
        module: &str,
    ) -> std::result::Result<Vec<ReflectionSymbolMetadata>, ReflectInvokeError> {
        let registry = self.registry()?;
        registry.module_symbols.get(module).cloned().ok_or_else(|| {
            ReflectInvokeError::ModuleNotFound {
                module: module.to_string(),
            }
        })
    }

    pub fn register_fn<F>(
        &self,
        module: &str,
        symbol: &str,
        handler: F,
    ) -> std::result::Result<(), ReflectInvokeError>
    where
        F: Fn(&[ReflectValue]) -> std::result::Result<ReflectValue, String> + Send + Sync + 'static,
    {
        self.register_handler(module, symbol, Arc::new(handler))
    }

    pub fn register_handler(
        &self,
        module: &str,
        symbol: &str,
        handler: ReflectHandler,
    ) -> std::result::Result<(), ReflectInvokeError> {
        let registry = self.registry()?;
        let symbol_key = resolve_symbol_key(&registry, module, symbol)?;
        let mut handlers = registry.handlers.write().map_err(|_| {
            ReflectInvokeError::ExecutionFailed("reflection handler registry poisoned".to_string())
        })?;
        handlers.insert(symbol_key, handler);
        Ok(())
    }

    pub fn register_i64_native_bindings_from_library(
        &self,
        library_path: impl AsRef<Path>,
    ) -> std::result::Result<usize, ReflectInvokeError> {
        let library_path = library_path.as_ref();
        let registry = self.registry()?;
        let library = unsafe { NativeLibrary::open(library_path) }.map_err(|reason| {
            ReflectInvokeError::NativeLibraryLoad {
                path: library_path.to_string_lossy().to_string(),
                reason,
            }
        })?;

        let mut pending = Vec::<(String, ReflectHandler)>::new();
        for (symbol_key, entry) in &registry.symbols {
            if !is_i64_signature(&entry.contract) {
                continue;
            }
            let native_symbol = entry.native_symbol.clone().unwrap_or_else(|| {
                symbol_key
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_string()
            });
            let native_fn = unsafe {
                load_i64_native_fn(
                    &library,
                    library_path,
                    &native_symbol,
                    entry.contract.params.len(),
                )?
            };
            let handler: ReflectHandler = Arc::new(move |args| unsafe { native_fn.invoke(args) });
            pending.push((symbol_key.clone(), handler));
        }

        let mut handlers = registry.handlers.write().map_err(|_| {
            ReflectInvokeError::ExecutionFailed("reflection handler registry poisoned".to_string())
        })?;
        let mut installed = 0usize;
        for (symbol, handler) in pending {
            handlers.insert(symbol, handler);
            installed += 1;
        }

        // Keep library loaded for the process lifetime.
        let _ = Box::leak(Box::new(library));
        Ok(installed)
    }

    pub fn call_i64(
        &self,
        module: &str,
        symbol: &str,
        args: &[ReflectValue],
    ) -> std::result::Result<i64, ReflectInvokeError> {
        let value = self.invoke(module, symbol, args, PrimitiveType::I64)?;
        match value {
            ReflectValue::I64(value) => Ok(value),
            other => Err(ReflectInvokeError::ReturnTypeMismatch {
                expected: "i64".to_string(),
                actual: other.type_name().to_string(),
            }),
        }
    }

    pub fn call_f64(
        &self,
        module: &str,
        symbol: &str,
        args: &[ReflectValue],
    ) -> std::result::Result<f64, ReflectInvokeError> {
        let value = self.invoke(module, symbol, args, PrimitiveType::F64)?;
        match value {
            ReflectValue::F64(value) => Ok(value),
            other => Err(ReflectInvokeError::ReturnTypeMismatch {
                expected: "f64".to_string(),
                actual: other.type_name().to_string(),
            }),
        }
    }

    pub fn call_bool(
        &self,
        module: &str,
        symbol: &str,
        args: &[ReflectValue],
    ) -> std::result::Result<bool, ReflectInvokeError> {
        let value = self.invoke(module, symbol, args, PrimitiveType::Bool)?;
        match value {
            ReflectValue::Bool(value) => Ok(value),
            other => Err(ReflectInvokeError::ReturnTypeMismatch {
                expected: "bool".to_string(),
                actual: other.type_name().to_string(),
            }),
        }
    }

    fn registry(&self) -> std::result::Result<Arc<ReflectionRegistry>, ReflectInvokeError> {
        match self
            .state
            .get_or_init(|| load_registry(&self.metadata_path).map(Arc::new))
        {
            Ok(registry) => Ok(Arc::clone(registry)),
            Err(err) => Err(ReflectInvokeError::Load(err.clone())),
        }
    }

    fn invoke(
        &self,
        module: &str,
        symbol: &str,
        args: &[ReflectValue],
        expected_ret: PrimitiveType,
    ) -> std::result::Result<ReflectValue, ReflectInvokeError> {
        let registry = self.registry()?;
        let symbol_key = resolve_symbol_key(&registry, module, symbol)?;
        let entry = registry.symbols.get(&symbol_key).ok_or_else(|| {
            ReflectInvokeError::SymbolNotFound {
                module: module.to_string(),
                symbol: symbol.to_string(),
            }
        })?;

        validate_contract(module, &symbol_key, &entry.contract, args, &expected_ret)?;

        let handler = registry
            .handlers
            .read()
            .map_err(|_| {
                ReflectInvokeError::ExecutionFailed(
                    "reflection handler registry poisoned".to_string(),
                )
            })?
            .get(&symbol_key)
            .cloned()
            .ok_or_else(|| ReflectInvokeError::SymbolNotInvocable {
                module: module.to_string(),
                symbol: symbol.to_string(),
            })?;

        let value = handler(args).map_err(ReflectInvokeError::ExecutionFailed)?;
        Ok(value)
    }
}

struct NativeLibrary {
    handle: *mut c_void,
}

impl NativeLibrary {
    unsafe fn open(path: &Path) -> std::result::Result<Self, String> {
        let handle = unsafe { native_loader::open(path)? };
        Ok(Self { handle })
    }

    unsafe fn get(&self, symbol: &str) -> std::result::Result<*mut c_void, String> {
        unsafe { native_loader::get(self.handle, symbol) }
    }
}

impl Drop for NativeLibrary {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        unsafe {
            let _ = native_loader::close(self.handle);
        }
    }
}

#[derive(Clone, Copy)]
enum I64NativeFn {
    Arity0(unsafe extern "C" fn() -> i64),
    Arity1(unsafe extern "C" fn(i64) -> i64),
    Arity2(unsafe extern "C" fn(i64, i64) -> i64),
    Arity3(unsafe extern "C" fn(i64, i64, i64) -> i64),
    Arity4(unsafe extern "C" fn(i64, i64, i64, i64) -> i64),
}

impl I64NativeFn {
    unsafe fn invoke(self, args: &[ReflectValue]) -> std::result::Result<ReflectValue, String> {
        let pick_i64 = |index: usize| -> std::result::Result<i64, String> {
            match args.get(index) {
                Some(ReflectValue::I64(value)) => Ok(*value),
                Some(other) => Err(format!(
                    "native i64 binding expected i64 at arg[{index}], got {}",
                    other.type_name()
                )),
                None => Err(format!("missing argument at index {index}")),
            }
        };
        let result = match self {
            I64NativeFn::Arity0(func) => func(),
            I64NativeFn::Arity1(func) => func(pick_i64(0)?),
            I64NativeFn::Arity2(func) => func(pick_i64(0)?, pick_i64(1)?),
            I64NativeFn::Arity3(func) => func(pick_i64(0)?, pick_i64(1)?, pick_i64(2)?),
            I64NativeFn::Arity4(func) => {
                func(pick_i64(0)?, pick_i64(1)?, pick_i64(2)?, pick_i64(3)?)
            }
        };
        Ok(ReflectValue::I64(result))
    }
}

fn is_i64_signature(contract: &SignatureContract) -> bool {
    if contract.ret != PrimitiveType::I64 {
        return false;
    }
    contract
        .params
        .iter()
        .all(|param| *param == PrimitiveType::I64)
}

unsafe fn load_i64_native_fn(
    library: &NativeLibrary,
    library_path: &Path,
    symbol: &str,
    arity: usize,
) -> std::result::Result<I64NativeFn, ReflectInvokeError> {
    let load_error = |reason: String| ReflectInvokeError::NativeSymbolLoad {
        path: library_path.to_string_lossy().to_string(),
        symbol: symbol.to_string(),
        reason,
    };

    let raw = unsafe { library.get(symbol) }.map_err(load_error)?;
    match arity {
        0 => Ok(I64NativeFn::Arity0(unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn() -> i64>(raw)
        })),
        1 => Ok(I64NativeFn::Arity1(unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64) -> i64>(raw)
        })),
        2 => Ok(I64NativeFn::Arity2(unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64, i64) -> i64>(raw)
        })),
        3 => Ok(I64NativeFn::Arity3(unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64, i64, i64) -> i64>(raw)
        })),
        4 => Ok(I64NativeFn::Arity4(unsafe {
            std::mem::transmute::<*mut c_void, unsafe extern "C" fn(i64, i64, i64, i64) -> i64>(raw)
        })),
        _ => Err(load_error(format!(
            "unsupported i64 native arity {}; supported: 0..=4",
            arity
        ))),
    }
}

fn resolve_symbol_key(
    registry: &ReflectionRegistry,
    module: &str,
    symbol: &str,
) -> std::result::Result<String, ReflectInvokeError> {
    let Some(module_symbols) = registry.module_symbol_index.get(module) else {
        return Err(ReflectInvokeError::ModuleNotFound {
            module: module.to_string(),
        });
    };

    let candidate = if symbol.contains("::") {
        symbol.to_string()
    } else {
        format!("{}::{}", module, symbol)
    };
    if module_symbols.contains(&candidate) {
        return Ok(candidate);
    }

    Err(ReflectInvokeError::SymbolNotFound {
        module: module.to_string(),
        symbol: symbol.to_string(),
    })
}

fn validate_contract(
    module: &str,
    symbol: &str,
    contract: &SignatureContract,
    args: &[ReflectValue],
    expected_ret: &PrimitiveType,
) -> std::result::Result<(), ReflectInvokeError> {
    if contract.params.len() != args.len() {
        return Err(ReflectInvokeError::ArityMismatch {
            expected: contract.params.len(),
            actual: args.len(),
        });
    }

    for (index, (expected, actual)) in contract.params.iter().zip(args.iter()).enumerate() {
        match expected {
            PrimitiveType::I64 => {
                if !matches!(actual, ReflectValue::I64(_)) {
                    return Err(ReflectInvokeError::TypeMismatch {
                        index,
                        expected: expected.label(),
                        actual: actual.type_name().to_string(),
                    });
                }
            }
            PrimitiveType::F64 => {
                if !matches!(actual, ReflectValue::F64(_)) {
                    return Err(ReflectInvokeError::TypeMismatch {
                        index,
                        expected: expected.label(),
                        actual: actual.type_name().to_string(),
                    });
                }
            }
            PrimitiveType::Bool => {
                if !matches!(actual, ReflectValue::Bool(_)) {
                    return Err(ReflectInvokeError::TypeMismatch {
                        index,
                        expected: expected.label(),
                        actual: actual.type_name().to_string(),
                    });
                }
            }
            PrimitiveType::Unit | PrimitiveType::Unsupported(_) => {
                return Err(ReflectInvokeError::UnsupportedSignature {
                    module: module.to_string(),
                    symbol: symbol.to_string(),
                    reason: format!("unsupported argument type {}", expected.label()),
                });
            }
        }
    }

    if contract.ret != *expected_ret {
        return Err(ReflectInvokeError::ReturnTypeMismatch {
            expected: expected_ret.label(),
            actual: contract.ret.label(),
        });
    }

    match contract.ret {
        PrimitiveType::I64 | PrimitiveType::F64 | PrimitiveType::Bool => Ok(()),
        PrimitiveType::Unit | PrimitiveType::Unsupported(_) => {
            Err(ReflectInvokeError::UnsupportedSignature {
                module: module.to_string(),
                symbol: symbol.to_string(),
                reason: format!("unsupported return type {}", contract.ret.label()),
            })
        }
    }
}

fn load_registry(path: &Path) -> std::result::Result<ReflectionRegistry, ReflectionLoadError> {
    let bytes = fs::read(path).map_err(|err| ReflectionLoadError::Io(err.to_string()))?;
    let metadata: ReflectionMetadata = serde_json::from_slice(&bytes)
        .map_err(|err| ReflectionLoadError::Parse(err.to_string()))?;
    validate_metadata(&metadata)?;

    let mut symbols = HashMap::<String, SymbolEntry>::new();
    let mut module_symbols = HashMap::<String, Vec<ReflectionSymbolMetadata>>::new();
    let mut module_symbol_index = HashMap::<String, HashSet<String>>::new();

    for module in &metadata.modules {
        let mut listed = Vec::new();
        let mut index = HashSet::new();
        for symbol in &module.symbols {
            let contract = parse_signature_contract(&symbol.signature)?;
            symbols.insert(
                symbol.symbol.clone(),
                SymbolEntry {
                    native_symbol: symbol.native_symbol.clone(),
                    contract,
                },
            );
            listed.push(symbol.clone());
            index.insert(symbol.symbol.clone());
        }
        listed.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        module_symbols.insert(module.module_id.clone(), listed);
        module_symbol_index.insert(module.module_id.clone(), index);
    }

    Ok(ReflectionRegistry {
        symbols,
        module_symbols,
        module_symbol_index,
        handlers: RwLock::new(HashMap::new()),
    })
}

fn validate_metadata(
    metadata: &ReflectionMetadata,
) -> std::result::Result<(), ReflectionLoadError> {
    if metadata.schema_version != REFLECTION_SCHEMA_VERSION {
        return Err(ReflectionLoadError::SchemaMismatch {
            expected: REFLECTION_SCHEMA_VERSION,
            found: metadata.schema_version,
        });
    }
    if metadata.compiler_version.trim().is_empty() {
        return Err(ReflectionLoadError::InvalidMetadata(
            "missing compiler_version".to_string(),
        ));
    }
    if metadata.compatible_compiler_versions.is_empty() {
        return Err(ReflectionLoadError::InvalidMetadata(
            "missing compatible_compiler_versions".to_string(),
        ));
    }
    let runtime_version = env!("CARGO_PKG_VERSION").to_string();
    if !metadata
        .compatible_compiler_versions
        .iter()
        .any(|version| version == &runtime_version)
    {
        return Err(ReflectionLoadError::IncompatibleCompilerVersion {
            compiler_version: metadata.compiler_version.clone(),
            runtime_version,
        });
    }

    let mut module_ids = HashSet::new();
    for module in &metadata.modules {
        if module.module_id.trim().is_empty() {
            return Err(ReflectionLoadError::InvalidMetadata(
                "module id cannot be empty".to_string(),
            ));
        }
        if !module_ids.insert(module.module_id.clone()) {
            return Err(ReflectionLoadError::InvalidMetadata(format!(
                "duplicate module id {}",
                module.module_id
            )));
        }
        let mut symbol_ids = HashSet::new();
        for symbol in &module.symbols {
            if symbol.symbol.trim().is_empty() {
                return Err(ReflectionLoadError::InvalidMetadata(format!(
                    "empty symbol in module {}",
                    module.module_id
                )));
            }
            if symbol.signature.trim().is_empty() {
                return Err(ReflectionLoadError::InvalidMetadata(format!(
                    "empty signature for symbol {}",
                    symbol.symbol
                )));
            }
            if let Some(native_symbol) = &symbol.native_symbol {
                if native_symbol.trim().is_empty() {
                    return Err(ReflectionLoadError::InvalidMetadata(format!(
                        "empty native_symbol for symbol {}",
                        symbol.symbol
                    )));
                }
            }
            if !symbol
                .symbol
                .starts_with(&(module.module_id.clone() + "::"))
            {
                return Err(ReflectionLoadError::InvalidMetadata(format!(
                    "symbol {} does not belong to module {}",
                    symbol.symbol, module.module_id
                )));
            }
            if !symbol_ids.insert(symbol.symbol.clone()) {
                return Err(ReflectionLoadError::InvalidMetadata(format!(
                    "duplicate symbol {} in module {}",
                    symbol.symbol, module.module_id
                )));
            }
        }
    }
    Ok(())
}

fn parse_signature_contract(
    signature: &str,
) -> std::result::Result<SignatureContract, ReflectionLoadError> {
    let params_raw = signature
        .split('|')
        .find_map(|part| part.strip_prefix("params=["))
        .and_then(|part| part.strip_suffix(']'))
        .ok_or_else(|| {
            ReflectionLoadError::InvalidMetadata(format!(
                "invalid function signature: {}",
                signature
            ))
        })?;
    let ret_raw = signature
        .split('|')
        .find_map(|part| part.strip_prefix("ret="))
        .ok_or_else(|| {
            ReflectionLoadError::InvalidMetadata(format!(
                "invalid function signature: {}",
                signature
            ))
        })?;

    let mut params = Vec::new();
    if !params_raw.trim().is_empty() {
        for param in params_raw.split(',') {
            let ty = param.rsplit(':').next().unwrap_or_default().trim();
            params.push(PrimitiveType::from_name(ty));
        }
    }

    Ok(SignatureContract {
        params,
        ret: PrimitiveType::from_name(ret_raw.trim()),
    })
}

#[cfg(windows)]
mod native_loader {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(path: *const u16) -> *mut c_void;
        fn GetProcAddress(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
        fn FreeLibrary(handle: *mut c_void) -> i32;
    }

    pub unsafe fn open(path: &Path) -> std::result::Result<*mut c_void, String> {
        let mut wide = path.as_os_str().encode_wide().collect::<Vec<u16>>();
        wide.push(0);
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(handle)
    }

    pub unsafe fn get(
        handle: *mut c_void,
        symbol: &str,
    ) -> std::result::Result<*mut c_void, String> {
        let c_symbol = CString::new(symbol)
            .map_err(|_| format!("symbol contains interior NUL byte: {symbol}"))?;
        let raw = unsafe { GetProcAddress(handle, c_symbol.as_bytes_with_nul().as_ptr()) };
        if raw.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(raw)
    }

    pub unsafe fn close(handle: *mut c_void) -> std::result::Result<(), String> {
        if unsafe { FreeLibrary(handle) } == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[link(name = "dl")]
unsafe extern "C" {}

#[cfg(unix)]
mod native_loader {
    use super::*;

    const RTLD_NOW: c_int = 2;

    unsafe extern "C" {
        fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
        fn dlerror() -> *const c_char;
    }

    fn dl_last_error() -> String {
        unsafe {
            let ptr = dlerror();
            if ptr.is_null() {
                "unknown dlerror".to_string()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    }

    pub unsafe fn open(path: &Path) -> std::result::Result<*mut c_void, String> {
        let path_string = path.to_string_lossy().into_owned();
        let c_path = CString::new(path_string)
            .map_err(|_| "library path contains interior NUL byte".to_string())?;
        let handle = unsafe { dlopen(c_path.as_ptr(), RTLD_NOW) };
        if handle.is_null() {
            return Err(dl_last_error());
        }
        Ok(handle)
    }

    pub unsafe fn get(
        handle: *mut c_void,
        symbol: &str,
    ) -> std::result::Result<*mut c_void, String> {
        let c_symbol = CString::new(symbol)
            .map_err(|_| format!("symbol contains interior NUL byte: {symbol}"))?;
        unsafe {
            // Clear any stale loader error first.
            let _ = dlerror();
        }
        let ptr = unsafe { dlsym(handle, c_symbol.as_ptr()) };
        if ptr.is_null() {
            return Err(dl_last_error());
        }
        Ok(ptr)
    }

    pub unsafe fn close(handle: *mut c_void) -> std::result::Result<(), String> {
        if unsafe { dlclose(handle) } != 0 {
            return Err(dl_last_error());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ReflectInvokeError, ReflectValue, ReflectionMetadata, ReflectionModuleMetadata,
        ReflectionRuntime, ReflectionSymbolMetadata, REFLECTION_SCHEMA_VERSION,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_metadata_path(tag: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sengoo-reflect-{}-{}-{}.json",
            tag,
            std::process::id(),
            ts
        ))
    }

    fn metadata_fixture(module_id: &str, symbol: &str, signature: &str) -> ReflectionMetadata {
        ReflectionMetadata {
            schema_version: REFLECTION_SCHEMA_VERSION,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            compatible_compiler_versions: vec![env!("CARGO_PKG_VERSION").to_string()],
            root_module: module_id.to_string(),
            modules: vec![ReflectionModuleMetadata {
                module_id: module_id.to_string(),
                symbols: vec![ReflectionSymbolMetadata {
                    symbol: symbol.to_string(),
                    signature: signature.to_string(),
                    native_symbol: Some(symbol.rsplit("::").next().unwrap_or_default().to_string()),
                }],
            }],
        }
    }

    fn write_metadata(tag: &str, metadata: &ReflectionMetadata) -> PathBuf {
        let path = temp_metadata_path(tag);
        let bytes = serde_json::to_vec_pretty(metadata).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn lazy_loader_initializes_on_first_api_call() {
        let module = "tests/main.sg";
        let symbol = "tests/main.sg::add";
        let metadata = metadata_fixture(
            module,
            symbol,
            "pub|add|async=false|self=-|tp=[]|params=[a:i64,b:i64]|ret=i64",
        );
        let path = write_metadata("lazy", &metadata);

        let runtime = ReflectionRuntime::new(&path);
        assert!(!runtime.is_loaded());

        let symbols = runtime.list_symbols(module).unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol, symbol);
        assert!(runtime.is_loaded());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn typed_invoke_returns_structured_errors_and_success() {
        let module = "tests/main.sg";
        let symbol = "tests/main.sg::add";
        let metadata = metadata_fixture(
            module,
            symbol,
            "pub|add|async=false|self=-|tp=[]|params=[a:i64,b:i64]|ret=i64",
        );
        let path = write_metadata("typed", &metadata);
        let runtime = ReflectionRuntime::new(&path);

        runtime
            .register_fn(module, "add", |args| match args {
                [ReflectValue::I64(a), ReflectValue::I64(b)] => Ok(ReflectValue::I64(a + b)),
                _ => Err("invalid args".to_string()),
            })
            .unwrap();

        let ok = runtime
            .call_i64(module, "add", &[ReflectValue::I64(2), ReflectValue::I64(5)])
            .unwrap();
        assert_eq!(ok, 7);

        let arity = runtime.call_i64(module, "add", &[ReflectValue::I64(2)]);
        assert!(matches!(
            arity,
            Err(ReflectInvokeError::ArityMismatch {
                expected: 2,
                actual: 1
            })
        ));

        let ty = runtime.call_i64(
            module,
            "add",
            &[ReflectValue::I64(1), ReflectValue::Bool(true)],
        );
        assert!(matches!(ty, Err(ReflectInvokeError::TypeMismatch { .. })));

        let missing = runtime.call_i64(module, "missing", &[ReflectValue::I64(1)]);
        assert!(matches!(
            missing,
            Err(ReflectInvokeError::SymbolNotFound { .. })
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn default_deny_rejects_unreflected_registration() {
        let module = "tests/main.sg";
        let symbol = "tests/main.sg::add";
        let metadata = metadata_fixture(
            module,
            symbol,
            "pub|add|async=false|self=-|tp=[]|params=[a:i64,b:i64]|ret=i64",
        );
        let path = write_metadata("default-deny", &metadata);
        let runtime = ReflectionRuntime::new(&path);

        let rejected = runtime.register_fn(module, "sub", |_args| Ok(ReflectValue::I64(0)));
        assert!(matches!(
            rejected,
            Err(ReflectInvokeError::SymbolNotFound { .. })
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn incompatible_compiler_version_is_rejected() {
        let module = "tests/main.sg";
        let symbol = "tests/main.sg::add";
        let mut metadata = metadata_fixture(
            module,
            symbol,
            "pub|add|async=false|self=-|tp=[]|params=[a:i64,b:i64]|ret=i64",
        );
        metadata.compatible_compiler_versions = vec!["999.999.999".to_string()];

        let path = write_metadata("compat", &metadata);
        let runtime = ReflectionRuntime::new(&path);

        let result = runtime.list_symbols(module);
        assert!(matches!(result, Err(ReflectInvokeError::Load(_))));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn native_binding_reports_library_load_error() {
        let module = "tests/main.sg";
        let symbol = "tests/main.sg::add";
        let metadata = metadata_fixture(
            module,
            symbol,
            "pub|add|async=false|self=-|tp=[]|params=[a:i64,b:i64]|ret=i64",
        );
        let path = write_metadata("native-load", &metadata);
        let runtime = ReflectionRuntime::new(&path);

        let err = runtime
            .register_i64_native_bindings_from_library("does-not-exist-for-reflection-test")
            .unwrap_err();
        assert!(matches!(err, ReflectInvokeError::NativeLibraryLoad { .. }));

        let _ = fs::remove_file(path);
    }
}
