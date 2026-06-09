use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Condvar, Mutex};
use std::thread::{self, JoinHandle};

use super::concurrent::PoolJobHandle;

pub(crate) const STATUS_INVALID_ARGUMENT: i64 = 2;

struct SchedulerWakeup {
    signaled: AtomicBool,
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl SchedulerWakeup {
    const fn new() -> Self {
        Self {
            signaled: AtomicBool::new(false),
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    fn signal(&self) {
        self.signaled.store(true, Ordering::Release);
        if let Ok(guard) = self.mutex.lock() {
            self.condvar.notify_one();
            drop(guard);
        }
    }

    fn take_signaled(&self) -> bool {
        self.signaled.swap(false, Ordering::AcqRel)
    }

    fn wait_for_signal_or_timeout(&self, timeout: std::time::Duration) {
        if self.signaled.load(Ordering::Acquire) {
            return;
        }
        let Ok(guard) = self.mutex.lock() else {
            return;
        };
        let _ = self.condvar.wait_timeout(guard, timeout).ok();
    }
}

static SCHEDULER_WAKEUP: SchedulerWakeup = SchedulerWakeup::new();

pub(crate) fn signal_scheduler_wakeup() {
    SCHEDULER_WAKEUP.signal();
}

pub(crate) fn take_cross_thread_wakeup() -> bool {
    SCHEDULER_WAKEUP.take_signaled()
}

pub(crate) fn wait_for_cross_thread_wakeup(timeout: std::time::Duration) {
    SCHEDULER_WAKEUP.wait_for_signal_or_timeout(timeout);
}

struct ThreadPool {
    _workers: Vec<JoinHandle<()>>,
    job_tx: Sender<PoolJobHandle>,
}

static POOL_ENABLED: AtomicBool = AtomicBool::new(false);
static POOL: Mutex<Option<ThreadPool>> = Mutex::new(None);

const MAX_WORKER_COUNT: i64 = 256;

pub(crate) fn runtime_enable_thread_pool(worker_count: i64) -> Result<i64, i64> {
    if worker_count < 1 {
        return Err(STATUS_INVALID_ARGUMENT);
    }
    if worker_count > MAX_WORKER_COUNT {
        return Err(STATUS_INVALID_ARGUMENT);
    }

    let mut pool_guard = POOL.lock().expect("thread pool mutex poisoned");
    if pool_guard.is_some() {
        return Ok(1);
    }

    let (job_tx, job_rx) = mpsc::channel::<PoolJobHandle>();
    let shared_rx = ArcReceiver::new(job_rx);
    let worker_count = worker_count as usize;
    let mut workers = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        workers.push(spawn_pool_worker(shared_rx.clone()));
    }
    *pool_guard = Some(ThreadPool {
        _workers: workers,
        job_tx,
    });
    POOL_ENABLED.store(true, Ordering::Release);
    Ok(1)
}

pub(crate) fn is_thread_pool_enabled() -> bool {
    POOL_ENABLED.load(Ordering::Acquire)
}

#[cfg(test)]
pub(crate) fn test_only_disable_thread_pool() {
    POOL_ENABLED.store(false, Ordering::Release);
    let mut pool_guard = POOL.lock().expect("thread pool mutex poisoned");
    *pool_guard = None;
}

#[derive(Clone)]
struct ArcReceiver {
    inner: std::sync::Arc<Mutex<Receiver<PoolJobHandle>>>,
}

impl ArcReceiver {
    fn new(receiver: Receiver<PoolJobHandle>) -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(receiver)),
        }
    }

    fn recv(&self) -> Option<PoolJobHandle> {
        self.inner
            .lock()
            .expect("pool receiver mutex poisoned")
            .recv()
            .ok()
    }
}

fn spawn_pool_worker(job_rx: ArcReceiver) -> JoinHandle<()> {
    thread::spawn(move || {
        while let Some(job) = job_rx.recv() {
            if job.canceled.load(Ordering::Acquire) {
                continue;
            }
            let value = (job.work_fn)();
            if job.canceled.load(Ordering::Acquire) {
                continue;
            }
            if let Ok(mut slot) = job.result.lock() {
                *slot = Some(value);
            }
            job.completed.store(true, Ordering::Release);
            signal_scheduler_wakeup();
        }
    })
}

pub(crate) fn submit_pool_job(work_fn: extern "C" fn() -> i64) -> Option<PoolJobHandle> {
    if !is_thread_pool_enabled() {
        return None;
    }
    let job = PoolJobHandle::new(work_fn);
    let pool_guard = POOL.lock().expect("thread pool mutex poisoned");
    let pool = pool_guard.as_ref()?;
    pool.job_tx.send(job.clone_for_queue()).ok()?;
    Some(job)
}
