use std::ffi::{c_void, CString};
use std::path::Path;

#[cfg(unix)]
use std::ffi::{c_char, c_int, CStr};

use super::{ReflectInvokeError, ReflectValue, SignatureContract};

pub(super) struct NativeLibrary {
    handle: *mut c_void,
}

impl NativeLibrary {
    pub(super) unsafe fn open(path: &Path) -> std::result::Result<Self, String> {
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
pub(super) enum I64NativeFn {
    Arity0(unsafe extern "C" fn() -> i64),
    Arity1(unsafe extern "C" fn(i64) -> i64),
    Arity2(unsafe extern "C" fn(i64, i64) -> i64),
    Arity3(unsafe extern "C" fn(i64, i64, i64) -> i64),
    Arity4(unsafe extern "C" fn(i64, i64, i64, i64) -> i64),
}

impl I64NativeFn {
    pub(super) unsafe fn invoke(
        self,
        args: &[ReflectValue],
    ) -> std::result::Result<ReflectValue, String> {
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

pub(super) fn is_i64_signature(contract: &SignatureContract) -> bool {
    if contract.ret != super::PrimitiveType::I64 {
        return false;
    }
    contract
        .params
        .iter()
        .all(|param| *param == super::PrimitiveType::I64)
}

pub(super) unsafe fn load_i64_native_fn(
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

#[cfg(windows)]
pub(super) mod native_loader {
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
pub(super) mod native_loader {
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
