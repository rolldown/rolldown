//! Shared async/CPU/blocking scheduler.
//! See `internal-docs/async-runtime/implementation.md`.

use std::{
  any::Any,
  collections::VecDeque,
  fmt,
  future::Future,
  panic::{AssertUnwindSafe, catch_unwind},
  pin::Pin,
  sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  task::{Context, Poll},
};

use async_task::{Runnable, Task};
use futures::FutureExt;

#[cfg(not(target_family = "wasm"))]
use futures::channel::oneshot;
#[cfg(not(target_family = "wasm"))]
use rayon::{ThreadPool, ThreadPoolBuilder};
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::AtomicUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFlavor {
  CurrentThread,
  MultiThread,
}

#[derive(Debug, Clone)]
pub struct RuntimeOptions {
  pub flavor: RuntimeFlavor,
  pub worker_threads: usize,
  pub max_blocking_tasks: usize,
  pub thread_name_prefix: String,
}

impl Default for RuntimeOptions {
  fn default() -> Self {
    let worker_threads = std::thread::available_parallelism().map_or(1, usize::from);
    Self {
      flavor: if cfg!(target_family = "wasm") {
        RuntimeFlavor::CurrentThread
      } else {
        RuntimeFlavor::MultiThread
      },
      worker_threads,
      max_blocking_tasks: worker_threads,
      thread_name_prefix: "rolldown-runtime".to_string(),
    }
  }
}

impl RuntimeOptions {
  fn validate(mut self) -> Result<Self, RuntimeConfigError> {
    if self.worker_threads == 0 {
      return Err(RuntimeConfigError("worker_threads must be greater than zero".to_string()));
    }
    if self.max_blocking_tasks == 0 {
      return Err(RuntimeConfigError("max_blocking_tasks must be greater than zero".to_string()));
    }
    if self.flavor == RuntimeFlavor::CurrentThread {
      self.worker_threads = 1;
    }
    self.max_blocking_tasks = self.max_blocking_tasks.min(self.worker_threads);
    #[cfg(target_family = "wasm")]
    if self.flavor == RuntimeFlavor::MultiThread {
      return Err(RuntimeConfigError(
        "the multi-thread runtime is unavailable in this WebAssembly build".to_string(),
      ));
    }
    Ok(self)
  }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfigError(String);

impl fmt::Display for RuntimeConfigError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl std::error::Error for RuntimeConfigError {}

#[derive(Debug)]
pub struct JoinError {
  message: String,
}

impl JoinError {
  fn from_panic(panic: &(dyn Any + Send + 'static)) -> Self {
    Self {
      message: if let Some(message) = panic.downcast_ref::<&str>() {
        (*message).to_string()
      } else if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
      } else {
        "async runtime task panicked".to_string()
      },
    }
  }
}

impl fmt::Display for JoinError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.message)
  }
}

impl std::error::Error for JoinError {}

enum JoinHandleInner<T> {
  Task(Task<Result<T, JoinError>>),
  #[cfg(not(target_family = "wasm"))]
  Blocking(oneshot::Receiver<Result<T, JoinError>>),
  Ready(Option<Result<T, JoinError>>),
}

pub struct JoinHandle<T>(JoinHandleInner<T>);

impl<T> Unpin for JoinHandle<T> {}

impl<T> JoinHandle<T> {
  pub fn detach(self) {
    if let JoinHandleInner::Task(task) = self.0 {
      task.detach();
    }
  }
}

impl<T> Future for JoinHandle<T> {
  type Output = Result<T, JoinError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match &mut self.get_mut().0 {
      JoinHandleInner::Task(task) => Pin::new(task).poll(cx),
      #[cfg(not(target_family = "wasm"))]
      JoinHandleInner::Blocking(receiver) => match Pin::new(receiver).poll(cx) {
        Poll::Ready(Ok(result)) => Poll::Ready(result),
        Poll::Ready(Err(_)) => Poll::Ready(Err(JoinError {
          message: "async runtime stopped before the blocking task completed".to_string(),
        })),
        Poll::Pending => Poll::Pending,
      },
      JoinHandleInner::Ready(result) => {
        Poll::Ready(result.take().expect("JoinHandle polled after completion"))
      }
    }
  }
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeMetricsSnapshot {
  pub flavor: RuntimeFlavor,
  pub worker_threads: usize,
  pub max_blocking_tasks: usize,
  pub tasks_spawned: u64,
  pub tasks_completed: u64,
  pub tasks_panicked: u64,
  pub runnable_schedules: u64,
  pub runnable_polls: u64,
  pub queued_runnables: u64,
  pub max_queued_runnables: u64,
  pub active_runnables: u64,
  pub max_active_runnables: u64,
  pub blocking_tasks_started: u64,
  pub blocking_tasks_completed: u64,
  pub active_blocking_tasks: u64,
  pub max_active_blocking_tasks: u64,
}

#[derive(Default)]
struct RuntimeMetrics {
  tasks_spawned: AtomicU64,
  tasks_completed: AtomicU64,
  tasks_panicked: AtomicU64,
  runnable_schedules: AtomicU64,
  runnable_polls: AtomicU64,
  queued_runnables: AtomicU64,
  max_queued_runnables: AtomicU64,
  active_runnables: AtomicU64,
  max_active_runnables: AtomicU64,
  blocking_tasks_started: AtomicU64,
  blocking_tasks_completed: AtomicU64,
  active_blocking_tasks: AtomicU64,
  max_active_blocking_tasks: AtomicU64,
}

impl RuntimeMetrics {
  fn runnable_scheduled(&self) {
    self.runnable_schedules.fetch_add(1, Ordering::Relaxed);
    let queued = self.queued_runnables.fetch_add(1, Ordering::Relaxed) + 1;
    self.max_queued_runnables.fetch_max(queued, Ordering::Relaxed);
  }

  fn runnable_started(&self) -> ActiveRunnableGuard<'_> {
    self.queued_runnables.fetch_sub(1, Ordering::Relaxed);
    self.runnable_polls.fetch_add(1, Ordering::Relaxed);
    let active = self.active_runnables.fetch_add(1, Ordering::Relaxed) + 1;
    self.max_active_runnables.fetch_max(active, Ordering::Relaxed);
    ActiveRunnableGuard { metrics: self }
  }

  fn blocking_started(self: &Arc<Self>) -> ActiveBlockingGuard {
    self.blocking_tasks_started.fetch_add(1, Ordering::Relaxed);
    let active = self.active_blocking_tasks.fetch_add(1, Ordering::Relaxed) + 1;
    self.max_active_blocking_tasks.fetch_max(active, Ordering::Relaxed);
    ActiveBlockingGuard { metrics: Arc::clone(self) }
  }

  fn reset(&self) {
    self.tasks_spawned.store(0, Ordering::Relaxed);
    self.tasks_completed.store(0, Ordering::Relaxed);
    self.tasks_panicked.store(0, Ordering::Relaxed);
    self.runnable_schedules.store(0, Ordering::Relaxed);
    self.runnable_polls.store(0, Ordering::Relaxed);
    self.queued_runnables.store(0, Ordering::Relaxed);
    self.max_queued_runnables.store(0, Ordering::Relaxed);
    self.active_runnables.store(0, Ordering::Relaxed);
    self.max_active_runnables.store(0, Ordering::Relaxed);
    self.blocking_tasks_started.store(0, Ordering::Relaxed);
    self.blocking_tasks_completed.store(0, Ordering::Relaxed);
    self.active_blocking_tasks.store(0, Ordering::Relaxed);
    self.max_active_blocking_tasks.store(0, Ordering::Relaxed);
  }
}

struct ActiveRunnableGuard<'a> {
  metrics: &'a RuntimeMetrics,
}

impl Drop for ActiveRunnableGuard<'_> {
  fn drop(&mut self) {
    self.metrics.active_runnables.fetch_sub(1, Ordering::Relaxed);
  }
}

struct ActiveBlockingGuard {
  metrics: Arc<RuntimeMetrics>,
}

impl Drop for ActiveBlockingGuard {
  fn drop(&mut self) {
    self.metrics.active_blocking_tasks.fetch_sub(1, Ordering::Relaxed);
    self.metrics.blocking_tasks_completed.fetch_add(1, Ordering::Relaxed);
  }
}

fn run_runnable(metrics: &RuntimeMetrics, runnable: Runnable) {
  let _active = metrics.runnable_started();
  let _ = catch_unwind(AssertUnwindSafe(|| runnable.run()));
}

struct CurrentThreadExecutor {
  queue: Mutex<VecDeque<Runnable>>,
  draining: AtomicBool,
  metrics: Arc<RuntimeMetrics>,
}

impl CurrentThreadExecutor {
  fn new(metrics: Arc<RuntimeMetrics>) -> Self {
    Self { queue: Mutex::new(VecDeque::new()), draining: AtomicBool::new(false), metrics }
  }

  fn schedule(self: &Arc<Self>, runnable: Runnable) {
    self.metrics.runnable_scheduled();
    self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_back(runnable);
    self.drain();
  }

  fn drain(self: &Arc<Self>) {
    if self.draining.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
      return;
    }

    loop {
      while self.drain_one() {}
      self.draining.store(false, Ordering::Release);

      let has_more =
        !self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty();
      if !has_more
        || self
          .draining
          .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
          .is_err()
      {
        return;
      }
    }
  }

  fn drain_one(&self) -> bool {
    let runnable = self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).pop_front();
    if let Some(runnable) = runnable {
      run_runnable(&self.metrics, runnable);
      true
    } else {
      false
    }
  }

  fn block_on(self: &Arc<Self>, future: Pin<&mut dyn Future<Output = ()>>) {
    futures::executor::block_on(DriveCurrentThread { executor: Arc::clone(self), future });
  }
}

struct DriveCurrentThread<'a> {
  executor: Arc<CurrentThreadExecutor>,
  future: Pin<&'a mut dyn Future<Output = ()>>,
}

impl Future for DriveCurrentThread<'_> {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    loop {
      if self.future.as_mut().poll(cx).is_ready() {
        return Poll::Ready(());
      }
      if !self.executor.drain_one() {
        return Poll::Pending;
      }
    }
  }
}

// Set while a pool worker is draining runnables/blocking jobs. Used by
// `MultiThreadExecutor::block_on` to detect a re-entrant (on-pool) call and
// drive the queue cooperatively instead of parking the worker (RD-1).
#[cfg(not(target_family = "wasm"))]
thread_local! {
  static ON_POOL_WORKER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(not(target_family = "wasm"))]
struct OnPoolWorkerGuard(bool);

#[cfg(not(target_family = "wasm"))]
impl OnPoolWorkerGuard {
  fn enter() -> Self {
    Self(ON_POOL_WORKER.with(|flag| flag.replace(true)))
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for OnPoolWorkerGuard {
  fn drop(&mut self) {
    ON_POOL_WORKER.with(|flag| flag.set(self.0));
  }
}

// Identifies one specific counted-blocking owner frame on one specific executor.
//
// `executor_id` is assigned once per `MultiThreadExecutor` (so a stale token left
// on a thread by a shut-down executor can never authorize work on a replacement
// executor); `frame` is unique per blocking-closure entry (so nested owners on
// the same thread can never run each other's queued jobs). Only the owner of a
// frame may run *that frame's own* descendant blocking job over the cap in a
// re-entrant `block_on` -- it is the only thread that can unblock the inner job
// it awaits, which is the genuine nested-blocking deadlock (RD-1 (A)). A plain
// runnable driver owns no frame, so it must respect `max_blocking` and park/drive
// instead of running extra blocking work over the cap (RD-1 (B)).
//
// LOAD-BEARING INVARIANT: no `spawn_blocking` closure in rolldown calls `block_on`
// today, so the owner over-cap escape below is defensive. If that ever changes,
// this token machinery (tag at schedule time + token-matched, executor-scoped
// escape) is what keeps the cap correct and prevents an owner from running an
// unrelated queued job over the cap.
#[cfg(not(target_family = "wasm"))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BlockingOwnerToken {
  executor_id: u64,
  frame: u64,
}

// Process-global id source for executors (B: executor-scoping) and for owner
// frames (B: per-frame isolation of the over-cap escape).
#[cfg(not(target_family = "wasm"))]
static NEXT_EXECUTOR_ID: AtomicU64 = AtomicU64::new(0);
#[cfg(not(target_family = "wasm"))]
static NEXT_BLOCKING_FRAME: AtomicU64 = AtomicU64::new(0);

// The owner frame (if any) currently running a counted blocking closure on THIS
// thread. Replaces the old ambient `OWNS_BLOCKING_SLOT` bool.
#[cfg(not(target_family = "wasm"))]
thread_local! {
  static BLOCKING_OWNER: std::cell::Cell<Option<BlockingOwnerToken>> =
    const { std::cell::Cell::new(None) };
}

// Save/replace/restore guard for `BLOCKING_OWNER`. Enter with `Some(token)` while
// running a counted blocking closure, or with `None` while a cooperative driver
// runs a plain queued runnable (ownership is LEXICAL to the blocking closure: a
// runnable an owner happens to drive via `run_one` is a logical non-owner and
// must not inherit the owner's over-cap privilege). Restored on drop, so the
// pattern is panic-safe even though `run_runnable` already catches panics.
#[cfg(not(target_family = "wasm"))]
struct BlockingOwnerGuard(Option<BlockingOwnerToken>);

#[cfg(not(target_family = "wasm"))]
impl BlockingOwnerGuard {
  fn enter(token: Option<BlockingOwnerToken>) -> Self {
    Self(BLOCKING_OWNER.with(|cell| cell.replace(token)))
  }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for BlockingOwnerGuard {
  fn drop(&mut self) {
    BLOCKING_OWNER.with(|cell| cell.set(self.0));
  }
}

// A blocking job queued for the pool, tagged with the owner frame that scheduled
// it (if it was scheduled while a counted blocking closure of THIS executor was
// active on the stack). The tag links an owner's awaited inner job to the owner,
// so the over-cap escape can run THAT job and only that job (RD-1 (B)).
#[cfg(not(target_family = "wasm"))]
struct QueuedBlocking {
  owner: Option<BlockingOwnerToken>,
  run: Box<dyn FnOnce() + Send + 'static>,
}

#[cfg(not(target_family = "wasm"))]
struct MultiThreadExecutor {
  // Stable per-executor id; tags owner tokens so a stale token from a shut-down
  // executor can never authorize an over-cap escape on a replacement (RD-1 (B)).
  id: u64,
  pool: ThreadPool,
  queue: Mutex<VecDeque<Runnable>>,
  blocking_queue: Mutex<VecDeque<QueuedBlocking>>,
  active_drainers: AtomicUsize,
  active_blocking: AtomicUsize,
  // Wakes pool workers that are parked inside a re-entrant cooperative
  // `block_on` when new queue work is scheduled or an awaited future is woken.
  // The pool has a fixed number of OS threads, so a parked worker cannot rely
  // on `ensure_drainer` spawning a replacement (there is no free thread to run
  // it) -- it must wake and run the work itself (RD-1).
  coop_signal: Arc<CoopSignal>,
  max_drainers: usize,
  max_blocking: usize,
  metrics: Arc<RuntimeMetrics>,
}

#[cfg(not(target_family = "wasm"))]
impl MultiThreadExecutor {
  fn new(
    options: &RuntimeOptions,
    metrics: Arc<RuntimeMetrics>,
  ) -> Result<Self, RuntimeConfigError> {
    let thread_name_prefix = options.thread_name_prefix.clone();
    let pool = ThreadPoolBuilder::new()
      .num_threads(options.worker_threads)
      .thread_name(move |index| format!("{thread_name_prefix}-{index}"))
      .build()
      .map_err(|error| RuntimeConfigError(format!("failed to create runtime workers: {error}")))?;
    Ok(Self {
      id: NEXT_EXECUTOR_ID.fetch_add(1, Ordering::Relaxed),
      pool,
      queue: Mutex::new(VecDeque::new()),
      blocking_queue: Mutex::new(VecDeque::new()),
      active_drainers: AtomicUsize::new(0),
      active_blocking: AtomicUsize::new(0),
      coop_signal: Arc::new(CoopSignal::default()),
      max_drainers: options.worker_threads,
      max_blocking: options.max_blocking_tasks.min(options.worker_threads),
      metrics,
    })
  }

  fn schedule(self: &Arc<Self>, runnable: Runnable) {
    self.metrics.runnable_scheduled();
    self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_back(runnable);
    // Wake any cooperative `block_on` driver that is parked waiting for work,
    // then make sure a (fresh) drainer exists for the normal case (RD-1).
    self.coop_signal.notify();
    self.ensure_drainer();
  }

  fn schedule_blocking<F, T>(self: &Arc<Self>, function: F) -> JoinHandle<T>
  where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
  {
    let (sender, receiver) = oneshot::channel();
    let metrics = Arc::clone(&self.metrics);
    // Tag the job with the current owner frame, but ONLY if that frame belongs to
    // THIS executor. The inner job an owner awaits is scheduled while the owner
    // frame is active on the stack, so it inherits the owner's exact token and is
    // the one job the over-cap escape is allowed to run. A job scheduled by a
    // non-owner -- or under a stale token from a different executor -- is an
    // ordinary capped job (`None`) (RD-1 (B)).
    let owner =
      BLOCKING_OWNER.with(std::cell::Cell::get).filter(|token| token.executor_id == self.id);
    self.blocking_queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_back(
      QueuedBlocking {
        owner,
        run: Box::new(move || {
          let _active = metrics.blocking_started();
          let result = catch_unwind(AssertUnwindSafe(function))
            .map_err(|panic| JoinError::from_panic(&*panic));
          let _ = sender.send(result);
        }),
      },
    );
    // Wake a parked cooperative driver so it can run this blocking job inline if
    // every blocking slot is held by parked drivers (RD-1 (b)).
    self.coop_signal.notify();
    self.ensure_drainer();
    JoinHandle(JoinHandleInner::Blocking(receiver))
  }

  fn ensure_drainer(self: &Arc<Self>) {
    loop {
      let active = self.active_drainers.load(Ordering::Acquire);
      if active >= self.max_drainers {
        return;
      }
      if self
        .active_drainers
        .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
      {
        let executor = Arc::clone(self);
        self.pool.spawn_fifo(move || executor.drain());
        return;
      }
    }
  }

  fn drain(self: Arc<Self>) {
    const RUNNABLE_BUDGET: usize = 64;

    // Mark this worker so a re-entrant `block_on` (reached from a polled task)
    // drives the queue cooperatively instead of parking the worker.
    let _on_pool = OnPoolWorkerGuard::enter();

    for _ in 0..RUNNABLE_BUDGET {
      let runnable =
        self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).pop_front();
      if let Some(runnable) = runnable {
        run_runnable(&self.metrics, runnable);
        continue;
      }
      if let Some(blocking) = self.take_blocking() {
        {
          let _owner = BlockingOwnerGuard::enter(Some(self.fresh_owner_token()));
          blocking();
        }
        self.active_blocking.fetch_sub(1, Ordering::Release);
        continue;
      }
      self.finish_draining();
      return;
    }

    self.finish_draining();
  }

  /// Mint a fresh owner token for a counted blocking closure entering on this
  /// executor. Each entry gets a unique `frame` so nested owners on the same
  /// thread can never run each other's queued jobs over the cap (RD-1 (B)).
  fn fresh_owner_token(&self) -> BlockingOwnerToken {
    BlockingOwnerToken {
      executor_id: self.id,
      frame: NEXT_BLOCKING_FRAME.fetch_add(1, Ordering::Relaxed),
    }
  }

  fn finish_draining(self: &Arc<Self>) {
    self.active_drainers.fetch_sub(1, Ordering::AcqRel);
    let has_runnable =
      !self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty();
    let has_blocking = self.active_blocking.load(Ordering::Acquire) < self.max_blocking
      && !self.blocking_queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty();
    if has_runnable || has_blocking {
      self.ensure_drainer();
    }
  }

  fn block_on(self: &Arc<Self>, future: Pin<&mut dyn Future<Output = ()>>) {
    if ON_POOL_WORKER.with(std::cell::Cell::get) {
      // Re-entrant call from a pool worker: parking here would freeze a worker
      // that the awaited future may itself need to make progress. Drive the
      // queue cooperatively instead (mirrors the CurrentThread drive loop).
      self.cooperative_block_on(future);
    } else {
      // Non-pool (e.g. napi caller) thread: keep parking — there is no pool
      // worker to starve, and other workers continue draining as usual.
      futures::executor::block_on(future);
    }
  }

  /// Run a single unit of queued work (a runnable, else a blocking job).
  /// Returns `true` if work was performed. Mirrors the body of `drain`.
  fn run_one(self: &Arc<Self>) -> bool {
    let runnable = self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).pop_front();
    if let Some(runnable) = runnable {
      // Ownership is lexical to the blocking closure: clear the owner frame for
      // the duration of this runnable so a nested `block_on` inside it is treated
      // as a non-owner and cannot borrow the driving owner's over-cap privilege
      // (RD-1 (B)). `run_runnable` catches panics; the guard also restores the
      // previous owner on drop, so this is panic-safe regardless.
      let _non_owner = BlockingOwnerGuard::enter(None);
      run_runnable(&self.metrics, runnable);
      return true;
    }
    if let Some(blocking) = self.take_blocking() {
      {
        let _owner = BlockingOwnerGuard::enter(Some(self.fresh_owner_token()));
        blocking();
      }
      self.active_blocking.fetch_sub(1, Ordering::Release);
      return true;
    }
    false
  }

  /// Token-matched, cooperative-only fallback for a re-entrant `block_on` that is
  /// about to park: run the queued blocking job that the given owner frame `token`
  /// scheduled, even though `max_blocking` is saturated. It scans for the FIRST
  /// job tagged `Some(token)` and runs ONLY that job -- never an unrelated queued
  /// job (RD-1 (B)): an owner may exceed the cap only to unblock the inner job it
  /// itself awaits, which is the genuine nested-blocking deadlock it must break
  /// (RD-1 (A)). If no job matches it returns `false` WITHOUT touching the queue,
  /// so the caller parks instead.
  ///
  /// This does not add a new concurrent blocking OS thread (the current worker
  /// would otherwise be parked, doing nothing), so it cannot over-subscribe real
  /// parallelism; it therefore does not touch `active_blocking`. The inline job
  /// runs under its OWN fresh owner frame so that a further nested `block_on` it
  /// performs tags its descendants correctly.
  fn run_owned_blocking_over_cap(self: &Arc<Self>, token: BlockingOwnerToken) -> bool {
    let job = {
      let mut queue = self.blocking_queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      match queue.iter().position(|queued| queued.owner == Some(token)) {
        Some(index) => queue.remove(index),
        None => None,
      }
    };
    if let Some(job) = job {
      let _owner = BlockingOwnerGuard::enter(Some(self.fresh_owner_token()));
      (job.run)();
      true
    } else {
      false
    }
  }

  /// Cooperative `block_on` for a re-entrant call from a pool worker. Instead
  /// of parking the worker (which would freeze one of the pool's fixed OS
  /// threads), it drives the awaited future while servicing queued work. When
  /// nothing is immediately runnable it parks on `coop_signal`, which is woken
  /// both by the awaited future's waker and by `schedule`/`schedule_blocking`,
  /// so a later wake reaches this worker even when every worker is parked and
  /// `ensure_drainer` has no free thread to spawn a replacement (RD-1).
  fn cooperative_block_on(self: &Arc<Self>, mut future: Pin<&mut dyn Future<Output = ()>>) {
    let signal = Arc::clone(&self.coop_signal);
    let waker = std::task::Waker::from(Arc::clone(&signal));
    let mut cx = Context::from_waker(&waker);
    loop {
      // Capture the wake generation *before* polling/draining so a wake that
      // races with this iteration is observed and we do not park stale.
      let generation = signal.generation();
      if future.as_mut().poll(&mut cx).is_ready() {
        return;
      }
      if self.run_one() {
        continue;
      }
      // Only a worker that owns a counted blocking frame OF THIS EXECUTOR may run
      // a queued blocking job over the cap, and only the job that frame itself
      // scheduled (the genuine nested-blocking case it must unblock). A plain
      // runnable driver owns no frame; a stale token from another executor fails
      // the `executor_id` check -- both respect `max_blocking` and park/drive
      // instead of starting extra blocking work (RD-1 (B)).
      if let Some(token) = BLOCKING_OWNER.with(std::cell::Cell::get) {
        if token.executor_id == self.id && self.run_owned_blocking_over_cap(token) {
          continue;
        }
      }
      // Nothing runnable right now: wait until new work is scheduled or the
      // awaited future is woken.
      signal.wait_if_unchanged(generation);
    }
  }

  /// Pop the next queued blocking job FIFO within the cap, ignoring its owner tag
  /// (any worker may run any job *within* `max_blocking`; only the over-cap escape
  /// is tag-restricted). Returns the runnable closure, dropping the tag.
  fn take_blocking(&self) -> Option<Box<dyn FnOnce() + Send + 'static>> {
    loop {
      let active = self.active_blocking.load(Ordering::Acquire);
      if active >= self.max_blocking {
        return None;
      }
      let mut queue = self.blocking_queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      if queue.is_empty() {
        return None;
      }
      if self
        .active_blocking
        .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok()
      {
        return queue.pop_front().map(|queued| queued.run);
      }
    }
  }
}

// Wake primitive for cooperative re-entrant `block_on` drivers parked on a pool
// worker. `notify` bumps a generation counter and wakes every parked driver,
// which then re-polls its future and re-checks the queue. Used both as the
// awaited future's `Waker` and as the signal raised by `schedule` /
// `schedule_blocking` so newly queued work reaches a parked worker (RD-1).
#[cfg(not(target_family = "wasm"))]
#[derive(Default)]
struct CoopSignal {
  generation: Mutex<u64>,
  condvar: std::sync::Condvar,
}

#[cfg(not(target_family = "wasm"))]
impl CoopSignal {
  fn generation(&self) -> u64 {
    *self.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  fn notify(&self) {
    {
      let mut generation =
        self.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      *generation = generation.wrapping_add(1);
    }
    self.condvar.notify_all();
  }

  /// Park until the generation moves past `observed`. If a `notify` already
  /// raced ahead (generation changed) this returns immediately, preventing a
  /// lost wakeup between the caller's last queue check and this park.
  fn wait_if_unchanged(&self, observed: u64) {
    let generation = self.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if *generation == observed {
      let guard = self.condvar.wait(generation).unwrap_or_else(std::sync::PoisonError::into_inner);
      drop(guard);
    }
  }
}

#[cfg(not(target_family = "wasm"))]
impl std::task::Wake for CoopSignal {
  fn wake(self: Arc<Self>) {
    self.notify();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.notify();
  }
}

#[derive(Clone)]
enum RuntimeBackend {
  CurrentThread(Arc<CurrentThreadExecutor>),
  #[cfg(not(target_family = "wasm"))]
  MultiThread(Arc<MultiThreadExecutor>),
}

impl RuntimeBackend {
  fn new(
    options: &RuntimeOptions,
    metrics: Arc<RuntimeMetrics>,
  ) -> Result<Self, RuntimeConfigError> {
    match options.flavor {
      RuntimeFlavor::CurrentThread => {
        Ok(Self::CurrentThread(Arc::new(CurrentThreadExecutor::new(metrics))))
      }
      RuntimeFlavor::MultiThread => {
        #[cfg(not(target_family = "wasm"))]
        {
          Ok(Self::MultiThread(Arc::new(MultiThreadExecutor::new(options, metrics)?)))
        }
        #[cfg(target_family = "wasm")]
        {
          let _ = metrics;
          Err(RuntimeConfigError(
            "the multi-thread runtime is unavailable in this WebAssembly build".to_string(),
          ))
        }
      }
    }
  }

  fn schedule(&self, runnable: Runnable) {
    match self {
      Self::CurrentThread(executor) => executor.schedule(runnable),
      #[cfg(not(target_family = "wasm"))]
      Self::MultiThread(executor) => executor.schedule(runnable),
    }
  }

  fn spawn_blocking<F, T>(&self, function: F, metrics: &Arc<RuntimeMetrics>) -> JoinHandle<T>
  where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
  {
    match self {
      Self::CurrentThread(_) => {
        let _active = metrics.blocking_started();
        let result =
          catch_unwind(AssertUnwindSafe(function)).map_err(|panic| JoinError::from_panic(&*panic));
        JoinHandle(JoinHandleInner::Ready(Some(result)))
      }
      #[cfg(not(target_family = "wasm"))]
      Self::MultiThread(executor) => {
        let _ = metrics;
        executor.schedule_blocking(function)
      }
    }
  }

  fn block_on(&self, future: Pin<&mut dyn Future<Output = ()>>) {
    match self {
      Self::CurrentThread(executor) => executor.block_on(future),
      #[cfg(not(target_family = "wasm"))]
      Self::MultiThread(executor) => executor.block_on(future),
    }
  }
}

struct RuntimeState {
  options: RuntimeOptions,
  backend: Option<RuntimeBackend>,
}

struct RuntimeController {
  state: Mutex<RuntimeState>,
  metrics: Arc<RuntimeMetrics>,
}

impl RuntimeController {
  fn new() -> Self {
    Self {
      state: Mutex::new(RuntimeState { options: RuntimeOptions::default(), backend: None }),
      metrics: Arc::new(RuntimeMetrics::default()),
    }
  }

  fn configure(&self, options: RuntimeOptions) -> Result<(), RuntimeConfigError> {
    let options = options.validate()?;
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.backend.is_some() {
      return Err(RuntimeConfigError(
        "the async runtime is already running; configure it before the first async call"
          .to_string(),
      ));
    }
    state.options = options;
    Ok(())
  }

  fn backend(&self) -> RuntimeBackend {
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(backend) = &state.backend {
      return backend.clone();
    }
    let backend = RuntimeBackend::new(&state.options, Arc::clone(&self.metrics))
      .expect("validated async runtime configuration must create a backend");
    state.backend = Some(backend.clone());
    backend
  }

  fn options(&self) -> RuntimeOptions {
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).options.clone()
  }

  fn shutdown(&self) {
    let backend =
      self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).backend.take();
    drop(backend);
  }
}

static RUNTIME: LazyLock<RuntimeController> = LazyLock::new(RuntimeController::new);

pub fn configure(options: RuntimeOptions) -> Result<(), RuntimeConfigError> {
  RUNTIME.configure(options)
}

pub fn configured_options() -> RuntimeOptions {
  RUNTIME.options()
}

pub fn is_multi_threaded() -> bool {
  RUNTIME.options().flavor == RuntimeFlavor::MultiThread
}

pub fn spawn<F, T>(future: F) -> JoinHandle<T>
where
  F: Future<Output = T> + Send + 'static,
  T: Send + 'static,
{
  let backend = RUNTIME.backend();
  let metrics = Arc::clone(&RUNTIME.metrics);
  metrics.tasks_spawned.fetch_add(1, Ordering::Relaxed);
  let wrapped = async move {
    let result =
      AssertUnwindSafe(future).catch_unwind().await.map_err(|panic| JoinError::from_panic(&*panic));
    if result.is_ok() {
      metrics.tasks_completed.fetch_add(1, Ordering::Relaxed);
    } else {
      metrics.tasks_panicked.fetch_add(1, Ordering::Relaxed);
    }
    result
  };
  let scheduler = backend.clone();
  let (runnable, task) = async_task::spawn(wrapped, move |runnable| {
    scheduler.schedule(runnable);
  });
  backend.schedule(runnable);
  JoinHandle(JoinHandleInner::Task(task))
}

pub fn spawn_detached<F>(future: F)
where
  F: Future<Output = ()> + Send + 'static,
{
  spawn(future).detach();
}

pub fn spawn_blocking<F, T>(function: F) -> JoinHandle<T>
where
  F: FnOnce() -> T + Send + 'static,
  T: Send + 'static,
{
  let backend = RUNTIME.backend();
  let metrics = Arc::clone(&RUNTIME.metrics);
  backend.spawn_blocking(function, &metrics)
}

pub fn block_on<F: Future>(future: F) -> F::Output {
  let mut output = None;
  {
    let mut erased = std::pin::pin!(async {
      output = Some(future.await);
    });
    block_on_dyn(erased.as_mut());
  }
  output.expect("async runtime returned before the future completed")
}

pub fn block_on_dyn(future: Pin<&mut dyn Future<Output = ()>>) {
  RUNTIME.backend().block_on(future);
}

pub fn shutdown() {
  RUNTIME.shutdown();
}

pub fn reset_metrics() {
  RUNTIME.metrics.reset();
}

pub fn metrics() -> RuntimeMetricsSnapshot {
  let options = RUNTIME.options();
  RuntimeMetricsSnapshot {
    flavor: options.flavor,
    worker_threads: options.worker_threads,
    max_blocking_tasks: options.max_blocking_tasks,
    tasks_spawned: RUNTIME.metrics.tasks_spawned.load(Ordering::Relaxed),
    tasks_completed: RUNTIME.metrics.tasks_completed.load(Ordering::Relaxed),
    tasks_panicked: RUNTIME.metrics.tasks_panicked.load(Ordering::Relaxed),
    runnable_schedules: RUNTIME.metrics.runnable_schedules.load(Ordering::Relaxed),
    runnable_polls: RUNTIME.metrics.runnable_polls.load(Ordering::Relaxed),
    queued_runnables: RUNTIME.metrics.queued_runnables.load(Ordering::Relaxed),
    max_queued_runnables: RUNTIME.metrics.max_queued_runnables.load(Ordering::Relaxed),
    active_runnables: RUNTIME.metrics.active_runnables.load(Ordering::Relaxed),
    max_active_runnables: RUNTIME.metrics.max_active_runnables.load(Ordering::Relaxed),
    blocking_tasks_started: RUNTIME.metrics.blocking_tasks_started.load(Ordering::Relaxed),
    blocking_tasks_completed: RUNTIME.metrics.blocking_tasks_completed.load(Ordering::Relaxed),
    active_blocking_tasks: RUNTIME.metrics.active_blocking_tasks.load(Ordering::Relaxed),
    max_active_blocking_tasks: RUNTIME.metrics.max_active_blocking_tasks.load(Ordering::Relaxed),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn current_thread_executor_drives_spawned_tasks() {
    let metrics = Arc::new(RuntimeMetrics::default());
    let executor = Arc::new(CurrentThreadExecutor::new(Arc::clone(&metrics)));
    let scheduler = Arc::clone(&executor);
    let (runnable, task) = async_task::spawn(async { 42 }, move |runnable| {
      scheduler.schedule(runnable);
    });

    executor.schedule(runnable);

    assert_eq!(futures::executor::block_on(task), 42);
    assert_eq!(metrics.runnable_polls.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.active_runnables.load(Ordering::Relaxed), 0);
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_block_on_does_not_park_pool_worker() {
    // Regression for RD-1: a pool-worker task that calls `block_on` on an inner
    // future depending on pool-scheduled blocking work must not park its worker.
    // With `worker_threads` tasks each parking a worker, the blocking jobs they
    // await can never be picked up by a drainer -> deadlock under the old code.
    use std::sync::{Barrier, mpsc};
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd1-test".to_string(),
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

      let task_count = 2usize;
      // Guarantees every task is being polled on its own pool worker *before*
      // any of them parks in `block_on`, so the deadlock is deterministic.
      let barrier = Arc::new(Barrier::new(task_count));

      let mut tasks = Vec::new();
      for _ in 0..task_count {
        let exec = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);
        let body = async move {
          barrier.wait();
          let inner_exec = Arc::clone(&exec);
          let mut inner = std::pin::pin!(async move {
            let value = inner_exec.schedule_blocking(|| 7usize).await.unwrap();
            assert_eq!(value, 7);
          });
          // Synchronous `block_on` from inside a pool-worker task: the vector.
          exec.block_on(inner.as_mut());
        };
        let scheduler = Arc::clone(&executor);
        let (runnable, task) =
          async_task::spawn(body, move |runnable| scheduler.schedule(runnable));
        executor.schedule(runnable);
        tasks.push(task);
      }

      for task in tasks {
        futures::executor::block_on(task);
      }
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "MultiThread block_on deadlocked ({error}): pool workers parked waiting on pool-scheduled work"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_block_on_from_caller_thread_still_parks() {
    // The non-pool (napi caller) path must keep using the plain parking
    // `block_on`; here we just assert it drives a ready future to completion.
    let metrics = Arc::new(RuntimeMetrics::default());
    let options = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "rd1-caller".to_string(),
    };
    let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

    let mut output = None;
    let mut future = std::pin::pin!(async {
      output = Some(41usize + 1);
    });
    executor.block_on(future.as_mut());
    assert_eq!(output, Some(42));
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_block_on_wakes_parked_driver_after_workers_park() {
    // Regression for RD-1 finding (a): a pool-worker task parks in `block_on`
    // because the work that will wake it has not been scheduled yet. Once every
    // pool worker is parked this way, a later async wake routes through
    // `schedule()` -> `ensure_drainer`, which (under the old code) refuses to
    // spawn a replacement because the parked workers are still counted as active
    // drainers -> the enqueued runnable never runs -> deadlock.
    use std::sync::{Barrier, mpsc};
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd1-park".to_string(),
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

      let task_count = 2usize;
      let barrier = Arc::new(Barrier::new(task_count));

      let mut senders = Vec::new();
      let mut tasks = Vec::new();
      for _ in 0..task_count {
        let (tx, rx) = oneshot::channel::<usize>();
        senders.push(tx);
        let exec = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);
        let body = async move {
          // Make sure both pool workers are occupied before either parks.
          barrier.wait();
          // Child task whose wake routes through `schedule()`: the oneshot
          // receiver's waker -> child runnable -> `schedule` -> `ensure_drainer`.
          let child_exec = Arc::clone(&exec);
          let (child_runnable, child_task) =
            async_task::spawn(async move { rx.await.unwrap() }, move |r| child_exec.schedule(r));
          exec.schedule(child_runnable);
          let mut inner = std::pin::pin!(async move {
            assert_eq!(child_task.await, 9usize);
          });
          exec.block_on(inner.as_mut());
        };
        let scheduler = Arc::clone(&executor);
        let (runnable, task) = async_task::spawn(body, move |r| scheduler.schedule(r));
        executor.schedule(runnable);
        tasks.push(task);
      }

      // Give both workers time to poll their child Pending and park in block_on.
      std::thread::sleep(Duration::from_millis(200));
      for tx in senders {
        let _ = tx.send(9usize);
      }

      for task in tasks {
        futures::executor::block_on(task);
      }
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "RD-1 (a): parked pool drivers were not woken after a post-park schedule() ({error})"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_nested_spawn_blocking_does_not_deadlock_when_saturated() {
    // Regression for RD-1 finding (b): a blocking closure re-enters `block_on`
    // and awaits another `spawn_blocking` while `max_blocking_tasks` is already
    // saturated by the outer blocking jobs. Under the old code the driver parks
    // (take_blocking returns None) while still holding its blocking slot, so the
    // inner job can never run -> deadlock.
    use std::sync::{Barrier, mpsc};
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd1-nested-blk".to_string(),
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

      let outer_count = 2usize; // saturates max_blocking_tasks
      let barrier = Arc::new(Barrier::new(outer_count));

      let mut handles = Vec::new();
      for _ in 0..outer_count {
        let exec = Arc::clone(&executor);
        let barrier = Arc::clone(&barrier);
        let handle = executor.schedule_blocking(move || {
          // Both outer blocking jobs hold a slot before either nests.
          barrier.wait();
          let inner_exec = Arc::clone(&exec);
          let mut value = 0usize;
          {
            let mut inner = std::pin::pin!(async {
              value = inner_exec.schedule_blocking(|| 5usize).await.unwrap();
            });
            exec.block_on(inner.as_mut());
          }
          value
        });
        handles.push(handle);
      }

      for handle in handles {
        assert_eq!(futures::executor::block_on(handle).unwrap(), 5usize);
      }
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("RD-1 (b): nested spawn_blocking deadlocked while saturated ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_block_on_respects_blocking_cap_for_non_owner_driver() {
    // Regression for RD-1 round-2 finding: a plain pool-worker runnable driver
    // (one that does NOT own a counted blocking slot) re-entering `block_on`
    // must NOT run queued blocking jobs over `max_blocking_tasks` via the
    // cooperative over-cap fallback. Two holder blocking jobs saturate the cap;
    // two driver tasks then `block_on(spawn_blocking(..))`. With the buggy
    // over-cap fallback the drivers run their inner blocking jobs immediately
    // while the holders still occupy both slots, so peak active blocking >= 3
    // (> cap 2). The cap must be honored AND everything must still complete.
    use std::sync::{Barrier, mpsc};
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let peak_out = Arc::new(Mutex::new(None));
    let peak_slot = Arc::clone(&peak_out);
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 4,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd1-cap".to_string(),
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, Arc::clone(&metrics)).unwrap());

      let holder_count = 2usize; // saturates max_blocking_tasks
      let driver_count = 2usize;

      // Holders signal once they hold a counted slot, then wait to be released.
      let (held_tx, held_rx) = mpsc::channel();
      let release = Arc::new(Barrier::new(holder_count + 1));

      let mut holder_handles = Vec::new();
      for _ in 0..holder_count {
        let held_tx = held_tx.clone();
        let release = Arc::clone(&release);
        let handle = executor.schedule_blocking(move || {
          held_tx.send(()).unwrap();
          release.wait();
          0usize
        });
        holder_handles.push(handle);
      }
      // Wait until both holders hold their slots (cap fully saturated).
      for _ in 0..holder_count {
        held_rx.recv().unwrap();
      }

      // Driver tasks: plain runnables (NOT blocking-slot owners) that block_on
      // an inner spawn_blocking.
      let mut driver_tasks = Vec::new();
      for _ in 0..driver_count {
        let exec = Arc::clone(&executor);
        let body = async move {
          let inner_exec = Arc::clone(&exec);
          let mut inner = std::pin::pin!(async move {
            let v = inner_exec.schedule_blocking(|| 5usize).await.unwrap();
            assert_eq!(v, 5);
          });
          exec.block_on(inner.as_mut());
        };
        let scheduler = Arc::clone(&executor);
        let (runnable, task) = async_task::spawn(body, move |r| scheduler.schedule(r));
        executor.schedule(runnable);
        driver_tasks.push(task);
      }

      // Let the driver tasks reach block_on. Under the buggy over-cap fallback
      // they run their inner blocking jobs NOW (over cap); under the fix they
      // park until a holder frees a slot.
      std::thread::sleep(Duration::from_millis(300));

      // Release the holders so the inner jobs can run within the cap.
      release.wait();

      for task in driver_tasks {
        futures::executor::block_on(task);
      }
      for handle in holder_handles {
        assert_eq!(futures::executor::block_on(handle).unwrap(), 0usize);
      }

      *peak_slot.lock().unwrap() = Some(metrics.max_active_blocking_tasks.load(Ordering::Relaxed));
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("RD-1 cap test deadlocked ({error})"),
    }
    let peak = peak_out.lock().unwrap().expect("peak captured");
    assert!(peak <= 2, "peak active blocking tasks {peak} exceeded max_blocking_tasks 2");
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_block_on_runnable_driven_by_owner_respects_cap() {
    // Regression for RD-1 round-3 finding: a counted blocking OWNER that re-enters
    // cooperative `block_on` can drive a plain queued runnable via `run_one`. That
    // runnable must NOT inherit the owner's over-cap blocking privilege. If the
    // runnable itself re-enters `block_on` awaiting a `spawn_blocking` while the cap
    // is saturated, the inner job must respect `max_blocking_tasks` (park until a
    // slot frees), not run over cap by borrowing the owner's blocking-owner token.
    //
    // Setup (worker_threads=2, max_blocking=2): owner O (a counted blocking closure)
    // holds slot 1; holder H2 holds slot 2 -> cap saturated. O spawns a plain task R
    // and drives it via cooperative `block_on`. With only 2 workers and 2 active
    // drainers, no replacement drainer can be spawned, so O itself runs R via
    // `run_one`. R re-enters `block_on` awaiting an inner `spawn_blocking` J. Under
    // the bug, R inherits O's slot flag and runs J over cap -> peak metric 3 (> 2).
    // Under the fix, R is a non-owner and parks until H2 frees a slot, so J runs
    // within the cap -> peak 2. Everything must still complete (no deadlock).
    use std::sync::{Barrier, mpsc};
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let peak_out = Arc::new(Mutex::new(None));
    let peak_slot = Arc::clone(&peak_out);
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd1-owner-runnable".to_string(),
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, Arc::clone(&metrics)).unwrap());

      let saturated = Arc::new(Barrier::new(2));
      let release = Arc::new(Barrier::new(2));

      // Holder H2: holds the second counted slot until released. Not the driver.
      let h2_saturated = Arc::clone(&saturated);
      let h2_release = Arc::clone(&release);
      let h2 = executor.schedule_blocking(move || {
        h2_saturated.wait();
        h2_release.wait();
        0usize
      });

      // Owner O: a counted blocking closure (slot 1). Once the cap is saturated it
      // spawns plain runnable R and drives it cooperatively via `block_on`.
      let o_exec = Arc::clone(&executor);
      let o_saturated = Arc::clone(&saturated);
      let o = executor.schedule_blocking(move || {
        o_saturated.wait(); // both slots held -> cap saturated

        let r_exec = Arc::clone(&o_exec);
        let body = async move {
          let inner_exec = Arc::clone(&r_exec);
          let mut inner = std::pin::pin!(async move {
            let v = inner_exec.schedule_blocking(|| 5usize).await.unwrap();
            assert_eq!(v, 5);
          });
          r_exec.block_on(inner.as_mut());
        };
        let sched = Arc::clone(&o_exec);
        let (runnable, task) = async_task::spawn(body, move |r| sched.schedule(r));
        o_exec.schedule(runnable);

        let mut wait_r = std::pin::pin!(async move {
          task.await;
        });
        o_exec.block_on(wait_r.as_mut());
        1usize
      });

      // Give O time to drive R into its inner `block_on`. Under the bug R already
      // ran its inner job over cap; under the fix R is parked waiting for a slot.
      std::thread::sleep(Duration::from_millis(300));
      // Release H2 so the inner job can run within the cap (fix path).
      release.wait();

      assert_eq!(futures::executor::block_on(o).unwrap(), 1usize);
      assert_eq!(futures::executor::block_on(h2).unwrap(), 0usize);

      *peak_slot.lock().unwrap() = Some(metrics.max_active_blocking_tasks.load(Ordering::Relaxed));
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("RD-1 round-3 owner-driven-runnable cap test deadlocked ({error})"),
    }
    let peak = peak_out.lock().unwrap().expect("peak captured");
    assert!(peak <= 2, "peak active blocking tasks {peak} exceeded max_blocking_tasks 2");
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_over_cap_escape_runs_only_owner_job_not_unrelated() {
    // Regression for RD-1 (B) HIGH: the old over-cap fallback blind-popped the
    // FRONT of the shared blocking queue, so an owner re-entering `block_on`
    // could run an ARBITRARY unrelated job over the cap. The token-matched escape
    // must run ONLY the owner frame's own descendant job.
    //
    // Setup (worker_threads=2, max_blocking=2): owner O is a counted blocking
    // closure (slot 1); holder H holds slot 2 -> cap saturated. An UNRELATED job
    // U (scheduled by the main thread, so tagged `None`) is queued AHEAD of O's
    // descendant D. O re-enters `block_on` awaiting D (tagged with O's frame).
    // The escape must skip U and run D. Because U can only run once a REAL slot
    // frees (O completing, after D runs), D STRICTLY precedes U under the fix.
    // The old front-pop ran U first -> order ["U", "D"], which this asserts away.
    use std::sync::{Barrier, mpsc};

    let (done_tx, done_rx) = mpsc::channel();
    let order_out = Arc::new(Mutex::new(Vec::<&'static str>::new()));
    let order_read = Arc::clone(&order_out);
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd1-unrelated".to_string(),
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, Arc::clone(&metrics)).unwrap());

      // O and H signal once they each hold a counted slot (cap saturated), then
      // wait so U is queued before O nests. H stays held until the very end.
      let (held_tx, held_rx) = mpsc::channel();
      let u_ready = Arc::new(Barrier::new(2)); // O <-> main: U is queued
      let h_release = Arc::new(Barrier::new(2)); // H <-> main: end of test

      // Holder H: holds slot 2 for the whole test.
      let h_held = held_tx.clone();
      let h_rel = Arc::clone(&h_release);
      let h = executor.schedule_blocking(move || {
        h_held.send(()).unwrap();
        h_rel.wait();
        0usize
      });

      // Owner O: holds slot 1, then (once U is queued) nests `block_on` awaiting
      // its OWN descendant D.
      let o_exec = Arc::clone(&executor);
      let o_ready = Arc::clone(&u_ready);
      let o_order = Arc::clone(&order_out);
      let o = executor.schedule_blocking(move || {
        held_tx.send(()).unwrap();
        o_ready.wait(); // wait until U is queued ahead of D
        let inner_exec = Arc::clone(&o_exec);
        let d_order = Arc::clone(&o_order);
        let mut inner = std::pin::pin!(async move {
          let v = inner_exec
            .schedule_blocking(move || {
              d_order.lock().unwrap().push("D");
              5usize
            })
            .await
            .unwrap();
          assert_eq!(v, 5);
        });
        o_exec.block_on(inner.as_mut());
        1usize
      });

      // Wait until BOTH O and H hold their slots -> cap saturated.
      held_rx.recv().unwrap();
      held_rx.recv().unwrap();

      // Now queue U (tagged None: scheduled by a non-owner main thread). It sits
      // ahead of D and must NOT be run over the cap by O's escape.
      let u_order = Arc::clone(&order_out);
      let u = executor.schedule_blocking(move || {
        u_order.lock().unwrap().push("U");
        2usize
      });

      // Release O to nest now that U is queued ahead of D.
      u_ready.wait();

      // O makes progress by running D over cap; U only runs after O frees a slot.
      assert_eq!(futures::executor::block_on(o).unwrap(), 1usize);
      assert_eq!(futures::executor::block_on(u).unwrap(), 2usize);

      // Release H and finish.
      h_release.wait();
      assert_eq!(futures::executor::block_on(h).unwrap(), 0usize);

      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(std::time::Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("RD-1 (B) unrelated-job escape test deadlocked ({error})"),
    }
    let order = order_read.lock().unwrap().clone();
    assert_eq!(
      order,
      vec!["D", "U"],
      "owner must run its OWN descendant D over cap; unrelated U must wait for a real slot"
    );
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_over_cap_escape_is_token_and_executor_scoped() {
    // Regression for RD-1 (B) MEDIUM: the over-cap escape must be both per-frame
    // (token-scoped) and per-executor (executor-scoped). Single-threaded unit
    // test exercising the two seams directly.
    let opts = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "rd1-scope".to_string(),
    };
    let exec1 =
      Arc::new(MultiThreadExecutor::new(&opts, Arc::new(RuntimeMetrics::default())).unwrap());
    let exec2 =
      Arc::new(MultiThreadExecutor::new(&opts, Arc::new(RuntimeMetrics::default())).unwrap());
    assert_ne!(exec1.id, exec2.id, "executors must get distinct ids");

    // (1) `schedule_blocking` tags ONLY same-executor owners. With exec1's token
    // ambient, a job scheduled on exec2 is tagged `None` (foreign executor), so
    // it can never be matched by any over-cap escape.
    let token1 = exec1.fresh_owner_token();
    {
      let _owner = BlockingOwnerGuard::enter(Some(token1));
      let _h = exec2.schedule_blocking(|| 0usize);
    }
    assert_eq!(
      exec2.blocking_queue.lock().unwrap().front().unwrap().owner,
      None,
      "a job scheduled under a foreign-executor token must be tagged None"
    );
    // exec2's own token finds no owned job; exec1's token is rejected at the gate.
    let token2 = exec2.fresh_owner_token();
    assert!(!exec2.run_owned_blocking_over_cap(token2), "no exec2-owned job is queued");
    exec2.blocking_queue.lock().unwrap().clear();

    // (2) Inject a job tagged with exec1's token directly into exec2's queue, then
    // confirm the executor gate (`token.executor_id == self.id`) is what isolates
    // executors: exec2's own token does not match it, and the gate condition for a
    // foreign token is false -- but the scan itself WOULD run it if the gate were
    // bypassed, proving the gate (not luck) is load-bearing.
    let ran = Arc::new(AtomicBool::new(false));
    let ran_job = Arc::clone(&ran);
    exec2.blocking_queue.lock().unwrap().push_back(QueuedBlocking {
      owner: Some(token1),
      run: Box::new(move || ran_job.store(true, Ordering::SeqCst)),
    });
    // Gate condition in cooperative_block_on for the foreign token: refuses.
    assert_ne!(token1.executor_id, exec2.id, "foreign token fails the executor gate");
    // exec2's own token does not match the exec1-tagged job (token-scoped scan).
    let token2b = exec2.fresh_owner_token();
    assert!(!exec2.run_owned_blocking_over_cap(token2b), "exec2 token must not match exec1's job");
    assert!(!ran.load(Ordering::SeqCst), "foreign-tagged job must not have run");
    // Were the gate bypassed, the scan WOULD find and run it -> proves the gate is
    // the isolation, and leaves the queue empty for a clean drop.
    assert!(exec2.run_owned_blocking_over_cap(token1), "scan matches the exact token");
    assert!(ran.load(Ordering::SeqCst));
  }

  #[test]
  fn current_thread_blocking_work_completes_inline() {
    let metrics = Arc::new(RuntimeMetrics::default());
    let backend =
      RuntimeBackend::CurrentThread(Arc::new(CurrentThreadExecutor::new(Arc::clone(&metrics))));

    let value = futures::executor::block_on(backend.spawn_blocking(|| 7, &metrics))
      .expect("blocking task should complete");

    assert_eq!(value, 7);
    assert_eq!(metrics.blocking_tasks_started.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.blocking_tasks_completed.load(Ordering::Relaxed), 1);
  }
}
