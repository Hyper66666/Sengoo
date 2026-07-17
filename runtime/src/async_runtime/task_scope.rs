use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Condvar, LazyLock, Mutex};

use super::bridge::{
    sengoo_async_cancel_task, sengoo_async_join_task, sengoo_async_spawn_task_raw,
    DetachedForeignAsyncTask,
};
use super::TaskLifecycleStatus;

static NEXT_SCOPE_ID: AtomicI64 = AtomicI64::new(1);

#[derive(Default)]
struct TaskScopeEntry {
    children: Vec<i64>,
    in_flight_spawns: usize,
    closing: bool,
}

static TASK_SCOPES: LazyLock<(Mutex<HashMap<i64, TaskScopeEntry>>, Condvar)> =
    LazyLock::new(|| (Mutex::new(HashMap::new()), Condvar::new()));

#[no_mangle]
pub extern "C" fn sengoo_async_task_scope_new() -> i64 {
    let scope_id = NEXT_SCOPE_ID.fetch_add(1, Ordering::AcqRel);
    if scope_id <= 0 {
        return 0;
    }
    TASK_SCOPES
        .0
        .lock()
        .expect("task scope registry mutex poisoned")
        .insert(scope_id, TaskScopeEntry::default());
    scope_id
}

#[no_mangle]
pub extern "C" fn sengoo_async_task_scope_spawn_raw(scope_id: i64, kind: i64, handle: i64) -> i64 {
    let (scopes, scope_changed) = &*TASK_SCOPES;
    {
        let mut scopes = scopes.lock().expect("task scope registry mutex poisoned");
        let Some(scope) = scopes.get_mut(&scope_id) else {
            drop(scopes);
            DetachedForeignAsyncTask { kind, handle }.release_rejected();
            return 0;
        };
        if scope.closing {
            drop(scopes);
            DetachedForeignAsyncTask { kind, handle }.release_rejected();
            return 0;
        }
        scope.in_flight_spawns += 1;
    }

    // Spawning may run rejection cleanup or scheduler code. Keep it outside the
    // registry lock while the in-flight count prevents close from removing the scope.
    let task_id = sengoo_async_spawn_task_raw(kind, handle);

    let mut scopes = scopes.lock().expect("task scope registry mutex poisoned");
    let Some(scope) = scopes.get_mut(&scope_id) else {
        debug_assert!(
            false,
            "in-flight task scope disappeared before spawn completed"
        );
        return 0;
    };
    scope.in_flight_spawns -= 1;
    if task_id != 0 {
        scope.children.push(task_id);
    }
    if scope.in_flight_spawns == 0 {
        scope_changed.notify_all();
    }
    i64::from(task_id != 0)
}

#[no_mangle]
pub extern "C" fn sengoo_async_task_scope_join(scope_id: i64) -> i64 {
    close_scope(scope_id, false)
}

#[no_mangle]
pub extern "C" fn sengoo_async_task_scope_cancel_join(scope_id: i64) -> i64 {
    close_scope(scope_id, true)
}

fn close_scope(scope_id: i64, cancel: bool) -> i64 {
    let (scopes, scope_changed) = &*TASK_SCOPES;
    let mut scopes = scopes.lock().expect("task scope registry mutex poisoned");
    let Some(scope) = scopes.get_mut(&scope_id) else {
        return TaskLifecycleStatus::Unknown as i64;
    };
    if scope.closing {
        return TaskLifecycleStatus::Unknown as i64;
    }
    scope.closing = true;
    while scopes
        .get(&scope_id)
        .is_some_and(|scope| scope.in_flight_spawns != 0)
    {
        scopes = scope_changed
            .wait(scopes)
            .expect("task scope registry mutex poisoned while closing");
    }
    let children = scopes
        .remove(&scope_id)
        .expect("closing task scope disappeared")
        .children;
    drop(scopes);

    if cancel {
        for task_id in &children {
            let _ = sengoo_async_cancel_task(*task_id);
        }
    }

    let mut summary = if cancel {
        TaskLifecycleStatus::Canceled
    } else {
        TaskLifecycleStatus::Completed
    };
    for task_id in children {
        let status = sengoo_async_join_task(task_id);
        if status == TaskLifecycleStatus::Failed as i64 {
            summary = TaskLifecycleStatus::Failed;
        } else if !cancel && status == TaskLifecycleStatus::Canceled as i64 {
            summary = TaskLifecycleStatus::Canceled;
        } else if status == TaskLifecycleStatus::Unknown as i64 {
            summary = TaskLifecycleStatus::Unknown;
        }
    }
    summary as i64
}

#[cfg(test)]
pub(crate) fn active_scope_count() -> usize {
    TASK_SCOPES
        .0
        .lock()
        .expect("task scope registry mutex poisoned")
        .len()
}
