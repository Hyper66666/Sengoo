use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Default)]
struct UserContextState {
    wake_after_ms: Option<i64>,
}

#[derive(Debug)]
struct UserContextRegistry {
    next_handle: i64,
    states: HashMap<i64, UserContextState>,
}

impl Default for UserContextRegistry {
    fn default() -> Self {
        Self {
            next_handle: 1,
            states: HashMap::new(),
        }
    }
}

impl UserContextRegistry {
    fn insert(&mut self) -> i64 {
        let mut handle = self.next_handle.max(1);
        while self.states.contains_key(&handle) {
            handle = if handle == i64::MAX { 1 } else { handle + 1 };
        }
        self.next_handle = if handle == i64::MAX { 1 } else { handle + 1 };
        self.states.insert(handle, UserContextState::default());
        handle
    }
}

static USER_CONTEXTS: OnceLock<Mutex<UserContextRegistry>> = OnceLock::new();

fn user_contexts() -> &'static Mutex<UserContextRegistry> {
    USER_CONTEXTS.get_or_init(|| Mutex::new(UserContextRegistry::default()))
}

#[no_mangle]
pub extern "C" fn sengoo_async_context_begin() -> i64 {
    user_contexts()
        .lock()
        .expect("user context registry poisoned")
        .insert()
}

#[no_mangle]
pub extern "C" fn sengoo_async_context_wake_after(handle: i64, delay_ms: i64) -> bool {
    if delay_ms < 0 {
        return false;
    }
    let mut registry = user_contexts()
        .lock()
        .expect("user context registry poisoned");
    let Some(state) = registry.states.get_mut(&handle) else {
        return false;
    };
    state.wake_after_ms = Some(
        state
            .wake_after_ms
            .map_or(delay_ms, |current| current.min(delay_ms)),
    );
    true
}

#[no_mangle]
pub extern "C" fn sengoo_async_context_wake(handle: i64) -> bool {
    sengoo_async_context_wake_after(handle, 0)
}

#[no_mangle]
pub extern "C" fn sengoo_async_context_finish_delay(handle: i64) -> i64 {
    user_contexts()
        .lock()
        .expect("user context registry poisoned")
        .states
        .remove(&handle)
        .and_then(|state| state.wake_after_ms)
        .unwrap_or(1)
}

#[no_mangle]
pub extern "C" fn sengoo_async_context_drop(handle: i64) -> bool {
    user_contexts()
        .lock()
        .expect("user context registry poisoned")
        .states
        .remove(&handle)
        .is_some()
}

#[cfg(test)]
fn user_context_live_handle_count() -> usize {
    user_contexts()
        .lock()
        .expect("user context registry poisoned")
        .states
        .len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_context_keeps_earliest_wakeup_and_finishes_once() {
        let handle = sengoo_async_context_begin();
        assert!(handle > 0);
        assert_eq!(user_context_live_handle_count(), 1);
        assert!(sengoo_async_context_wake_after(handle, 20));
        assert!(sengoo_async_context_wake_after(handle, 5));
        assert_eq!(sengoo_async_context_finish_delay(handle), 5);
        assert_eq!(sengoo_async_context_finish_delay(handle), 1);
        assert_eq!(user_context_live_handle_count(), 0);
    }

    #[test]
    fn user_context_wake_is_zero_delay_and_drop_is_exact() {
        let handle = sengoo_async_context_begin();
        assert!(sengoo_async_context_wake(handle));
        assert_eq!(sengoo_async_context_finish_delay(handle), 0);
        assert!(!sengoo_async_context_drop(handle));

        let dropped = sengoo_async_context_begin();
        assert!(sengoo_async_context_drop(dropped));
        assert!(!sengoo_async_context_drop(dropped));
        assert!(!sengoo_async_context_wake_after(dropped, 1));
        assert_eq!(user_context_live_handle_count(), 0);
    }

    #[test]
    fn user_context_missing_or_invalid_wakeup_uses_bounded_fallback() {
        let handle = sengoo_async_context_begin();
        assert!(!sengoo_async_context_wake_after(handle, -1));
        assert_eq!(sengoo_async_context_finish_delay(handle), 1);
        assert_eq!(sengoo_async_context_finish_delay(0), 1);
    }
}
