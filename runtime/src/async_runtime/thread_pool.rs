use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
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

#[derive(Default)]
struct WorkerQueue {
    jobs: Mutex<VecDeque<PoolJobHandle>>,
}

struct PoolState {
    queues: Vec<Arc<WorkerQueue>>,
    next_queue: AtomicUsize,
    steal_count: AtomicUsize,
    shutdown: AtomicBool,
    wake_mutex: Mutex<()>,
    wake: Condvar,
}

impl PoolState {
    fn new(worker_count: usize) -> Self {
        Self {
            queues: (0..worker_count)
                .map(|_| Arc::new(WorkerQueue::default()))
                .collect(),
            next_queue: AtomicUsize::new(0),
            steal_count: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            wake_mutex: Mutex::new(()),
            wake: Condvar::new(),
        }
    }

    fn enqueue(&self, job: PoolJobHandle) -> bool {
        let index = self.next_queue.fetch_add(1, Ordering::Relaxed) % self.queues.len();
        self.enqueue_to(job, index)
    }

    fn enqueue_to(&self, job: PoolJobHandle, index: usize) -> bool {
        let Some(queue) = self.queues.get(index) else {
            return false;
        };
        if self.shutdown.load(Ordering::Acquire) {
            return false;
        }
        queue
            .jobs
            .lock()
            .expect("worker queue mutex poisoned")
            .push_back(job);
        self.wake.notify_one();
        true
    }

    fn pop_or_steal(&self, worker_index: usize) -> Option<PoolJobHandle> {
        if let Some(job) = self.queues[worker_index]
            .jobs
            .lock()
            .expect("worker queue mutex poisoned")
            .pop_front()
        {
            return Some(job);
        }

        for offset in 1..self.queues.len() {
            let victim = (worker_index + offset) % self.queues.len();
            if let Some(job) = self.queues[victim]
                .jobs
                .lock()
                .expect("worker queue mutex poisoned")
                .pop_back()
            {
                self.steal_count.fetch_add(1, Ordering::Relaxed);
                return Some(job);
            }
        }
        None
    }

    fn wait_for_work(&self) {
        if self.shutdown.load(Ordering::Acquire) {
            return;
        }
        let Ok(guard) = self.wake_mutex.lock() else {
            return;
        };
        let _ = self
            .wake
            .wait_timeout(guard, std::time::Duration::from_millis(10))
            .ok();
    }
}

struct ThreadPool {
    workers: Vec<JoinHandle<()>>,
    state: Arc<PoolState>,
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.state.shutdown.store(true, Ordering::Release);
        self.state.wake.notify_all();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

static POOL_ENABLED: AtomicBool = AtomicBool::new(false);
static POOL: Mutex<Option<ThreadPool>> = Mutex::new(None);

const MAX_WORKER_COUNT: i64 = 256;

pub(crate) fn runtime_enable_thread_pool(worker_count: i64) -> Result<i64, i64> {
    if !(1..=MAX_WORKER_COUNT).contains(&worker_count) {
        return Err(STATUS_INVALID_ARGUMENT);
    }

    let mut pool_guard = POOL.lock().expect("thread pool mutex poisoned");
    if pool_guard.is_some() {
        return Ok(1);
    }

    let worker_count = worker_count as usize;
    let state = Arc::new(PoolState::new(worker_count));
    let workers = (0..worker_count)
        .map(|worker_index| spawn_pool_worker(state.clone(), worker_index))
        .collect();
    *pool_guard = Some(ThreadPool { workers, state });
    POOL_ENABLED.store(true, Ordering::Release);
    Ok(1)
}

pub(crate) fn is_thread_pool_enabled() -> bool {
    POOL_ENABLED.load(Ordering::Acquire)
}

#[cfg(test)]
pub(crate) fn test_only_disable_thread_pool() {
    POOL_ENABLED.store(false, Ordering::Release);
    let pool = POOL.lock().expect("thread pool mutex poisoned").take();
    drop(pool);
}

fn spawn_pool_worker(state: Arc<PoolState>, worker_index: usize) -> JoinHandle<()> {
    thread::spawn(move || loop {
        if let Some(job) = state.pop_or_steal(worker_index) {
            execute_job(job);
            continue;
        }
        if state.shutdown.load(Ordering::Acquire) {
            break;
        }
        state.wait_for_work();
    })
}

fn execute_job(job: PoolJobHandle) {
    if job.canceled.load(Ordering::Acquire) {
        return;
    }
    let Some(value) = job.execute() else {
        return;
    };
    if job.canceled.load(Ordering::Acquire) {
        return;
    }
    if let Ok(mut slot) = job.result.lock() {
        *slot = Some(value);
    }
    job.completed.store(true, Ordering::Release);
    signal_scheduler_wakeup();
}

fn submit_job(job: &PoolJobHandle) -> bool {
    if !is_thread_pool_enabled() {
        return false;
    }
    let pool_guard = POOL.lock().expect("thread pool mutex poisoned");
    pool_guard
        .as_ref()
        .is_some_and(|pool| pool.state.enqueue(job.clone_for_queue()))
}

pub(crate) fn submit_pool_job(work_fn: extern "C" fn() -> i64) -> Option<PoolJobHandle> {
    let job = PoolJobHandle::new(work_fn);
    submit_job(&job).then_some(job)
}

pub(crate) fn submit_pool_task<F>(work: F) -> Option<PoolJobHandle>
where
    F: FnOnce() -> i64 + Send + 'static,
{
    let job = PoolJobHandle::new_task(Box::new(work));
    submit_job(&job).then_some(job)
}

#[cfg(test)]
pub(crate) fn test_only_submit_to_worker<F>(work: F, worker_index: usize) -> Option<PoolJobHandle>
where
    F: FnOnce() -> i64 + Send + 'static,
{
    let job = PoolJobHandle::new_task(Box::new(work));
    let pool_guard = POOL.lock().expect("thread pool mutex poisoned");
    pool_guard
        .as_ref()
        .and_then(|pool| {
            pool.state
                .enqueue_to(job.clone_for_queue(), worker_index)
                .then_some(())
        })
        .map(|()| job)
}

#[cfg(test)]
pub(crate) fn test_only_steal_count() -> usize {
    POOL.lock()
        .expect("thread pool mutex poisoned")
        .as_ref()
        .map(|pool| pool.state.steal_count.load(Ordering::Acquire))
        .unwrap_or(0)
}
