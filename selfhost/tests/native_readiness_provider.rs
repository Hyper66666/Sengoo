use sengoo_runtime::async_runtime::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicI64, Ordering};
struct Observed;
static ALLOCATIONS: AtomicI64 = AtomicI64::new(0);
static BYTES: AtomicI64 = AtomicI64::new(0);
unsafe impl GlobalAlloc for Observed {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(l.size() as i64, Ordering::Relaxed);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(l);
        if !p.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(l.size() as i64, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        ALLOCATIONS.fetch_sub(1, Ordering::Relaxed);
        BYTES.fetch_sub(l.size() as i64, Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = System.realloc(p, l, n);
        if !q.is_null() {
            BYTES.fetch_add(n as i64 - l.size() as i64, Ordering::Relaxed);
        }
        q
    }
}
#[global_allocator]
static ALLOCATOR: Observed = Observed;
extern "C" fn observe(kind: i64) -> i64 {
    if kind == 0 {
        ALLOCATIONS.load(Ordering::Relaxed)
    } else {
        BYTES.load(Ordering::Relaxed)
    }
}
// Runtime bridge link requirements for unrelated generated-program dispatch.
// Any call is a failed probe; no readiness operation is implemented here.
#[no_mangle]
extern "C" fn main__start() -> i64 {
    std::process::abort()
}
#[no_mangle]
extern "C" fn main__poll(_: i64) -> i64 {
    std::process::abort()
}
#[no_mangle]
extern "C" fn main__result(_: i64) -> i64 {
    std::process::abort()
}
macro_rules! dispatch_trap {
    ($name:ident,$ty:ty) => {
        #[no_mangle]
        extern "C" fn $name(_: i64, _: i64) -> $ty {
            std::process::abort()
        }
    };
}
dispatch_trap!(sengoo_async_poll_dispatch, i64);
dispatch_trap!(sengoo_async_cancel_dispatch, bool);
dispatch_trap!(sengoo_async_drop_dispatch, ());
dispatch_trap!(sengoo_async_result_dispatch_i8, i8);
dispatch_trap!(sengoo_async_result_dispatch_i16, i16);
dispatch_trap!(sengoo_async_result_dispatch_i32, i32);
dispatch_trap!(sengoo_async_result_dispatch_i64, i64);
dispatch_trap!(sengoo_async_result_dispatch_bool, bool);
dispatch_trap!(sengoo_async_result_dispatch_f32, f32);
dispatch_trap!(sengoo_async_result_dispatch_f64, f64);
extern "C" {
    fn readiness_probe(
        open: extern "C" fn(i64) -> i64,
        close: extern "C" fn(i64) -> bool,
        read: unsafe extern "C" fn(i64, *mut u8, i64) -> i64,
        start: extern "C" fn(i64, i64) -> i64,
        poll: unsafe extern "C" fn(i64) -> i64,
        result: unsafe extern "C" fn(*mut AsyncFileReadinessOutcome, i64),
        cancel: unsafe extern "C" fn(i64) -> bool,
        drop: unsafe extern "C" fn(i64),
        observe: extern "C" fn(i64) -> i64,
        path: *const std::ffi::c_char,
        rounds: i32,
    ) -> i32;
}
fn main() {
    assert_eq!(std::mem::size_of::<AsyncFileReadinessOutcome>(), 16);
    assert_eq!(std::mem::align_of::<AsyncFileReadinessOutcome>(), 8);
    assert_eq!(std::mem::offset_of!(AsyncFileReadinessOutcome, is_ok), 0);
    assert_eq!(std::mem::offset_of!(AsyncFileReadinessOutcome, value), 1);
    assert_eq!(std::mem::offset_of!(AsyncFileReadinessOutcome, error), 8);
    let path = std::env::args().nth(1).expect("fixture path");
    let c_path = std::ffi::CString::new(path).unwrap();
    let code = unsafe {
        readiness_probe(
            sengoo_async_file_open_read,
            sengoo_async_file_close,
            sengoo_async_file_read_into,
            sengoo_async_file_wait_readable__start,
            sengoo_async_file_wait_readable__poll,
            sengoo_async_file_wait_readable__result,
            sengoo_async_file_wait_readable__cancel,
            sengoo_async_file_wait_readable__drop,
            observe,
            c_path.as_ptr(),
            100,
        )
    };
    if code != 0 {
        std::process::exit(1);
    }
    println!("passed readiness provider: 16 cases x 100 measured rounds; exact Rust allocation count/bytes and Win32 handles restored");
}
