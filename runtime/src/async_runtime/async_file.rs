use std::collections::HashMap;
use std::ffi::{c_char, CStr};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::futures::PollLifecycle;
use super::reactor::{owned_file_poll_readable, owned_file_register, owned_file_unregister};
use super::{handle_mut, handle_take_box, record_external_poll_wakeup_hint};

const STATUS_UNKNOWN: i64 = 1;
const STATUS_INVALID_ARGUMENT: i64 = 2;
const STATUS_INVALID_HANDLE: i64 = 3;
const STATUS_NOT_FOUND: i64 = 5;
const STATUS_PERMISSION_DENIED: i64 = 7;
const STATUS_UNSUPPORTED: i64 = 8;
const STATUS_IO: i64 = 9;
const STATUS_TIMEOUT: i64 = 11;

struct AsyncFileRegistry {
    next_handle: AtomicU64,
    files: Mutex<HashMap<u64, File>>,
}

impl Default for AsyncFileRegistry {
    fn default() -> Self {
        Self {
            next_handle: AtomicU64::new(1),
            files: Mutex::new(HashMap::new()),
        }
    }
}

impl AsyncFileRegistry {
    fn insert(&self, file: File) -> i64 {
        let handle = self.next_handle.fetch_add(1, Ordering::Relaxed).max(1);
        self.files
            .lock()
            .expect("async file registry poisoned")
            .insert(handle, file);
        i64::try_from(handle).unwrap_or(0)
    }
}

static ASYNC_FILES: OnceLock<AsyncFileRegistry> = OnceLock::new();

fn async_files() -> &'static AsyncFileRegistry {
    ASYNC_FILES.get_or_init(AsyncFileRegistry::default)
}

fn status_from_io_kind(kind: ErrorKind) -> i64 {
    match kind {
        ErrorKind::InvalidInput | ErrorKind::InvalidData => STATUS_INVALID_ARGUMENT,
        ErrorKind::NotFound => STATUS_NOT_FOUND,
        ErrorKind::PermissionDenied => STATUS_PERMISSION_DENIED,
        ErrorKind::Unsupported => STATUS_UNSUPPORTED,
        _ => STATUS_IO,
    }
}

fn negative_status(status: i64) -> i64 {
    -status.max(1)
}

#[no_mangle]
pub extern "C" fn sengoo_async_file_open_read(path_ptr: i64) -> i64 {
    let path = path_ptr as *const c_char;
    if path.is_null() {
        return negative_status(STATUS_INVALID_ARGUMENT);
    }
    let path = unsafe { CStr::from_ptr(path) };
    if path.is_empty() {
        return negative_status(STATUS_INVALID_ARGUMENT);
    }
    let Ok(path) = path.to_str() else {
        return negative_status(STATUS_INVALID_ARGUMENT);
    };
    match File::open(path) {
        Ok(file) => async_files().insert(file),
        Err(error) => negative_status(status_from_io_kind(error.kind())),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_file_close(handle: i64) -> bool {
    let Ok(handle) = u64::try_from(handle) else {
        return false;
    };
    async_files()
        .files
        .lock()
        .expect("async file registry poisoned")
        .remove(&handle)
        .is_some()
}

#[no_mangle]
/// # Safety
///
/// `buffer` must be valid for writes of `capacity` bytes for the duration of
/// this call.
pub unsafe extern "C" fn sengoo_async_file_read_into(
    handle: i64,
    buffer: *mut u8,
    capacity: i64,
) -> i64 {
    if capacity < 0 || (buffer.is_null() && capacity > 0) {
        return negative_status(STATUS_INVALID_ARGUMENT);
    }
    let Ok(handle) = u64::try_from(handle) else {
        return negative_status(STATUS_INVALID_HANDLE);
    };
    let mut files = async_files()
        .files
        .lock()
        .expect("async file registry poisoned");
    let Some(file) = files.get_mut(&handle) else {
        return negative_status(STATUS_INVALID_HANDLE);
    };
    let output = if capacity == 0 {
        &mut []
    } else {
        unsafe { std::slice::from_raw_parts_mut(buffer, capacity as usize) }
    };
    match file.read(output) {
        Ok(count) => i64::try_from(count).unwrap_or_else(|_| negative_status(STATUS_UNKNOWN)),
        Err(error) => negative_status(status_from_io_kind(error.kind())),
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AsyncFileReadinessOutcome {
    pub is_ok: bool,
    pub value: bool,
    pub error: i64,
}

#[derive(Debug, Clone, Copy)]
enum AsyncFileWaitOutcome {
    Pending,
    Ready(AsyncFileReadinessOutcome),
}

struct AsyncFileWaitState {
    interest_id: Option<u64>,
    deadline: Instant,
    lifecycle: PollLifecycle,
    outcome: AsyncFileWaitOutcome,
}

fn readiness_error(status: i64) -> AsyncFileReadinessOutcome {
    AsyncFileReadinessOutcome {
        is_ok: false,
        value: false,
        error: status,
    }
}

fn release_wait_interest(interest_id: &mut Option<u64>) {
    if let Some(interest) = interest_id.take() {
        owned_file_unregister(interest);
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_file_wait_readable__start(file_handle: i64, timeout_ms: i64) -> i64 {
    let (interest_id, outcome) = if timeout_ms < 0 {
        (
            None,
            AsyncFileWaitOutcome::Ready(readiness_error(STATUS_INVALID_ARGUMENT)),
        )
    } else if let Ok(file_handle) = u64::try_from(file_handle) {
        let files = async_files()
            .files
            .lock()
            .expect("async file registry poisoned");
        match files.get(&file_handle) {
            Some(file) => match owned_file_register(file) {
                Ok(interest) => (Some(interest), AsyncFileWaitOutcome::Pending),
                Err(error) => (
                    None,
                    AsyncFileWaitOutcome::Ready(readiness_error(status_from_io_kind(error))),
                ),
            },
            None => (
                None,
                AsyncFileWaitOutcome::Ready(readiness_error(STATUS_INVALID_HANDLE)),
            ),
        }
    } else {
        (
            None,
            AsyncFileWaitOutcome::Ready(readiness_error(STATUS_INVALID_HANDLE)),
        )
    };
    let state = AsyncFileWaitState {
        interest_id,
        deadline: Instant::now()
            .checked_add(Duration::from_millis(timeout_ms.max(0) as u64))
            .unwrap_or_else(Instant::now),
        lifecycle: PollLifecycle::default(),
        outcome,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_file_wait_readable__start`].
pub unsafe extern "C" fn sengoo_async_file_wait_readable__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<AsyncFileWaitState>(handle) else {
        return 1;
    };
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if matches!(state.outcome, AsyncFileWaitOutcome::Ready(_)) {
        guard.mark_ready();
        return 1;
    }
    let Some(interest) = state.interest_id else {
        state.outcome = AsyncFileWaitOutcome::Ready(readiness_error(STATUS_INVALID_HANDLE));
        guard.mark_ready();
        return 1;
    };
    match owned_file_poll_readable(interest) {
        Ok(true) => {
            release_wait_interest(&mut state.interest_id);
            state.outcome = AsyncFileWaitOutcome::Ready(AsyncFileReadinessOutcome {
                is_ok: true,
                value: true,
                error: 0,
            });
            guard.mark_ready();
            1
        }
        Ok(false) if Instant::now() >= state.deadline => {
            release_wait_interest(&mut state.interest_id);
            state.outcome = AsyncFileWaitOutcome::Ready(readiness_error(STATUS_TIMEOUT));
            guard.mark_ready();
            1
        }
        Ok(false) => {
            record_external_poll_wakeup_hint(state.deadline);
            0
        }
        Err(error) => {
            release_wait_interest(&mut state.interest_id);
            state.outcome =
                AsyncFileWaitOutcome::Ready(readiness_error(status_from_io_kind(error)));
            guard.mark_ready();
            1
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `output` must be null or valid for one initialized
/// [`AsyncFileReadinessOutcome`] write.
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_file_wait_readable__start`].
pub unsafe extern "C" fn sengoo_async_file_wait_readable__result(
    output: *mut AsyncFileReadinessOutcome,
    handle: i64,
) {
    let outcome = if let Some(mut state) = handle_take_box::<AsyncFileWaitState>(handle) {
        release_wait_interest(&mut state.interest_id);
        match state.outcome {
            AsyncFileWaitOutcome::Ready(outcome) => outcome,
            AsyncFileWaitOutcome::Pending => readiness_error(STATUS_UNKNOWN),
        }
    } else {
        readiness_error(STATUS_INVALID_HANDLE)
    };
    if !output.is_null() {
        unsafe { output.write(outcome) };
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_file_wait_readable__start`].
pub unsafe extern "C" fn sengoo_async_file_wait_readable__cancel(handle: i64) -> bool {
    let Some(mut state) = handle_take_box::<AsyncFileWaitState>(handle) else {
        return false;
    };
    release_wait_interest(&mut state.interest_id);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_file_wait_readable__start`].
pub unsafe extern "C" fn sengoo_async_file_wait_readable__drop(handle: i64) {
    let Some(mut state) = handle_take_box::<AsyncFileWaitState>(handle) else {
        return;
    };
    release_wait_interest(&mut state.interest_id);
}

#[cfg(test)]
fn async_file_live_handle_count() -> usize {
    async_files()
        .files
        .lock()
        .expect("async file registry poisoned")
        .len()
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

    fn temp_file(contents: &[u8]) -> (std::path::PathBuf, CString) {
        let id = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("sengoo-async-file-{}-{id}.txt", std::process::id()));
        fs::write(&path, contents).expect("write async-file fixture");
        let c_path = CString::new(path.to_string_lossy().as_bytes())
            .expect("temporary path must not contain NUL");
        (path, c_path)
    }

    fn wait_result(handle: i64) -> AsyncFileReadinessOutcome {
        let mut outcome = readiness_error(STATUS_UNKNOWN);
        unsafe { sengoo_async_file_wait_readable__result(&mut outcome, handle) };
        outcome
    }

    #[test]
    fn reactor_async_file_open_wait_read_and_close_is_owned() {
        let _guard = super::super::ASYNC_RUNTIME_TEST_GUARD
            .lock()
            .expect("async runtime test guard mutex poisoned");
        let (path, c_path) = temp_file(b"ready");
        let file = sengoo_async_file_open_read(c_path.as_ptr() as i64);
        assert!(file > 0);
        assert_eq!(async_file_live_handle_count(), 1);

        let wait = sengoo_async_file_wait_readable__start(file, 1_000);
        assert!(wait > 0);
        assert_eq!(unsafe { sengoo_async_file_wait_readable__poll(wait) }, 1);
        let outcome = wait_result(wait);
        assert!(outcome.is_ok);
        assert!(outcome.value);
        assert_eq!(outcome.error, 0);

        let mut bytes = [0u8; 8];
        let read =
            unsafe { sengoo_async_file_read_into(file, bytes.as_mut_ptr(), bytes.len() as i64) };
        assert_eq!(read, 5);
        assert_eq!(&bytes[..5], b"ready");
        assert!(sengoo_async_file_close(file));
        assert_eq!(async_file_live_handle_count(), 0);
        assert!(!sengoo_async_file_close(file));

        fs::remove_file(path).expect("remove async-file fixture");
    }

    #[test]
    fn reactor_async_file_invalid_handle_has_stable_status_and_no_interest() {
        let _guard = super::super::ASYNC_RUNTIME_TEST_GUARD
            .lock()
            .expect("async runtime test guard mutex poisoned");
        let before = super::super::reactor::interest_count();
        let wait = sengoo_async_file_wait_readable__start(0, 10);
        assert!(wait > 0);
        assert_eq!(unsafe { sengoo_async_file_wait_readable__poll(wait) }, 1);
        let outcome = wait_result(wait);
        assert!(!outcome.is_ok);
        assert!(!outcome.value);
        assert_eq!(outcome.error, 3);
        assert_eq!(super::super::reactor::interest_count(), before);
    }

    #[test]
    fn reactor_async_file_wait_owns_duplicate_after_source_close() {
        let _guard = super::super::ASYNC_RUNTIME_TEST_GUARD
            .lock()
            .expect("async runtime test guard mutex poisoned");
        let (path, c_path) = temp_file(b"owned");
        let baseline = super::super::reactor::interest_count();
        let file = sengoo_async_file_open_read(c_path.as_ptr() as i64);
        let wait = sengoo_async_file_wait_readable__start(file, 1_000);
        assert_eq!(super::super::reactor::interest_count(), baseline + 1);
        assert!(sengoo_async_file_close(file));

        assert_eq!(unsafe { sengoo_async_file_wait_readable__poll(wait) }, 1);
        let outcome = wait_result(wait);
        assert!(outcome.is_ok);
        assert!(outcome.value);
        assert_eq!(super::super::reactor::interest_count(), baseline);

        fs::remove_file(path).expect("remove async-file fixture");
    }

    #[test]
    fn reactor_async_file_cancel_and_drop_unregister_without_polling() {
        let _guard = super::super::ASYNC_RUNTIME_TEST_GUARD
            .lock()
            .expect("async runtime test guard mutex poisoned");
        let (path, c_path) = temp_file(b"cleanup");
        let baseline = super::super::reactor::interest_count();
        let file = sengoo_async_file_open_read(c_path.as_ptr() as i64);

        let canceled = sengoo_async_file_wait_readable__start(file, 1_000);
        assert_eq!(super::super::reactor::interest_count(), baseline + 1);
        assert!(unsafe { sengoo_async_file_wait_readable__cancel(canceled) });
        assert_eq!(super::super::reactor::interest_count(), baseline);

        let dropped = sengoo_async_file_wait_readable__start(file, 1_000);
        assert_eq!(super::super::reactor::interest_count(), baseline + 1);
        unsafe { sengoo_async_file_wait_readable__drop(dropped) };
        assert_eq!(super::super::reactor::interest_count(), baseline);

        assert!(sengoo_async_file_close(file));
        fs::remove_file(path).expect("remove async-file fixture");
    }
}
