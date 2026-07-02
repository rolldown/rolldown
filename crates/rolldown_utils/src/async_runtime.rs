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
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
  },
  task::{Context, Poll},
  time::{Duration, Instant},
};

use async_task::{Runnable, Task};
use futures::FutureExt;

#[cfg(not(target_family = "wasm"))]
use futures::channel::oneshot;
#[cfg(not(target_family = "wasm"))]
use rayon::{ThreadPool, ThreadPoolBuilder};

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
  /// Deadline-based `block_on` deadlock detection (wake-path §6(d)): bounds
  /// how long a runtime-owned park (the CurrentThread `block_on` parker and
  /// MultiThread COOPERATIVE driver parks -- never the foreign/napi
  /// whole-build park, see §8.5) may sleep with ZERO runtime progress before
  /// panicking with the typed [`BlockOnDeadlock`] diagnostic. `None` (the
  /// default) disables deadline detection unless the
  /// [`PARK_DEADLINE_ENV`] environment variable is set at executor
  /// construction: a legitimately long park (e.g. awaiting a slow JS plugin)
  /// must never panic a production build. The threadless-wasm CERTAIN
  /// deadlock check is independent of this knob and always on.
  pub park_deadline: Option<Duration>,
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
      park_deadline: None,
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

/// Environment variable that arms deadline-based `block_on` deadlock
/// detection: milliseconds a runtime-owned park may sleep with zero runtime
/// progress before panicking (see [`RuntimeOptions::park_deadline`], which
/// takes precedence when set). Missing, non-numeric or `0` all mean
/// "disabled".
pub const PARK_DEADLINE_ENV: &str = "ROLLDOWN_PARK_DEADLINE_MS";

/// Parse a raw [`PARK_DEADLINE_ENV`] value; unusable values (unset,
/// non-numeric, `0`) disable deadline detection rather than erroring, the
/// same treatment `env_config::resolve_thread_count` gives the thread-count
/// variables.
fn parse_park_deadline(raw: Option<String>) -> Option<Duration> {
  let millis = raw?.parse::<u64>().ok().filter(|&millis| millis != 0)?;
  Some(Duration::from_millis(millis))
}

/// Read [`PARK_DEADLINE_ENV`] at executor construction (wake-path §6(d)); an
/// explicit `RuntimeOptions::park_deadline` takes precedence over it.
fn park_deadline_from_env() -> Option<Duration> {
  parse_park_deadline(std::env::var(PARK_DEADLINE_ENV).ok())
}

/// True on builds where no second thread can EVER deliver a wake to a parked
/// `block_on`: wasm without the `atomics` target feature (the single-thread
/// `wasm32-wasip1` build and `wasm32-unknown-unknown`). On such a build a
/// CurrentThread park decision with an empty queue and no pending wake token
/// is a PROVABLE deadlock (R2). The threaded wasi build
/// (`wasm32-wasip1-threads`) has real OS threads, so its parks are not
/// provably dead and fall under the optional deadline detection instead, like
/// native builds.
const THREADLESS_BUILD: bool = cfg!(all(target_family = "wasm", not(target_feature = "atomics")));

#[derive(Debug, Clone)]
pub struct RuntimeConfigError(String);

impl fmt::Display for RuntimeConfigError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl std::error::Error for RuntimeConfigError {}

/// Which self-detected `block_on` deadlock class fired (wake-path §6(d)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockOnDeadlockKind {
  /// CurrentThread on a threadless build: the park decision found an empty
  /// queue and no pending wake token, and no other thread exists that could
  /// ever deliver one -- a PROVABLE deadlock, detected unconditionally (not
  /// timing-based).
  CurrentThreadCertain,
  /// Threaded CurrentThread: a `block_on` park outlived the configured
  /// deadline with zero runtime progress.
  CurrentThreadDeadline,
  /// MultiThread: a cooperative pool driver's re-entrant `block_on` park
  /// outlived the configured deadline with zero executor progress. The
  /// foreign/napi whole-build park is exempt by design (§8.5).
  MultiThreadCooperativeDeadline,
}

/// Typed panic payload for a self-detected `block_on` deadlock. Thrown with
/// `std::panic::panic_any` at the park DECISION (never merely on a `Pending`
/// return -- a self-waking future is not a deadlock, intel §8.6), so the
/// runtime fails loudly instead of freezing the thread (and, in the
/// CurrentThread-on-JS-thread case, the whole JS event loop, where not even
/// test-harness timeouts can fire). When the panic unwinds out of a spawned
/// task's poll, `JoinError::from_panic` preserves this diagnostic's message.
#[derive(Debug, Clone)]
pub struct BlockOnDeadlock {
  pub kind: BlockOnDeadlockKind,
  /// The armed deadline for the deadline-based kinds; `None` for the provable
  /// threadless case.
  pub park_deadline: Option<Duration>,
}

impl BlockOnDeadlock {
  fn current_thread_certain() -> Self {
    Self { kind: BlockOnDeadlockKind::CurrentThreadCertain, park_deadline: None }
  }

  fn current_thread_deadline(deadline: Duration) -> Self {
    Self { kind: BlockOnDeadlockKind::CurrentThreadDeadline, park_deadline: Some(deadline) }
  }

  // The MultiThread executor (and thus this constructor's caller) does not
  // exist on wasm builds.
  #[cfg(not(target_family = "wasm"))]
  fn multi_thread_cooperative(deadline: Duration) -> Self {
    Self {
      kind: BlockOnDeadlockKind::MultiThreadCooperativeDeadline,
      park_deadline: Some(deadline),
    }
  }
}

impl fmt::Display for BlockOnDeadlock {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let deadline_ms = self.park_deadline.map_or(0, |deadline| deadline.as_millis());
    match self.kind {
      BlockOnDeadlockKind::CurrentThreadCertain => f.write_str(
        "provable async-runtime deadlock: `block_on` on the CurrentThread flavor is about to \
         park with an empty task queue and no pending wake on a threadless build -- no other \
         thread exists that could ever deliver the wake, so this park would never end. This is \
         the block_on-awaiting-JS hazard: native code called `block_on` on a future that \
         (transitively) awaits a JS continuation (e.g. a threadsafe-function callback, like the \
         dynamic-import-vars plugin's `resolver`), but the only thread that could run that \
         continuation is the one now parking.",
      ),
      BlockOnDeadlockKind::CurrentThreadDeadline => write!(
        f,
        "suspected async-runtime deadlock: `block_on` on the CurrentThread flavor parked for \
         {deadline_ms}ms ({PARK_DEADLINE_ENV} / RuntimeOptions::park_deadline) with an empty \
         task queue, no pending wake and ZERO runtime progress across the whole window. This is \
         the signature of the block_on-awaiting-JS hazard: a `block_on` on the JS thread \
         awaiting a JS continuation (e.g. a threadsafe-function callback) that can only run \
         once this thread returns to the event loop. If this park is legitimate, raise or unset \
         the deadline.",
      ),
      BlockOnDeadlockKind::MultiThreadCooperativeDeadline => write!(
        f,
        "suspected async-runtime deadlock: a cooperative pool driver's re-entrant `block_on` \
         parked for {deadline_ms}ms ({PARK_DEADLINE_ENV} / RuntimeOptions::park_deadline) with \
         no runnable or blocking work available and ZERO executor progress across the whole \
         window -- the awaited future's wake (often a JS continuation) can no longer arrive. \
         The foreign-thread whole-build `block_on` park is exempt from this deadline by design. \
         If this park is legitimate, raise or unset the deadline.",
      ),
    }
  }
}

impl std::error::Error for BlockOnDeadlock {}

#[derive(Debug)]
pub struct JoinError {
  message: String,
}

impl JoinError {
  fn from_panic(panic: &(dyn Any + Send + 'static)) -> Self {
    Self {
      // The typed deadlock diagnostic first: a deadline firing inside a
      // spawned task's poll unwinds into the spawn wrapper's catch_unwind and
      // surfaces here -- the diagnostic text must survive the trip.
      message: if let Some(deadlock) = panic.downcast_ref::<BlockOnDeadlock>() {
        deadlock.to_string()
      } else if let Some(message) = panic.downcast_ref::<&str>() {
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
  // Bumped at ENQUEUE time (before the queue push), the blocking-side twin of
  // `runnable_schedules`. Internal-only (not exported in the public snapshot):
  // it exists so the deadlock detector's progress fingerprint can observe a
  // blocking submission the moment it is made -- `blocking_tasks_started`
  // only advances once a worker RUNS the job, which may be arbitrarily later
  // (Codex round-2 finding).
  blocking_tasks_scheduled: AtomicU64,
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

  /// Enqueue-time twin of [`Self::runnable_scheduled`] for blocking work:
  /// called BEFORE the blocking-queue push, so the deadlock detector's
  /// fingerprint sees a submission even when no worker has started the job
  /// yet (Codex round-2).
  fn blocking_scheduled(&self) {
    self.blocking_tasks_scheduled.fetch_add(1, Ordering::Relaxed);
  }

  fn blocking_started(self: &Arc<Self>) -> ActiveBlockingGuard {
    self.blocking_tasks_started.fetch_add(1, Ordering::Relaxed);
    let active = self.active_blocking_tasks.fetch_add(1, Ordering::Relaxed) + 1;
    self.max_active_blocking_tasks.fetch_max(active, Ordering::Relaxed);
    ActiveBlockingGuard { metrics: Arc::clone(self) }
  }

  /// Coarse liveness fingerprint for deadline-based deadlock detection (wake
  /// -path §8.5): any of these counters advancing between two reads means the
  /// runtime did SOMETHING (scheduled or polled a runnable, completed a task,
  /// scheduled, started or finished a blocking job), so a parked driver
  /// observing an advance re-arms its deadline instead of firing. Both ENQUEUE
  /// counters (`runnable_schedules`, `blocking_tasks_scheduled`) are included
  /// so a submission is progress the moment it is made -- not only once a
  /// worker gets to run it (Codex round-2).
  fn progress_fingerprint(&self) -> ProgressFingerprint {
    ProgressFingerprint {
      runnable_schedules: self.runnable_schedules.load(Ordering::Relaxed),
      runnable_polls: self.runnable_polls.load(Ordering::Relaxed),
      tasks_completed: self.tasks_completed.load(Ordering::Relaxed),
      blocking_tasks_scheduled: self.blocking_tasks_scheduled.load(Ordering::Relaxed),
      blocking_tasks_started: self.blocking_tasks_started.load(Ordering::Relaxed),
      blocking_tasks_completed: self.blocking_tasks_completed.load(Ordering::Relaxed),
    }
  }

  fn reset(&self) {
    self.tasks_spawned.store(0, Ordering::Relaxed);
    self.tasks_completed.store(0, Ordering::Relaxed);
    self.tasks_panicked.store(0, Ordering::Relaxed);
    self.runnable_schedules.store(0, Ordering::Relaxed);
    self.runnable_polls.store(0, Ordering::Relaxed);
    self.queued_runnables.store(0, Ordering::Relaxed);
    self.max_queued_runnables.store(0, Ordering::Relaxed);
    self.blocking_tasks_scheduled.store(0, Ordering::Relaxed);
    self.active_runnables.store(0, Ordering::Relaxed);
    self.max_active_runnables.store(0, Ordering::Relaxed);
    self.blocking_tasks_started.store(0, Ordering::Relaxed);
    self.blocking_tasks_completed.store(0, Ordering::Relaxed);
    self.active_blocking_tasks.store(0, Ordering::Relaxed);
    self.max_active_blocking_tasks.store(0, Ordering::Relaxed);
  }
}

/// See [`RuntimeMetrics::progress_fingerprint`].
#[derive(Clone, Copy, PartialEq, Eq)]
struct ProgressFingerprint {
  runnable_schedules: u64,
  runnable_polls: u64,
  tasks_completed: u64,
  blocking_tasks_scheduled: u64,
  blocking_tasks_started: u64,
  blocking_tasks_completed: u64,
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
  // Deadlock detection (wake-path §6(d)). `threadless`: no second thread can
  // EVER deliver a wake to a parked `block_on`, so a park decision with an
  // empty queue and no pending wake token is a PROVABLE deadlock (panic
  // immediately, no timing involved). `park_deadline`: on threaded builds,
  // the optional bound for a park with zero runtime progress -- off unless
  // configured, because a legitimately long park must never panic a
  // production build.
  threadless: bool,
  park_deadline: Option<Duration>,
}

impl CurrentThreadExecutor {
  /// Build-default construction, kept for the pre-existing unit tests
  /// (production goes through [`Self::with_detection`] in
  /// `RuntimeBackend::new`, which also resolves the env deadline).
  #[cfg(test)]
  fn new(metrics: Arc<RuntimeMetrics>) -> Self {
    Self::with_detection(metrics, THREADLESS_BUILD, None)
  }

  /// Full constructor for the deadlock-detection knobs: `new` fixes
  /// `threadless` to the build's [`THREADLESS_BUILD`] with the deadline off;
  /// `RuntimeBackend::new` resolves the deadline from
  /// `RuntimeOptions::park_deadline` / [`PARK_DEADLINE_ENV`]; tests inject a
  /// native `threadless` stand-in and short deadlines through it.
  fn with_detection(
    metrics: Arc<RuntimeMetrics>,
    threadless: bool,
    park_deadline: Option<Duration>,
  ) -> Self {
    Self {
      queue: Mutex::new(VecDeque::new()),
      draining: AtomicBool::new(false),
      metrics,
      threadless,
      park_deadline,
    }
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

  /// Drive `future` to completion on this thread, draining queued runnables
  /// between polls (the old `DriveCurrentThread` wrapper, folded in). This
  /// loop OWNS its park site (wake-path §6(d)): the future is polled with
  /// this frame's own [`DriverParker`] as its waker, so the park DECISION can
  /// inspect the wake token directly instead of handing it to
  /// `futures::executor::block_on`'s opaque parker:
  ///
  ///   * `Pending` + queue empty + wake token pending: a self-waking future
  ///     (the `PendThenReady` shape) -- consume the token and re-poll. NOT a
  ///     park, NOT a deadlock; intel §8.6 pins panic-at-park-decision, never
  ///     at Pending-return.
  ///   * `Pending` + queue empty + NO token, threadless build: PROVABLE
  ///     deadlock -- no other thread exists that could ever deliver the wake
  ///     (the block_on-awaiting-JS class, R2). Panic immediately with the
  ///     typed [`BlockOnDeadlock`] diagnostic.
  ///   * `Pending` + queue empty + NO token, threaded build: park. When a
  ///     `park_deadline` is armed, the park is bounded with progress-based
  ///     reset (see [`park_with_deadline`]) and panics typed if a full
  ///     window passes with zero runtime progress.
  fn block_on(self: &Arc<Self>, mut future: Pin<&mut dyn Future<Output = ()>>) {
    let parker = Arc::new(DriverParker::default());
    let waker = std::task::Waker::from(Arc::clone(&parker));
    let mut cx = Context::from_waker(&waker);
    loop {
      if future.as_mut().poll(&mut cx).is_ready() {
        return;
      }
      if self.drain_one() {
        continue;
      }
      // PARK DECISION: the future is Pending and the queue is empty. A token
      // stored during the poll (or by a racing thread) means a wake is
      // already pending: consume it and re-poll instead of parking.
      if parker.consume_permit() {
        continue;
      }
      if self.threadless {
        std::panic::panic_any(BlockOnDeadlock::current_thread_certain());
      }
      match self.park_deadline {
        // A wake landing between `consume_permit` above and the park is
        // stored as the parker's permit, so `park` returns immediately --
        // never lost.
        None => parker.park(),
        Some(deadline) => match park_with_deadline(&parker, deadline, &self.metrics) {
          DeadlineParkOutcome::Woken => {}
          DeadlineParkOutcome::Expired(armed) => {
            // EXPIRY DECISION (Codex round-1, CT analog -- no registry
            // here): a wake can land in the instructions after the window
            // closed, and the fingerprint read inside `park_with_deadline`
            // is stale by now. Re-check both before declaring death; a hit
            // means the park was healthy -- continue the loop (re-poll,
            // drain, fresh full window on the next park).
            if parker.consume_permit() || self.metrics.progress_fingerprint() != armed {
              continue;
            }
            std::panic::panic_any(BlockOnDeadlock::current_thread_deadline(deadline));
          }
        },
      }
    }
  }
}

// Identifies the executor whose pool worker is currently draining runnables /
// blocking jobs on THIS thread (or `None` when not on a pool worker). Used by
// `MultiThreadExecutor::block_on` to detect a re-entrant (on-pool) call OF THE
// SAME EXECUTOR and drive the queue cooperatively instead of parking the worker
// (RD-1). Carrying the executor id (not a bare bool) keeps this marker scoped
// exactly like `BlockingOwnerToken.executor_id`: a worker of executor A that
// re-enters `block_on` on executor B must NOT be treated as B's on-pool driver
// -- it owns none of B's accounting, so it parks like any foreign-thread caller
// rather than driving B's queues as an unaccounted within-cap driver. Under the
// single global executor used in production these ids always match, so this is a
// defensive scoping gate (consistent with the blocking-owner machinery), not a
// reachable production change.
#[cfg(not(target_family = "wasm"))]
thread_local! {
  static ON_POOL_WORKER: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

// Per-worker LIFO slot (wake-path §6(c), tokio-style): holds at most ONE
// runnable that was scheduled FROM this pool worker (typically a task the
// currently running runnable just woke). It is popped by this same thread's
// next drain/run_one iteration BEFORE the shared FIFO, so the hot
// wake-then-await ping-pong pattern stays on one core with no queue mutex, no
// wake and no drainer spawn. Tagged with the executor id -- the same scoping
// pattern as `ON_POOL_WORKER` and `BlockingOwnerToken.executor_id` -- so only
// drain/run_one/flush of the SAME executor may pop it.
//
// STRANDING INVARIANT (wake-path §8.3): slot work is invisible to every other
// thread (`finish_draining`'s re-check and `wake_one` cannot see it), so the
// owning thread MUST either run its slot entry or flush it to the shared FIFO
// before it stops draining -- see the flush calls at `drain`'s returns, the
// pre-park flush in `cooperative_block_on`, and `LifoSlotFlushGuard` for
// unwinds. A runnable left here while the thread returns to rayon would be a
// silently lost task and a `queued_runnables` leak.
#[cfg(not(target_family = "wasm"))]
thread_local! {
  static LIFO_SLOT: std::cell::Cell<Option<(u64, Runnable)>> =
    const { std::cell::Cell::new(None) };
}

#[cfg(not(target_family = "wasm"))]
struct OnPoolWorkerGuard(Option<u64>);

#[cfg(not(target_family = "wasm"))]
impl OnPoolWorkerGuard {
  // Save the previous marker and install `id`, so nested/re-entrant drains
  // (possibly from different executors on the same thread) restore the exact
  // prior executor id on drop instead of unconditionally clearing it.
  fn enter(id: u64) -> Self {
    Self(ON_POOL_WORKER.with(|flag| flag.replace(Some(id))))
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
  // Registry of pool workers currently parked inside a re-entrant cooperative
  // `block_on`, woken ONE at a time when new queue work is scheduled. The pool
  // has a fixed number of OS threads, so a parked worker cannot rely on
  // `ensure_drainer` spawning a replacement (there is no free thread to run
  // it) -- it must wake and run the work itself (RD-1). Future-wakes do NOT go
  // through this registry: each driver's own `DriverParker` is its awaited
  // future's waker (see the wake-domain note at `DriverParker`).
  parked_drivers: ParkedDrivers,
  max_drainers: usize,
  max_blocking: usize,
  // Deadline-based deadlock detection for COOPERATIVE driver parks ONLY
  // (wake-path §6(d)); the foreign/napi whole-build park in `block_on`'s
  // else-branch is exempt by design (§8.5) -- it parks for entire builds and
  // a deadline there would panic healthy production runs. Off unless
  // configured via `RuntimeOptions::park_deadline` / `PARK_DEADLINE_ENV`.
  park_deadline: Option<Duration>,
  metrics: Arc<RuntimeMetrics>,
}

#[cfg(not(target_family = "wasm"))]
impl MultiThreadExecutor {
  /// Max consecutive runnable runs before a worker yields: `drain` exits (and
  /// respawns via `finish_draining`) and `cooperative_block_on` flushes its
  /// LIFO slot, so a hot slot streak cannot starve the shared FIFO or the
  /// blocking queue on either loop (wake-path §8.7).
  const RUNNABLE_BUDGET: usize = 64;

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
      parked_drivers: ParkedDrivers::default(),
      max_drainers: options.worker_threads,
      max_blocking: options.max_blocking_tasks.min(options.worker_threads),
      park_deadline: options.park_deadline.or_else(park_deadline_from_env),
      metrics,
    })
  }

  fn schedule(self: &Arc<Self>, runnable: Runnable) {
    // Fires for BOTH the slot and the FIFO path: a slot-resident runnable is
    // still "queued" until popped (`runnable_started` balances it either way).
    self.metrics.runnable_scheduled();
    // LIFO fast path (wake-path §6(c)): a runnable scheduled from a pool
    // worker OF THIS EXECUTOR goes into that worker's slot instead of the
    // shared FIFO -- this thread pops it before its next FIFO look, so no
    // queue mutex, no wake, no drainer spawn.
    //
    // "Pops it before its next FIFO look" is only true while RUNNABLE code is
    // on the stack (drain/run_one loop iterations, cooperative polls). It is
    // FALSE inside a blocking closure: `ON_POOL_WORKER` spans the whole drain
    // frame including `blocking()` calls, but the slot is only popped after
    // the closure returns -- so a closure that spawns and then synchronously
    // waits on the child would deadlock with the child slotted (no wake, no
    // drainer, even with idle workers). `BLOCKING_OWNER` is `Some` exactly
    // while straight-line blocking-closure code runs (drain/run_one blocking
    // branches, the over-cap escape) and `None` while runnables run (the
    // slot/FIFO owner-clear guards), so it is the discriminator: bypass the
    // slot and take the FIFO+wake path whenever an owner frame is ambient.
    // (A cooperative poll entered FROM a blocking closure also carries the
    // closure's token and loses the fast path -- a rare, conservative,
    // correctness-first trade.)
    if ON_POOL_WORKER.with(std::cell::Cell::get) == Some(self.id)
      && BLOCKING_OWNER.with(std::cell::Cell::get).is_none()
    {
      match LIFO_SLOT.with(std::cell::Cell::take) {
        None => {
          LIFO_SLOT.with(|slot| slot.set(Some((self.id, runnable))));
          return;
        }
        // Same-executor occupant: the NEWEST runnable takes the slot (it is
        // the hottest); the displaced occupant falls back to the shared FIFO
        // with a normal wake so other workers can pick it up.
        Some((id, displaced)) if id == self.id => {
          LIFO_SLOT.with(|slot| slot.set(Some((self.id, runnable))));
          self.push_to_queue_and_wake(displaced);
          return;
        }
        // Foreign-executor occupant (defensive: requires a thread acting as a
        // worker of two executors, e.g. under a test-installed marker; the
        // production process has one global executor). We must not flush
        // another executor's runnable onto OUR queue, so leave it in place --
        // its own executor's drain on this thread pops or flushes it -- and
        // route the new runnable to the FIFO.
        Some(foreign) => {
          LIFO_SLOT.with(|slot| slot.set(Some(foreign)));
        }
      }
    }
    self.push_to_queue_and_wake(runnable);
  }

  /// Shared-FIFO tail of [`Self::schedule`]: push, then wake (also used for
  /// displaced/flushed slot runnables, whose `runnable_scheduled` accounting
  /// already happened at their original schedule).
  fn push_to_queue_and_wake(self: &Arc<Self>, runnable: Runnable) {
    self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_back(runnable);
    self.wake_for_new_work();
  }

  /// Queue-wake tail shared by `schedule` and `schedule_blocking`: wake ONE
  /// parked cooperative driver; when none is parked, fall back to spawning a
  /// drainer (RD-1). A single wake suffices because the woken driver re-checks
  /// every work source in its loop before re-parking, and a wake it absorbs
  /// without consuming is handed on at `cooperative_block_on` exit (miss
  /// compensation, wake-path §8.2). MUST be called AFTER the work was pushed
  /// under its queue mutex -- see the lost-wakeup argument at
  /// [`ParkedDrivers::wake_one`].
  fn wake_for_new_work(self: &Arc<Self>) {
    if !self.parked_drivers.wake_one() {
      self.ensure_drainer();
    }
  }

  fn schedule_blocking<F, T>(self: &Arc<Self>, function: F) -> JoinHandle<T>
  where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
  {
    let (sender, receiver) = oneshot::channel();
    let metrics = Arc::clone(&self.metrics);
    // Enqueue-time progress signal, BEFORE the queue push/wake (Codex
    // round-2): the deadlock detector's fingerprint must be able to observe
    // this submission even while the job merely sits queued -- symmetric with
    // `runnable_scheduled` on the runnable path.
    self.metrics.blocking_scheduled();
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
    self.wake_for_new_work();
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
    // Mark this worker so a re-entrant `block_on` (reached from a polled task)
    // drives the queue cooperatively instead of parking the worker.
    let _on_pool = OnPoolWorkerGuard::enter(self.id);
    // Unwind backstop (wake-path §8.3): never leave a runnable stranded in
    // the LIFO slot. Normal exits flush explicitly BEFORE `finish_draining`
    // below (so its re-check can see the flushed runnable); this guard only
    // covers unwinds, which should be impossible (runnables and blocking
    // wrappers are catch_unwind'd) but must not lose a task if they happen.
    let _slot_backstop = LifoSlotFlushGuard(&self);

    // The LIFO slot shares this budget with the FIFO: a hot wake/await pair
    // can hold the slot for at most RUNNABLE_BUDGET consecutive runs (the
    // streak cap, wake-path §8.7) before this drainer exits, flushes the slot
    // and lets `finish_draining` re-arm fairness for FIFO and blocking work.
    for _ in 0..Self::RUNNABLE_BUDGET {
      if let Some(runnable) = self.pop_lifo_slot() {
        // Same lexical owner-clear as `run_one`'s slot/FIFO pops: the POP can
        // run while an owner frame is ambient (e.g. a cooperative `block_on`
        // entered from a blocking closure), and the slot must not smuggle
        // that frame's over-cap privilege into the runnable (RD-1 (B)).
        let _non_owner = BlockingOwnerGuard::enter(None);
        run_runnable(&self.metrics, runnable);
        continue;
      }
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
      // MANDATORY flush before finish_draining (wake-path §8.3). On this path
      // the slot was observed empty at the top of the iteration and no user
      // code ran since, so the flush is defensive -- kept unconditional so
      // every drain exit upholds the stranding invariant by construction.
      self.flush_lifo_slot();
      self.finish_draining();
      return;
    }

    // Budget exhausted: the last runnable may have scheduled into the slot.
    // MANDATORY flush (wake-path §8.3) before `finish_draining` so its
    // re-check sees the runnable on the shared FIFO and re-arms a drainer.
    self.flush_lifo_slot();
    self.finish_draining();
  }

  /// Pop this worker's LIFO slot if it holds a runnable of THIS executor.
  /// A foreign executor's entry is left untouched (see the scoping note at
  /// `LIFO_SLOT`).
  fn pop_lifo_slot(&self) -> Option<Runnable> {
    LIFO_SLOT.with(|slot| match slot.take() {
      Some((id, runnable)) if id == self.id => Some(runnable),
      other => {
        slot.set(other);
        None
      }
    })
  }

  /// Move this worker's slot entry (same-executor only) to the shared FIFO,
  /// with a wake so any worker can pick it up. MANDATORY (wake-path §8.3) on
  /// every path where this thread stops draining this executor: `drain`'s
  /// returns (BEFORE `finish_draining`), before a cooperative park, and on
  /// unwind via `LifoSlotFlushGuard` -- a runnable stranded in a thread-local
  /// slot while its thread leaves executor code is a silently lost task.
  fn flush_lifo_slot(self: &Arc<Self>) {
    if let Some(runnable) = self.pop_lifo_slot() {
      self.push_to_queue_and_wake(runnable);
    }
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
    // LIFO-slot invariant (wake-path §8.4): this re-check reads only the
    // SHARED queues -- per-worker slots are invisible to it, and that is
    // sound because slot work never needs a foreign observer. A slot entry is
    // always either popped and run by its owning thread (drain / run_one) or
    // flushed to the shared FIFO by that same thread BEFORE it stops draining
    // (both `drain` returns flush ahead of this call; `LifoSlotFlushGuard`
    // covers unwinds; cooperative parks flush defensively). No thread leaves
    // executor code with its slot occupied, so the exit-then-respawn window
    // below cannot hide slot work from `ensure_drainer`.
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
    if ON_POOL_WORKER.with(std::cell::Cell::get) == Some(self.id) {
      // Re-entrant call from a pool worker OF THIS EXECUTOR: parking here would
      // freeze a worker that the awaited future may itself need to make
      // progress. Drive the queue cooperatively instead (mirrors the
      // CurrentThread drive loop). The id check mirrors the
      // `BlockingOwnerToken.executor_id` gate: a worker of a DIFFERENT executor
      // owns none of this executor's accounting, so it must take the parking
      // path below rather than drive these queues as an unaccounted driver.
      self.cooperative_block_on(future);
    } else {
      // Non-pool (e.g. napi caller) thread, or a pool worker of another
      // executor: keep parking — there is no pool worker of THIS executor to
      // starve, and other workers continue draining as usual.
      futures::executor::block_on(future);
    }
  }

  /// Run a single unit of queued work (this worker's LIFO slot first, then a
  /// shared-FIFO runnable, else a blocking job). Returns `true` if work was
  /// performed. Mirrors the body of `drain`.
  fn run_one(self: &Arc<Self>) -> bool {
    if let Some(runnable) = self.pop_lifo_slot() {
      // Same owner-clear as the FIFO branch below (RD-1 (B)): the slot must
      // not smuggle an owner frame's over-cap privilege into the runnables it
      // carries any more than the shared FIFO does.
      let _non_owner = BlockingOwnerGuard::enter(None);
      run_runnable(&self.metrics, runnable);
      return true;
    }
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

  /// Production seam for the over-cap escape in `cooperative_block_on`: read the
  /// ambient owner frame, apply the executor-id gate, and (only for a frame of
  /// THIS executor) attempt the token-matched over-cap run. Returns `true` iff a
  /// same-executor owned job was run. A foreign-executor token (or no token at
  /// all) fails the `executor_id` gate and returns `false` WITHOUT touching the
  /// queue, so `cooperative_block_on` parks/drives instead (RD-1 (B)). Extracted
  /// as an `&self` helper so the executor-id gate is exercised through the ambient
  /// thread-local exactly as production reaches it.
  fn try_owned_blocking_over_cap(self: &Arc<Self>) -> bool {
    if let Some(token) = BLOCKING_OWNER.with(std::cell::Cell::get) {
      if token.executor_id == self.id {
        return self.run_owned_blocking_over_cap(token);
      }
    }
    false
  }

  /// Cooperative `block_on` for a re-entrant call from a pool worker. Instead
  /// of parking the worker (which would freeze one of the pool's fixed OS
  /// threads), it drives the awaited future while servicing queued work. When
  /// nothing is immediately runnable it parks on its own [`DriverParker`],
  /// which is woken directly by the awaited future's waker and through the
  /// `parked_drivers` registry by `schedule`/`schedule_blocking`, so a later
  /// wake reaches this worker even when every worker is parked and
  /// `ensure_drainer` has no free thread to spawn a replacement (RD-1).
  fn cooperative_block_on(self: &Arc<Self>, mut future: Pin<&mut dyn Future<Output = ()>>) {
    // One parker per `block_on` frame. Nested re-entrant frames each own one:
    // only the innermost frame runs or parks at any moment, and an outer
    // frame's future-wake is stored as its parker's permit, consumed when
    // control returns to that frame's loop.
    let parker = Arc::new(DriverParker::default());
    // WAKE-DOMAIN SPLIT (wake-path §8.2): the awaited future is polled with
    // THIS driver's own parker as its waker, so a future-wake targets exactly
    // the driver that awaits it -- delivered as a stored permit (skip next
    // park) when this driver is currently running, so it is never lost.
    let waker = std::task::Waker::from(Arc::clone(&parker));
    let mut cx = Context::from_waker(&waker);
    // Consecutive `run_one` successes since the last yield point: the
    // cooperative loop mirrors `drain`'s RUNNABLE_BUDGET streak cap. Without
    // it, a hot slot chain (each runnable schedules a successor into this
    // worker's slot, which `run_one` pops BEFORE the FIFO) would keep this
    // loop inside `run_one` forever and the shared FIFO would never be
    // reached from this thread -- a deadlock on a 1-worker pool when the
    // awaited future depends on a FIFO task (wake-path §8.7).
    let mut streak = 0usize;
    loop {
      if future.as_mut().poll(&mut cx).is_ready() {
        // MANDATORY exit flush (wake-path §8.3): the final poll above may
        // have scheduled same-executor work into THIS worker's slot (no FIFO
        // push, no wake). The caller may synchronously wait on that work
        // after `block_on` returns -- and the enclosing `drain` frame that
        // would otherwise pop the slot can be arbitrarily far away -- so the
        // slot must be handed to the shared FIFO (with a wake) before every
        // return from this loop.
        self.flush_lifo_slot();
        // MISS COMPENSATION (wake-path §8.2): a registry wake may have been
        // delivered to this driver while its future was completing -- the
        // permit absorbed, the queue item not consumed. Hand the wake on
        // before exiting so queue work is not stranded while other drivers
        // stay parked. (The enclosing `drain` loop re-checks the queues too,
        // but that can be arbitrarily far away, e.g. beneath a long-running
        // task or blocking closure.)
        if self.has_queued_work() {
          self.wake_for_new_work();
        }
        return;
      }
      if self.run_one() {
        streak += 1;
        if streak >= Self::RUNNABLE_BUDGET {
          // Budget yield: move a hot slot occupant to the shared FIFO (with
          // a wake), so the next `run_one` pops the FIFO front (the oldest
          // work) and other workers can steal the chain. The awaited future
          // is polled every iteration regardless, so it is never starved.
          self.flush_lifo_slot();
          streak = 0;
        }
        continue;
      }
      streak = 0;
      // Only a worker that owns a counted blocking frame OF THIS EXECUTOR may run
      // a queued blocking job over the cap, and only the job that frame itself
      // scheduled (the genuine nested-blocking case it must unblock). A plain
      // runnable driver owns no frame; a stale token from another executor fails
      // the `executor_id` check -- both respect `max_blocking` and park/drive
      // instead of starting extra blocking work (RD-1 (B)).
      if self.try_owned_blocking_over_cap() {
        continue;
      }
      // MANDATORY pre-park flush (wake-path §8.3). Defensive on every
      // currently reachable path: `run_one` above popped the slot and no user
      // code ran since -- but parking with a stranded slot runnable would
      // silently lose that task, so the invariant is upheld unconditionally.
      self.flush_lifo_slot();
      // Park protocol: register FIRST, then re-check, then park -- the
      // waiter-side mirror of the push-then-wake schedule side. See the
      // lost-wakeup argument at [`ParkedDrivers::wake_one`].
      self.parked_drivers.register(&parker);
      if self.has_queued_work() {
        self.parked_drivers.deregister(&parker);
        continue;
      }
      match self.park_deadline {
        None => parker.park(),
        Some(deadline) => {
          // Deadline-bounded cooperative park (wake-path §6(d)) with
          // PROGRESS-BASED reset (§8.5): a candidate deadlock only when a
          // FULL deadline window passes with zero executor progress. The
          // foreign-thread whole-build park in `block_on`'s else-branch is
          // exempt by design.
          match park_with_deadline(&parker, deadline, &self.metrics) {
            DeadlineParkOutcome::Woken => {}
            DeadlineParkOutcome::Expired(armed) => {
              #[cfg(test)]
              run_deadline_expiry_test_hook();
              // EXPIRY DECISION (Codex rounds 1+2). While this parker was
              // registered, a racing `wake_one` may have popped it, stored
              // its permit and reported the wake as DELIVERED (so the
              // scheduler spawned no drainer) -- panicking now would swallow
              // a healthy wake and kill a live build. Deregister FIRST so no
              // FURTHER wake can be counted against this parker, then
              // re-check every way a wake could have landed around the
              // expiry:
              //   * permit: a `wake_one` (or direct future-wake) that landed
              //     while still registered MUST be acted on;
              //   * queued work: the work behind such a wake was pushed
              //     under the queue mutex BEFORE `wake_one` popped us, so
              //     even if its `unpark` is still in flight the queue
              //     re-check observes it;
              //   * fingerprint: §8.5's premise is ZERO progress, and the
              //     read inside `park_with_deadline` is stale by the time we
              //     get here -- re-read as the LAST step. Both ENQUEUE
              //     counters are in the fingerprint (`runnable_schedules`,
              //     and since Codex round-2 `blocking_tasks_scheduled`,
              //     bumped before the push), so a submission of EITHER kind
              //     landing after the queue re-check above is still caught
              //     here.
              // Any hit => not a deadlock: continue the cooperative loop
              // (fresh future poll, `run_one` picks up the work, and the
              // next park arms a fresh FULL deadline window). The residue is
              // exactly a BARE direct future-wake -- no queue push, no
              // counter movement -- landing in the few instructions after
              // these re-checks: the inherent, unclosable residue of any
              // timing-based detector, one reason the deadline is opt-in.
              self.parked_drivers.deregister(&parker);
              if parker.consume_permit() || self.has_queued_work() {
                continue;
              }
              // Test-only seam (Codex round-2 regression): between the
              // permit/queued-work re-checks and the fingerprint verdict.
              #[cfg(test)]
              run_deadline_verdict_test_hook();
              if self.metrics.progress_fingerprint() != armed {
                continue;
              }
              std::panic::panic_any(BlockOnDeadlock::multi_thread_cooperative(deadline));
            }
          }
        }
      }
      // `wake_one` removes the parkers it pops; a direct future-wake does
      // not. Deregister-if-present covers both.
      self.parked_drivers.deregister(&parker);
    }
  }

  /// Is there work a cooperative driver could pick up right now? Used by the
  /// pre-park re-check and the exit compensation in `cooperative_block_on`;
  /// mirrors `finish_draining`'s re-check (a queued runnable, or a queued
  /// blocking job within the cap). The over-cap OWNED job is deliberately not
  /// re-checked here: a job tagged with this driver's owner frame can only be
  /// scheduled from this very thread (the frame token is ambient only here),
  /// so it cannot appear concurrently with this driver's park -- the loop's
  /// `try_owned_blocking_over_cap` step always observes it first.
  fn has_queued_work(&self) -> bool {
    if !self.queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty() {
      return true;
    }
    self.active_blocking.load(Ordering::Acquire) < self.max_blocking
      && !self.blocking_queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty()
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

/// Unwind backstop for `drain` (wake-path §8.3): flushes the worker's LIFO
/// slot on drop so an unwinding drain can never strand a runnable in the
/// thread-local slot of a thread that is about to return to rayon. Normal
/// drain exits flush explicitly BEFORE `finish_draining`; this drop is then a
/// no-op (the slot is already empty).
#[cfg(not(target_family = "wasm"))]
struct LifoSlotFlushGuard<'a>(&'a Arc<MultiThreadExecutor>);

#[cfg(not(target_family = "wasm"))]
impl Drop for LifoSlotFlushGuard<'_> {
  fn drop(&mut self) {
    self.0.flush_lifo_slot();
  }
}

// ---------------------------------------------------------------------------
// Targeted wake machinery for cooperative re-entrant `block_on` drivers
// (replaces the old `CoopSignal` generation broadcast).
//
// WAKE-DOMAIN SPLIT (wake-path §8.2 / §6(b)): under the broadcast, ANY notify
// woke ALL parked drivers, so misdirected wakes were impossible by
// construction (and the churn was the measured tax). With targeted wakes the
// two wake sources are split:
//   * FUTURE-WAKES: `cooperative_block_on` polls its future with the driver's
//     OWN `DriverParker` as the waker, so a future-wake reaches exactly the
//     driver that awaits it. The permit bit makes a wake delivered while that
//     driver is RUNNING (mid-poll / mid-run_one) stick: the driver skips its
//     next park instead of losing the wake.
//   * QUEUE-WAKES (`schedule` / `schedule_blocking`): any parked driver can
//     serve queue work, so these go through the `ParkedDrivers` registry and
//     wake ONE parked driver; `ensure_drainer` remains the fallback when none
//     is parked, and an absorbed wake is handed on at `cooperative_block_on`
//     exit (miss compensation).
// Foreign-thread (napi caller) parks in `block_on` use
// `futures::executor::block_on`'s own parker and are OUTSIDE this mechanism.
// The CurrentThread flavor's `block_on` reuses `DriverParker` (one per
// frame, no registry) so it too OWNS its park site for deadlock detection
// (wake-path §6(d)); `ParkedDrivers` remains MultiThread-only.

/// No permit; the driver is running (or consumed its last permit).
const PARKER_EMPTY: usize = 0;
/// A wake permit is stored; the next `park` consumes it without sleeping.
const PARKER_NOTIFIED: usize = 1;
/// The driver is sleeping on the condvar (or committing to sleep under `lock`).
const PARKER_SLEEPING: usize = 2;

/// One driver's private parker: a saturating one-permit token plus a condvar
/// to sleep on when no permit is stored. Also the `Waker` for the future that
/// driver's `cooperative_block_on` / CurrentThread `block_on` frame awaits.
#[derive(Default)]
struct DriverParker {
  state: AtomicUsize,
  lock: Mutex<()>,
  condvar: std::sync::Condvar,
}

impl DriverParker {
  /// Grant the wake permit. Wakes the driver if it is sleeping; otherwise the
  /// permit is stored and the driver's next [`Self::park`] returns
  /// immediately -- a wake delivered while the driver is running is never
  /// lost (wake-path §8.2). Multiple grants coalesce into one permit, which
  /// is safe because a woken driver re-checks every work source in its loop
  /// before it can park again.
  fn unpark(&self) {
    if self.state.swap(PARKER_NOTIFIED, Ordering::SeqCst) == PARKER_SLEEPING {
      // The sleeper publishes PARKER_SLEEPING while holding `lock` and
      // releases the lock only inside `condvar.wait`; the empty critical
      // section here fences the notify so it cannot fire in the window
      // between the sleeper's state store and its wait.
      drop(self.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
      self.condvar.notify_one();
    }
  }

  /// Consume a stored wake permit if one is present, without ever sleeping.
  /// This is the park DECISION's token check (wake-path §6(d)): a `true`
  /// return means a wake is already pending, so the caller must re-poll
  /// instead of parking (or panicking).
  fn consume_permit(&self) -> bool {
    self
      .state
      .compare_exchange(PARKER_NOTIFIED, PARKER_EMPTY, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
  }

  /// Block until the permit is granted, consuming it. Returns immediately if
  /// a permit is already stored. Only the owning driver calls `park`, so at
  /// most one thread ever sleeps here.
  fn park(&self) {
    // Fast path: consume a stored permit without touching the lock.
    if self.consume_permit() {
      return;
    }
    let mut guard = self.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match self.state.compare_exchange(
      PARKER_EMPTY,
      PARKER_SLEEPING,
      Ordering::SeqCst,
      Ordering::SeqCst,
    ) {
      Ok(_) => {}
      // The permit arrived between the fast path and here: consume it. (Only
      // this thread stores PARKER_SLEEPING, so the observed value can only be
      // PARKER_NOTIFIED.)
      Err(_) => {
        self.state.store(PARKER_EMPTY, Ordering::SeqCst);
        return;
      }
    }
    loop {
      guard = self.condvar.wait(guard).unwrap_or_else(std::sync::PoisonError::into_inner);
      if self
        .state
        .compare_exchange(PARKER_NOTIFIED, PARKER_EMPTY, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
      {
        return;
      }
      // Spurious wakeup (state still PARKER_SLEEPING): keep waiting.
    }
  }

  /// Like [`Self::park`], but bounded: returns `true` when woken by a permit
  /// (consumed) and `false` when `timeout` elapsed with no permit granted.
  /// A `false` return has retracted the SLEEPING claim and left the parker
  /// EMPTY, so the caller may park again. Used by the deadline-based deadlock
  /// detection (wake-path §6(d)).
  fn park_timeout(&self, timeout: Duration) -> bool {
    if self.consume_permit() {
      return true;
    }
    let mut guard = self.lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match self.state.compare_exchange(
      PARKER_EMPTY,
      PARKER_SLEEPING,
      Ordering::SeqCst,
      Ordering::SeqCst,
    ) {
      Ok(_) => {}
      // The permit arrived between the fast path and here: consume it. (Only
      // this thread stores PARKER_SLEEPING, so the observed value can only be
      // PARKER_NOTIFIED.)
      Err(_) => {
        self.state.store(PARKER_EMPTY, Ordering::SeqCst);
        return true;
      }
    }
    let deadline = Instant::now() + timeout;
    loop {
      if self
        .state
        .compare_exchange(PARKER_NOTIFIED, PARKER_EMPTY, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
      {
        return true;
      }
      let now = Instant::now();
      if now >= deadline {
        // Timed out: retract the SLEEPING claim -- unless a permit landed in
        // the meantime (an `unpark` that already saw SLEEPING may be waiting
        // on `lock` to notify; consuming its permit here IS the wake it
        // intended to deliver, so report "woken", not "timed out").
        return match self.state.compare_exchange(
          PARKER_SLEEPING,
          PARKER_EMPTY,
          Ordering::SeqCst,
          Ordering::SeqCst,
        ) {
          Ok(_) => false,
          Err(_) => {
            self.state.store(PARKER_EMPTY, Ordering::SeqCst);
            true
          }
        };
      }
      let (returned_guard, _timed_out) = self
        .condvar
        .wait_timeout(guard, deadline - now)
        .unwrap_or_else(std::sync::PoisonError::into_inner);
      guard = returned_guard;
      // Loop re-checks the permit first, then the deadline (spurious wakeups
      // simply wait out the remaining time).
    }
  }
}

impl std::task::Wake for DriverParker {
  fn wake(self: Arc<Self>) {
    self.unpark();
  }

  fn wake_by_ref(self: &Arc<Self>) {
    self.unpark();
  }
}

/// Outcome of [`park_with_deadline`].
enum DeadlineParkOutcome {
  /// Woken by a permit (consumed); the park was healthy.
  Woken,
  /// A full deadline window elapsed with zero observed progress. Carries the
  /// fingerprint captured when that window was ARMED so the caller can make
  /// the final expiry DECISION against a fresh read -- expiry alone is NOT a
  /// verdict: a wake can land in the instructions after the window closes
  /// (Codex round-1), and for MultiThread a `wake_one` can still pop the
  /// still-registered parker and count its wake as DELIVERED. Callers must
  /// deregister (MT), then re-check permit / queued work / fingerprint before
  /// panicking.
  Expired(ProgressFingerprint),
}

/// Deadline-bounded park with PROGRESS-BASED reset (wake-path §6(d), §8.5).
/// Every time `deadline` elapses without a wake, the runtime's progress
/// fingerprint is compared against the value captured when the deadline was
/// ARMED: any advance re-arms the deadline instead of firing, because a
/// legitimately long park -- e.g. awaiting a slow JS plugin while other work
/// proceeds -- must never be declared dead. `Expired` is a *candidate*
/// deadlock only; the panic decision lives with the caller (see
/// [`DeadlineParkOutcome`]).
fn park_with_deadline(
  parker: &DriverParker,
  deadline: Duration,
  metrics: &RuntimeMetrics,
) -> DeadlineParkOutcome {
  let mut armed = metrics.progress_fingerprint();
  loop {
    if parker.park_timeout(deadline) {
      return DeadlineParkOutcome::Woken;
    }
    let current = metrics.progress_fingerprint();
    if current != armed {
      armed = current;
      continue;
    }
    return DeadlineParkOutcome::Expired(armed);
  }
}

// Test-only injection seam for the deadline-expiry race (Codex round-1
// regression test): fired on the driver thread immediately after
// `park_with_deadline` reports `Expired`, while the parker is STILL
// REGISTERED in `parked_drivers` and before the expiry decision runs -- the
// exact window in which a racing `wake_one` can pop the parker, store its
// permit and report the wake as delivered. Lets a test interleave that race
// deterministically instead of wall-clock lottery.
#[cfg(all(test, not(target_family = "wasm")))]
thread_local! {
  static DEADLINE_EXPIRY_TEST_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
    const { std::cell::RefCell::new(None) };
}

#[cfg(all(test, not(target_family = "wasm")))]
fn run_deadline_expiry_test_hook() {
  if let Some(hook) = DEADLINE_EXPIRY_TEST_HOOK.with(|slot| slot.borrow_mut().take()) {
    hook();
  }
}

// Second test-only injection seam (Codex round-2 regression test): fired
// AFTER the expiry decision's permit and queued-work re-checks both came up
// empty (parker already deregistered) and BEFORE the final fingerprint
// verdict -- the window in which an enqueue is invisible to every earlier
// re-check and only its ENQUEUE counter in the fingerprint can prevent a
// false BlockOnDeadlock.
#[cfg(all(test, not(target_family = "wasm")))]
thread_local! {
  static DEADLINE_VERDICT_TEST_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
    const { std::cell::RefCell::new(None) };
}

#[cfg(all(test, not(target_family = "wasm")))]
fn run_deadline_verdict_test_hook() {
  if let Some(hook) = DEADLINE_VERDICT_TEST_HOOK.with(|slot| slot.borrow_mut().take()) {
    hook();
  }
}

/// Registry of parked (or committed-to-parking) cooperative drivers, used by
/// queue-wakes to wake exactly one of them.
#[cfg(not(target_family = "wasm"))]
#[derive(Default)]
struct ParkedDrivers {
  parked: Mutex<Vec<Arc<DriverParker>>>,
  // Mirror of `parked.len()`, maintained under the mutex and read lock-free
  // by `wake_one`'s no-waiter fast path.
  count: AtomicUsize,
}

#[cfg(not(target_family = "wasm"))]
impl ParkedDrivers {
  /// Register `parker` as parked-or-parking. MUST precede the caller's final
  /// work re-check -- see the lost-wakeup argument at [`Self::wake_one`].
  fn register(&self, parker: &Arc<DriverParker>) {
    let mut parked = self.parked.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    parked.push(Arc::clone(parker));
    self.count.store(parked.len(), Ordering::SeqCst);
  }

  /// Remove `parker` if still registered. `wake_one` removes the parkers it
  /// pops, while a direct future-wake does not, so drivers call this after
  /// every park (and after a re-check aborts one).
  fn deregister(&self, parker: &Arc<DriverParker>) {
    let mut parked = self.parked.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(index) = parked.iter().position(|candidate| Arc::ptr_eq(candidate, parker)) {
      parked.swap_remove(index);
      self.count.store(parked.len(), Ordering::SeqCst);
    }
  }

  /// Wake one parked driver -- most recently parked first, its stack and
  /// caches are the warmest -- returning whether anyone was woken.
  ///
  /// LOST-WAKEUP ARGUMENT (wake-path §8.1, registry form). A scheduler pushes
  /// work under a work-queue mutex Q, THEN calls `wake_one`; a parking driver
  /// registers itself (the `count` store above, under the registry mutex),
  /// THEN re-checks the work queues under Q, and only then parks. Order the
  /// two Q critical sections:
  ///   * If the driver's re-check locks Q after the scheduler's push unlocked
  ///     it, the re-check observes the work and the driver does not park --
  ///     skipping the wake (fast path below) is safe.
  ///   * Otherwise the driver's re-check unlocked Q before the scheduler's
  ///     push locked it. The driver's `count` store is sequenced before its
  ///     re-check, the re-check's unlock of Q synchronizes-with the push's
  ///     lock of Q, and the push is sequenced before the fast-path load
  ///     below; the load therefore happens-after the `count` store and must
  ///     observe `count >= 1`: the slow path pops a registered parker and
  ///     wakes it.
  ///
  /// A wake delivered to a driver that raced out of its park (or aborted it)
  /// is stored as that driver's permit; the driver re-checks every work
  /// source before it can sleep on the next park, so the wake still lands.
  fn wake_one(&self) -> bool {
    if self.count.load(Ordering::SeqCst) == 0 {
      // No-waiter fast path: nobody is parked or committed to parking (any
      // in-flight parker that missed this check re-checks the queues after
      // registering, per the argument above).
      return false;
    }
    let parker = {
      let mut parked = self.parked.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      let parker = parked.pop();
      self.count.store(parked.len(), Ordering::SeqCst);
      parker
    };
    match parker {
      Some(parker) => {
        parker.unpark();
        true
      }
      None => false,
    }
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
        Ok(Self::CurrentThread(Arc::new(CurrentThreadExecutor::with_detection(
          metrics,
          THREADLESS_BUILD,
          options.park_deadline.or_else(park_deadline_from_env),
        ))))
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
        // Same submitted-counter semantics as the MultiThread path (the job
        // just also runs inline immediately here).
        metrics.blocking_scheduled();
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
        park_deadline: None,
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
      park_deadline: None,
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
  fn multi_thread_block_on_from_caller_thread_receives_pool_produced_value() {
    // RD-10: cross-thread liveness guard for the NON-pool (napi-caller) path of
    // `MultiThreadExecutor::block_on`. The caller thread is not a pool worker, so
    // `block_on` takes the parking branch (`futures::executor::block_on`) while a
    // SEPARATELY spawned task runs on a pool worker and produces the awaited value.
    // This pins ONLY the cross-thread wake path: the pool task completing must wake
    // the parked caller's waker so `block_on` returns the pool-computed value.
    //
    // HONEST ORDERING (no sleeps): the awaited pool task is GATED on a oneshot and
    // therefore returns `Pending` on its first poll. A `CallerProbe` future wraps the
    // task and, ONLY after the task poll returns `Pending` (i.e. AFTER `block_on`'s
    // parking waker is registered on the task), signals "caller parked" to a releaser
    // thread. The releaser then -- and only then -- releases the gate, so the pool
    // task completes strictly AFTER the caller registered its waker. This forecloses
    // the "already-ready before await" interleaving: the task cannot be `Ready` on the
    // caller's first poll, because its gate is opened only in response to that first
    // `Pending` poll. The completion is thus forced to travel the real cross-thread
    // wake (pool worker completes task -> wakes parked caller -> `block_on` returns);
    // if that wake is broken the caller parks forever and the bounded recv_timeout
    // below fails loudly instead of hanging the suite.
    //
    // SCOPE: this is the cross-thread liveness guard only. It does NOT exercise the
    // caller-is-pool-worker reentrancy case (that is covered by RD-1's own reentrancy
    // tests).
    use std::sync::mpsc;
    use std::time::Duration;

    // Wraps the pool task so the caller's FIRST poll registers `block_on`'s waker on
    // the task and, only when that poll is `Pending`, hands the "caller parked" signal
    // to the releaser. Sending strictly after the `Pending` poll guarantees the waker
    // is already registered before the gate can open.
    struct CallerProbe {
      task: Task<usize>,
      parked_tx: Option<mpsc::Sender<()>>,
    }
    impl Future for CallerProbe {
      type Output = usize;
      fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<usize> {
        // `Task` is `Unpin`; poll it through a fresh `Pin`.
        let poll = Pin::new(&mut self.task).poll(cx);
        if poll.is_pending() {
          if let Some(tx) = self.parked_tx.take() {
            // Waker is now registered on `task`; release the gate is safe.
            let _ = tx.send(());
          }
        }
        poll
      }
    }

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd10-caller".to_string(),
        park_deadline: None,
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

      // The pool task's inner future is GATED on a oneshot: its first poll on a pool
      // worker registers the gate waker and returns `Pending`. When the releaser sends
      // on `gate_tx`, the gate's wake reschedules the task (via `schedule` ->
      // `ensure_drainer`) onto a pool worker, where it resolves to 42.
      let (gate_tx, gate_rx) = oneshot::channel::<usize>();
      let scheduler = Arc::clone(&executor);
      let (runnable, task) =
        async_task::spawn(async move { gate_rx.await.expect("gate sender dropped") }, move |r| {
          scheduler.schedule(r);
        });
      executor.schedule(runnable);

      // Releaser thread: opens the gate ONLY after the caller has parked (registered
      // its waker on the task). This enforces caller-parked HAPPENS-BEFORE pool-task
      // completes, so completion must travel the cross-thread wake path.
      let (parked_tx, parked_rx) = mpsc::channel::<()>();
      let releaser = std::thread::spawn(move || {
        if parked_rx.recv().is_ok() {
          let _ = gate_tx.send(42usize);
        }
      });

      // The caller (this child) thread is NOT a pool worker, so `block_on` parks via
      // `futures::executor::block_on` rather than driving the queue cooperatively.
      let mut output = None;
      {
        let mut future = std::pin::pin!(async {
          output = Some(CallerProbe { task, parked_tx: Some(parked_tx) }.await);
        });
        executor.block_on(future.as_mut());
      }

      releaser.join().unwrap();
      assert_eq!(output, Some(42usize), "block_on must return the pool-computed value");
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "RD-10: non-pool block_on did not receive the pool-produced value ({error}): the cross-thread wake path stalled"
      ),
    }
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
        park_deadline: None,
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

  /// Spin (yielding) until `condition` holds, failing loudly on timeout.
  /// Used to observe executor-internal states (e.g. "both drivers are
  /// registered as parked") without sleeps.
  #[cfg(not(target_family = "wasm"))]
  fn wait_until(what: &str, condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !condition() {
      assert!(std::time::Instant::now() < deadline, "timed out waiting for: {what}");
      std::thread::yield_now();
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn parked_drivers_wake_one_with_no_parked_drivers_skips_the_lock() {
    // Wake-path (a)/(b): with nothing registered, `wake_one` must return
    // `false` WITHOUT touching the registry mutex (the no-waiter fast path --
    // the common case, since queue-wakes fire on every schedule while drivers
    // park only in rare re-entrant `block_on`). Proxy for "did not take the
    // mutex": the test HOLDS the registry lock; a fast-path wake_one completes
    // anyway, while (control) a wake_one issued with a parker registered must
    // take the slow path and block on that held lock until it is released.
    use std::sync::mpsc;
    use std::time::Duration;

    let registry = Arc::new(ParkedDrivers::default());

    // Fast path: empty registry -> wake_one completes although the lock is held.
    let guard = registry.parked.lock().unwrap();
    let (done_tx, done_rx) = mpsc::channel();
    let waker_thread = {
      let registry = Arc::clone(&registry);
      std::thread::spawn(move || {
        done_tx.send(registry.wake_one()).unwrap();
      })
    };
    let woke = done_rx
      .recv_timeout(Duration::from_secs(5))
      .expect("wake_one with an empty registry must skip the lock (no-waiter fast path)");
    assert!(!woke, "an empty registry has nobody to wake");
    waker_thread.join().unwrap();
    drop(guard);

    // Control: with a parker registered the SAME held lock must block
    // wake_one (slow path). This proves the fast path above genuinely skipped
    // the lock rather than wake_one never locking at all.
    let parker = Arc::new(DriverParker::default());
    registry.register(&parker);
    let guard = registry.parked.lock().unwrap();
    let (done_tx, done_rx) = mpsc::channel();
    let waker_thread = {
      let registry = Arc::clone(&registry);
      std::thread::spawn(move || {
        done_tx.send(registry.wake_one()).unwrap();
      })
    };
    assert!(
      done_rx.recv_timeout(Duration::from_millis(300)).is_err(),
      "wake_one with a registered parker must take the slow path (block on the held lock)"
    );
    drop(guard);
    let woke = done_rx
      .recv_timeout(Duration::from_secs(5))
      .expect("slow-path wake_one must complete once the lock is released");
    assert!(woke, "the slow path must pop and wake the registered parker");
    waker_thread.join().unwrap();
    // The popped parker received its permit: a park now returns immediately
    // instead of sleeping (bounded by the test harness if broken).
    parker.park();
    assert_eq!(registry.count.load(Ordering::SeqCst), 0, "wake_one deregisters what it pops");
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn driver_parker_permit_granted_while_running_is_not_lost() {
    // Wake-path (b) §8.2: a wake delivered while the driver is RUNNING (not
    // parked) must be stored as a permit that makes the next `park` return
    // immediately; multiple such wakes coalesce into ONE permit.
    use std::sync::mpsc;
    use std::time::Duration;

    let parker = Arc::new(DriverParker::default());
    // Two wakes while "running" (nobody parked yet).
    parker.unpark();
    parker.unpark();

    let (tx, rx) = mpsc::channel();
    let driver = {
      let parker = Arc::clone(&parker);
      std::thread::spawn(move || {
        parker.park(); // must consume the stored permit without sleeping
        tx.send(1).unwrap();
        parker.park(); // no permit left: must genuinely sleep
        tx.send(2).unwrap();
      })
    };

    assert_eq!(
      rx.recv_timeout(Duration::from_secs(5)).expect("a stored permit must not be lost"),
      1
    );
    assert!(
      rx.recv_timeout(Duration::from_millis(300)).is_err(),
      "two pre-park wakes must coalesce into one permit (second park must sleep)"
    );
    parker.unpark();
    assert_eq!(
      rx.recv_timeout(Duration::from_secs(5)).expect("unpark must wake a sleeping driver"),
      2
    );
    driver.join().unwrap();
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn parked_drivers_wake_one_wakes_exactly_one() {
    // Wake-path (b): 2 parked drivers + 1 wake -> exactly one wakes, the
    // other stays parked. The old broadcast could not distinguish drivers;
    // targeted wakes must not degrade into "wake everyone" (that is the
    // measured churn) nor "wake no one" (lost wakeup).
    use std::sync::mpsc;
    use std::time::Duration;

    let registry = Arc::new(ParkedDrivers::default());
    let (woke_tx, woke_rx) = mpsc::channel();
    let mut drivers = Vec::new();
    for index in 0..2 {
      let registry = Arc::clone(&registry);
      let woke_tx = woke_tx.clone();
      drivers.push(std::thread::spawn(move || {
        let parker = Arc::new(DriverParker::default());
        registry.register(&parker);
        parker.park();
        registry.deregister(&parker);
        woke_tx.send(index).unwrap();
      }));
    }
    wait_until("both drivers registered as parked", || {
      registry.count.load(Ordering::SeqCst) == 2
    });

    assert!(registry.wake_one(), "wake_one must wake a parked driver");
    woke_rx.recv_timeout(Duration::from_secs(5)).expect("exactly one driver must wake");
    assert!(
      woke_rx.recv_timeout(Duration::from_millis(300)).is_err(),
      "the second driver must stay parked after a single wake_one"
    );
    assert_eq!(registry.count.load(Ordering::SeqCst), 1, "one driver must remain registered");

    assert!(registry.wake_one(), "a second wake_one must wake the remaining driver");
    woke_rx.recv_timeout(Duration::from_secs(5)).expect("the second driver must wake");
    for driver in drivers {
      driver.join().unwrap();
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_future_wake_targets_its_own_parked_driver() {
    // Wake-path (b) targeting: a future-wake must reach the driver that
    // AWAITS that future, not "some parked driver". D1 parks first awaiting
    // rx1; D2 parks after it awaiting rx2. Completing rx1 must wake D1 even
    // though D2 parked more recently -- a future-wake misrouted through the
    // registry (which pops most-recently-parked) would wake D2, leave D1
    // parked forever, and trip the bounded timeout. The old notify_all
    // broadcast never distinguished drivers, so this pins the new obligation.
    use std::sync::mpsc;
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "wake-target".to_string(),
        park_deadline: None,
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

      let (tx1, rx1) = oneshot::channel::<usize>();
      let (tx2, rx2) = oneshot::channel::<usize>();
      let t1_done = Arc::new(AtomicBool::new(false));
      let t2_done = Arc::new(AtomicBool::new(false));

      // T1: parks FIRST, awaiting rx1.
      let t1_exec = Arc::clone(&executor);
      let t1_flag = Arc::clone(&t1_done);
      let t1_body = async move {
        let mut inner = std::pin::pin!(async move {
          assert_eq!(rx1.await.unwrap(), 7usize);
        });
        t1_exec.block_on(inner.as_mut());
        t1_flag.store(true, Ordering::SeqCst);
      };
      let sched1 = Arc::clone(&executor);
      let (runnable1, task1) = async_task::spawn(t1_body, move |r| sched1.schedule(r));
      executor.schedule(runnable1);
      wait_until("driver 1 parked", || executor.parked_drivers.count.load(Ordering::SeqCst) == 1);

      // T2: parks SECOND (most recently). Its runnable is pushed RAW (no
      // `schedule`, so no wake_one can pop the already-parked D1) and a
      // drainer is spawned for it on the free second worker.
      let t2_exec = Arc::clone(&executor);
      let t2_flag = Arc::clone(&t2_done);
      let t2_body = async move {
        let mut inner = std::pin::pin!(async move {
          assert_eq!(rx2.await.unwrap(), 9usize);
        });
        t2_exec.block_on(inner.as_mut());
        t2_flag.store(true, Ordering::SeqCst);
      };
      let sched2 = Arc::clone(&executor);
      let (runnable2, task2) = async_task::spawn(t2_body, move |r| sched2.schedule(r));
      executor.metrics.runnable_scheduled(); // keep the raw push's accounting balanced
      executor.queue.lock().unwrap().push_back(runnable2);
      executor.ensure_drainer();
      wait_until("driver 2 parked", || executor.parked_drivers.count.load(Ordering::SeqCst) == 2);

      // Complete D1's future. The wake must land on D1 (first-parked), the
      // driver that owns rx1's waker -- not on the more recently parked D2.
      tx1.send(7usize).unwrap();
      wait_until("T1 completed via its own driver's wake", || t1_done.load(Ordering::SeqCst));
      futures::executor::block_on(task1);
      assert!(
        !t2_done.load(Ordering::SeqCst),
        "D2 must still be parked: its future was never completed"
      );
      assert_eq!(
        executor.parked_drivers.count.load(Ordering::SeqCst),
        1,
        "exactly the untargeted driver must remain parked"
      );

      // Teardown: release D2 as well.
      tx2.send(9usize).unwrap();
      futures::executor::block_on(task2);
      assert!(t2_done.load(Ordering::SeqCst));
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "future-wake did not reach its own parked driver ({error}): the wake was misdirected or lost"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_cooperative_exit_hands_absorbed_wake_to_parked_driver() {
    // Wake-path (b) miss compensation (§8.2): a driver that exits
    // `cooperative_block_on` (future ready) while queue work remains must hand
    // the wake on -- it may have absorbed a `wake_one` permit meant for that
    // work. Topology: the SOLE pool worker is parked as a cooperative driver
    // (so `ensure_drainer` can spawn nothing and only a wake can reach it); a
    // runnable is pushed RAW (no wake of its own); the test thread then runs a
    // ready future through the cooperative branch (the 1793 fake-on-pool
    // technique). Its exit compensation is the ONLY mechanism left that can
    // wake the parked worker to drain the runnable.
    use std::sync::mpsc;
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "wake-compensate".to_string(),
        park_deadline: None,
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

      // Park the sole worker as a cooperative driver awaiting a gate.
      let (gate_tx, gate_rx) = oneshot::channel::<()>();
      let td_exec = Arc::clone(&executor);
      let td_body = async move {
        let mut inner = std::pin::pin!(async move {
          gate_rx.await.unwrap();
        });
        td_exec.block_on(inner.as_mut());
      };
      let sched = Arc::clone(&executor);
      let (td_runnable, td_task) = async_task::spawn(td_body, move |r| sched.schedule(r));
      executor.schedule(td_runnable);
      wait_until("the sole driver parked", || {
        executor.parked_drivers.count.load(Ordering::SeqCst) == 1
      });

      // Strand a runnable: pushed RAW, so no wake accompanies it, and the
      // sole worker is parked-as-drainer, so `ensure_drainer` cannot help.
      let (ran_tx, ran_rx) = mpsc::channel::<()>();
      let sched_stranded = Arc::clone(&executor);
      let (stranded_runnable, stranded_task) = async_task::spawn(
        async move {
          ran_tx.send(()).unwrap();
        },
        move |r| sched_stranded.schedule(r),
      );
      executor.metrics.runnable_scheduled(); // keep the raw push's accounting balanced
      executor.queue.lock().unwrap().push_back(stranded_runnable);
      stranded_task.detach();

      // Drive a ready future through the cooperative branch on THIS thread
      // (fake on-pool marker, as in the executor-scoping test). Its first
      // poll is Ready, so the loop performs no work itself -- only the exit
      // compensation can hand the queued runnable to the parked driver.
      {
        let _on_pool = OnPoolWorkerGuard::enter(executor.id);
        let mut ready = std::pin::pin!(async {});
        executor.block_on(ready.as_mut());
      }

      ran_rx.recv_timeout(Duration::from_secs(5)).expect(
        "exit compensation must wake the parked driver to run the stranded queued runnable",
      );

      // Teardown: release the parked driver.
      gate_tx.send(()).unwrap();
      futures::executor::block_on(td_task);
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("cooperative-exit miss compensation failed ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn wake_path_wake_one_vs_park_interleaving_loses_no_wakeups() {
    // Wake-path (a)/(b) stress: N rounds of {producer: publish work,
    // wake_one()} racing {consumer: check work, register, re-check, park} --
    // the exact schedule-vs-park protocol `schedule` and
    // `cooperative_block_on` run. This walks the interleaving at every offset
    // the OS scheduler produces; a single lost wakeup parks the consumer
    // forever with work pending, and the bounded recv_timeout turns that into
    // a loud failure instead of a hung suite. (No loom in this repo; std
    // threads.)
    use std::sync::mpsc;
    use std::time::Duration;

    const ROUNDS: usize = 20_000;

    let (done_tx, done_rx) = mpsc::channel();
    let registry = Arc::new(ParkedDrivers::default());
    let work = Arc::new(AtomicBool::new(false));

    let consumer = {
      let registry = Arc::clone(&registry);
      let work = Arc::clone(&work);
      std::thread::spawn(move || {
        let parker = Arc::new(DriverParker::default());
        for _ in 0..ROUNDS {
          loop {
            if work.swap(false, Ordering::SeqCst) {
              break;
            }
            // The cooperative park protocol: register FIRST, then re-check,
            // then park (see `ParkedDrivers::wake_one`).
            registry.register(&parker);
            if work.load(Ordering::SeqCst) {
              registry.deregister(&parker);
              continue;
            }
            parker.park();
            registry.deregister(&parker);
          }
        }
        done_tx.send(()).unwrap();
      })
    };

    for _ in 0..ROUNDS {
      work.store(true, Ordering::SeqCst);
      // The schedule side: publish work, then wake one parked driver. When
      // wake_one misses (consumer not registered yet), the consumer's
      // post-register re-check must see the work instead.
      let _ = registry.wake_one();
      while work.load(Ordering::SeqCst) {
        std::hint::spin_loop();
      }
    }

    done_rx
      .recv_timeout(Duration::from_secs(30))
      .expect("a wake_one()-side wakeup was lost: the consumer is parked with work pending");
    consumer.join().unwrap();
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_same_worker_schedule_runs_before_older_fifo_work() {
    // Wake-path (c): a runnable scheduled from a pool worker of the SAME
    // executor lands in that worker's LIFO slot (no FIFO push, no wake) and is
    // run by the next drain/run_one iteration BEFORE older shared-FIFO work.
    let options = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "lifo-order".to_string(),
      park_deadline: None,
    };
    let executor =
      Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
    let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

    // Older work, pushed RAW onto the FIFO (so no drainer is spawned that
    // could race the manual `run_one` calls below).
    let old_order = Arc::clone(&order);
    let sched = Arc::clone(&executor);
    let (old_runnable, old_task) = async_task::spawn(
      async move {
        old_order.lock().unwrap().push("old");
      },
      move |r| sched.schedule(r),
    );
    executor.metrics.runnable_scheduled(); // keep the raw push's accounting balanced
    executor.queue.lock().unwrap().push_back(old_runnable);
    old_task.detach();

    // Simulate being on this executor's pool worker: the schedule below must
    // take the slot path (which never touches the FIFO nor spawns a drainer).
    let _on_pool = OnPoolWorkerGuard::enter(executor.id);
    let new_order = Arc::clone(&order);
    let sched = Arc::clone(&executor);
    let (new_runnable, new_task) = async_task::spawn(
      async move {
        new_order.lock().unwrap().push("new");
      },
      move |r| sched.schedule(r),
    );
    executor.schedule(new_runnable);
    new_task.detach();
    assert_eq!(
      executor.queue.lock().unwrap().len(),
      1,
      "the slot-path schedule must not push to the shared FIFO"
    );

    assert!(executor.run_one(), "run_one must pop the slot first");
    assert!(executor.run_one(), "run_one must then pop the FIFO");
    assert!(!executor.run_one(), "no work must remain");
    assert_eq!(
      *order.lock().unwrap(),
      vec!["new", "old"],
      "the slot runnable (newest) must run before older FIFO work"
    );
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_drain_runs_slot_before_older_fifo_work() {
    // Wake-path (c), `drain` integration: with one worker, task P scheduling
    // child C mid-drain must see C run IMMEDIATELY after P (via the slot),
    // jumping ahead of the older queued T1/T2.
    use std::sync::mpsc;
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "lifo-drain".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
      let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

      let p_exec = Arc::clone(&executor);
      let p_order = Arc::clone(&order);
      let p_body = async move {
        p_order.lock().unwrap().push("P");
        // Scheduled from the pool worker mid-drain: must land in the slot.
        let c_order = Arc::clone(&p_order);
        let sched = Arc::clone(&p_exec);
        let (c_runnable, c_task) = async_task::spawn(
          async move {
            c_order.lock().unwrap().push("C");
          },
          move |r| sched.schedule(r),
        );
        p_exec.schedule(c_runnable);
        c_task.detach();
      };

      // Push P, T1, T2 RAW (deterministic FIFO order, exactly one drainer),
      // mirroring the `runnable_scheduled` accounting a real schedule does so
      // the queued-runnables counter stays balanced.
      let mut tasks = Vec::new();
      let sched = Arc::clone(&executor);
      let (p_runnable, p_task) = async_task::spawn(p_body, move |r| sched.schedule(r));
      executor.metrics.runnable_scheduled();
      executor.queue.lock().unwrap().push_back(p_runnable);
      tasks.push(p_task);
      for name in ["T1", "T2"] {
        let t_order = Arc::clone(&order);
        let sched = Arc::clone(&executor);
        let (t_runnable, t_task) = async_task::spawn(
          async move {
            t_order.lock().unwrap().push(name);
          },
          move |r| sched.schedule(r),
        );
        executor.metrics.runnable_scheduled();
        executor.queue.lock().unwrap().push_back(t_runnable);
        tasks.push(t_task);
      }
      executor.ensure_drainer();

      for task in tasks {
        futures::executor::block_on(task);
      }
      wait_until("the detached child C ran", || order.lock().unwrap().len() == 4);
      assert_eq!(
        *order.lock().unwrap(),
        vec!["P", "C", "T1", "T2"],
        "the slot child C must run right after its scheduler P, ahead of older FIFO work"
      );
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("LIFO drain-order test did not complete ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_displaced_occupant_falls_back_to_fifo() {
    // Wake-path (c): two same-executor schedules from one worker -- the
    // NEWEST takes the slot; the displaced occupant must land on the shared
    // FIFO (with a wake), never be dropped. `active_drainers` is saturated so
    // the displacement wake cannot spawn a real drainer racing the manual
    // `run_one` calls.
    let options = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "lifo-displace".to_string(),
      park_deadline: None,
    };
    let executor =
      Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
    executor.active_drainers.store(executor.max_drainers, Ordering::SeqCst);
    let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

    let _on_pool = OnPoolWorkerGuard::enter(executor.id);
    for name in ["a", "b"] {
      let n_order = Arc::clone(&order);
      let sched = Arc::clone(&executor);
      let (runnable, task) = async_task::spawn(
        async move {
          n_order.lock().unwrap().push(name);
        },
        move |r| sched.schedule(r),
      );
      executor.schedule(runnable);
      task.detach();
    }
    assert_eq!(
      executor.queue.lock().unwrap().len(),
      1,
      "the displaced occupant (a) must have been pushed to the shared FIFO"
    );

    assert!(executor.run_one(), "run_one must pop the slot (b) first");
    assert!(executor.run_one(), "run_one must then pop the displaced (a) from the FIFO");
    assert!(!executor.run_one(), "no work must remain");
    assert_eq!(
      *order.lock().unwrap(),
      vec!["b", "a"],
      "the newest schedule must win the slot; the displaced one must survive via the FIFO"
    );
    executor.active_drainers.store(0, Ordering::SeqCst);
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_is_executor_scoped() {
    // Wake-path (c): the slot is tagged with the executor id (mirroring the
    // ON_POOL_WORKER / BlockingOwnerToken scoping). A worker of executor A
    // scheduling onto executor B must bypass the slot (B's FIFO); and with
    // A's runnable IN the slot, B must neither pop nor displace it.
    let options = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "lifo-scope".to_string(),
      park_deadline: None,
    };
    let exec1 =
      Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
    let exec2 =
      Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
    assert_ne!(exec1.id, exec2.id, "executors must get distinct ids");
    exec1.active_drainers.store(exec1.max_drainers, Ordering::SeqCst);
    exec2.active_drainers.store(exec2.max_drainers, Ordering::SeqCst);
    let order = Arc::new(Mutex::new(Vec::<&'static str>::new()));

    // As exec1's worker: r1 goes to the slot, tagged exec1.
    let _on_pool1 = OnPoolWorkerGuard::enter(exec1.id);
    let r1_order = Arc::clone(&order);
    let sched = Arc::clone(&exec1);
    let (r1, t1) = async_task::spawn(
      async move {
        r1_order.lock().unwrap().push("one");
      },
      move |r| sched.schedule(r),
    );
    exec1.schedule(r1);
    t1.detach();
    assert!(exec1.queue.lock().unwrap().is_empty(), "r1 must sit in the slot, not exec1's FIFO");

    {
      // Nested: now act as exec2's worker while exec1's runnable occupies the
      // slot. exec2 must not displace the foreign entry; r2 goes to its FIFO.
      let _on_pool2 = OnPoolWorkerGuard::enter(exec2.id);
      let r2_order = Arc::clone(&order);
      let sched = Arc::clone(&exec2);
      let (r2, t2) = async_task::spawn(
        async move {
          r2_order.lock().unwrap().push("two");
        },
        move |r| sched.schedule(r),
      );
      exec2.schedule(r2);
      t2.detach();
      assert_eq!(
        exec2.queue.lock().unwrap().len(),
        1,
        "a foreign-occupied slot must route exec2's schedule to exec2's FIFO"
      );
      assert!(
        exec2.pop_lifo_slot().is_none(),
        "exec2 must not pop exec1's slot entry (executor-scoped pop)"
      );
      assert!(exec2.run_one(), "exec2 must drive r2 from its own FIFO");
    }

    // Back as exec1's worker: its slot entry is intact and runnable.
    assert!(exec1.run_one(), "exec1's slot entry must have survived the foreign activity");
    assert_eq!(*order.lock().unwrap(), vec!["two", "one"]);
    exec1.active_drainers.store(0, Ordering::SeqCst);
    exec2.active_drainers.store(0, Ordering::SeqCst);
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_flushed_on_budget_exhaustion_completes_long_chains() {
    // Wake-path (c) §8.3: a 200-link chain in which each task schedules the
    // next FROM the pool worker (every link lands in the slot) crosses
    // multiple RUNNABLE_BUDGET windows. At each exhaustion the slot MUST be
    // flushed to the FIFO before `finish_draining` (so its re-check sees the
    // runnable and re-arms a drainer); a missing flush strands the chain at
    // the first budget boundary (lost task + queued_runnables leak).
    use std::sync::mpsc;
    use std::time::Duration;

    const CHAIN: u64 = 200; // > RUNNABLE_BUDGET (64)

    fn spawn_link(
      executor: &Arc<MultiThreadExecutor>,
      remaining: u64,
      counter: Arc<AtomicU64>,
      done: mpsc::Sender<()>,
    ) {
      let body_exec = Arc::clone(executor);
      let body = async move {
        counter.fetch_add(1, Ordering::SeqCst);
        if remaining > 0 {
          spawn_link(&body_exec, remaining - 1, counter, done);
        } else {
          done.send(()).unwrap();
        }
      };
      let sched = Arc::clone(executor);
      let (runnable, task) = async_task::spawn(body, move |r| sched.schedule(r));
      executor.schedule(runnable);
      task.detach();
    }

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "lifo-budget".to_string(),
        park_deadline: None,
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, Arc::clone(&metrics)).unwrap());

      let counter = Arc::new(AtomicU64::new(0));
      let (chain_tx, chain_rx) = mpsc::channel();
      spawn_link(&executor, CHAIN - 1, Arc::clone(&counter), chain_tx);

      chain_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the slot chain stalled at a budget boundary: slot not flushed on drain exit");
      assert_eq!(counter.load(Ordering::SeqCst), CHAIN, "every chain link must have run");
      wait_until("runnable counters settle to zero (no slot leak)", || {
        metrics.queued_runnables.load(Ordering::Relaxed) == 0
          && metrics.active_runnables.load(Ordering::Relaxed) == 0
      });
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(15)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("LIFO budget-flush chain test did not complete ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_flush_moves_slot_runnable_to_shared_fifo() {
    // Wake-path (c) §8.3: direct pin for the flush primitive used at drain
    // exits, before cooperative parks and by the unwind guard -- a
    // slot-resident runnable moves to the shared FIFO with its task still
    // completable, never dropped. (The pre-park call is defensive: `run_one`
    // pops the slot before any park is reachable, so this primitive is its
    // only observable seam.)
    let options = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "lifo-flush".to_string(),
      park_deadline: None,
    };
    let executor =
      Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
    executor.active_drainers.store(executor.max_drainers, Ordering::SeqCst);
    let ran = Arc::new(AtomicBool::new(false));

    let _on_pool = OnPoolWorkerGuard::enter(executor.id);
    let ran_flag = Arc::clone(&ran);
    let sched = Arc::clone(&executor);
    let (runnable, task) = async_task::spawn(
      async move {
        ran_flag.store(true, Ordering::SeqCst);
      },
      move |r| sched.schedule(r),
    );
    executor.schedule(runnable);
    task.detach();
    assert!(executor.queue.lock().unwrap().is_empty(), "the runnable must start in the slot");

    executor.flush_lifo_slot();
    assert!(executor.pop_lifo_slot().is_none(), "the slot must be empty after a flush");
    assert_eq!(
      executor.queue.lock().unwrap().len(),
      1,
      "the flushed runnable must be on the shared FIFO"
    );
    assert!(executor.run_one(), "the flushed runnable must still run");
    assert!(ran.load(Ordering::SeqCst), "the flushed task must complete, not be dropped");
    executor.active_drainers.store(0, Ordering::SeqCst);
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_runnable_does_not_inherit_blocking_owner() {
    // Wake-path (c), extending the RD-1 round-3 owner-clear (the 628 anchor)
    // to the slot path: a runnable popped from the slot runs with
    // BLOCKING_OWNER cleared, so it cannot borrow the driving owner frame's
    // over-cap privilege; the ambient owner is restored afterwards.
    let options = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "lifo-owner".to_string(),
      park_deadline: None,
    };
    let executor =
      Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
    let observed = Arc::new(Mutex::new(None::<Option<BlockingOwnerToken>>));

    let _on_pool = OnPoolWorkerGuard::enter(executor.id);

    // Install the slot entry from RUNNABLE context (no ambient owner) -- the
    // only context that may use the slot since the finding-3 fix diverts
    // schedules issued under an owner frame to the shared FIFO.
    let observed_slot = Arc::clone(&observed);
    let sched = Arc::clone(&executor);
    let (runnable, task) = async_task::spawn(
      async move {
        *observed_slot.lock().unwrap() = Some(BLOCKING_OWNER.with(std::cell::Cell::get));
      },
      move |r| sched.schedule(r),
    );
    executor.schedule(runnable);
    task.detach();
    assert!(executor.queue.lock().unwrap().is_empty(), "the runnable must sit in the slot");

    // The POP can still happen while an owner frame is ambient (a cooperative
    // `block_on` entered from a blocking closure drives `run_one` with the
    // closure's token on the stack); the slot branch must clear it.
    let token = executor.fresh_owner_token();
    let _owner = BlockingOwnerGuard::enter(Some(token));
    assert!(executor.run_one(), "the slot runnable must run");
    assert_eq!(
      *observed.lock().unwrap(),
      Some(None),
      "a slot runnable must observe a CLEARED blocking owner"
    );
    assert_eq!(
      BLOCKING_OWNER.with(std::cell::Cell::get),
      Some(token),
      "the ambient owner frame must be restored after the slot runnable"
    );
  }

  // Shared ping-pong probe for the LIFO fairness tests: two tasks that wake
  // each other on every poll. Scheduled from a pool worker, each wake lands in
  // that worker's LIFO slot, keeping the slot hot indefinitely. A task
  // finishes when `stop` is raised (or `stop_after` polls happened), releasing
  // its partner's stored waker so both complete.
  #[cfg(not(target_family = "wasm"))]
  struct PingPongShared {
    count: AtomicU64,
    stop: AtomicBool,
    stop_after: u64,
    wakers: Mutex<[Option<std::task::Waker>; 2]>,
  }

  #[cfg(not(target_family = "wasm"))]
  struct PingPong {
    shared: Arc<PingPongShared>,
    me: usize,
  }

  #[cfg(not(target_family = "wasm"))]
  impl Future for PingPong {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
      let shared = &self.shared;
      let polls = shared.count.fetch_add(1, Ordering::SeqCst);
      if shared.stop.load(Ordering::SeqCst) || polls >= shared.stop_after {
        let partner = shared.wakers.lock().unwrap()[1 - self.me].take();
        if let Some(partner) = partner {
          partner.wake();
        }
        return Poll::Ready(());
      }
      let mut wakers = shared.wakers.lock().unwrap();
      wakers[self.me] = Some(cx.waker().clone());
      if let Some(partner) = wakers[1 - self.me].take() {
        drop(wakers);
        partner.wake();
      }
      Poll::Pending
    }
  }

  #[cfg(not(target_family = "wasm"))]
  fn spawn_ping_pong_pair(
    executor: &Arc<MultiThreadExecutor>,
    shared: &Arc<PingPongShared>,
  ) -> Vec<Task<()>> {
    let mut tasks = Vec::new();
    for me in 0..2 {
      let sched = Arc::clone(executor);
      let (runnable, task) =
        async_task::spawn(PingPong { shared: Arc::clone(shared), me }, move |r| sched.schedule(r));
      executor.schedule(runnable);
      tasks.push(task);
    }
    tasks
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn lifo_slot_ping_pong_does_not_starve_queued_fifo_task() {
    // Wake-path (c) streak cap: slot pops share `drain`'s RUNNABLE_BUDGET, so
    // a hot wake/await pair holds a worker for at most one budget window
    // before the slot is flushed and the FIFO gets its turn. With ONE worker,
    // a third task queued behind an unbounded ping-pong pair must still run;
    // if slot pops bypassed the budget the drain would never exit and T3
    // would starve forever (caught by the bounded timeout).
    use std::sync::mpsc;
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "lifo-streak".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let shared = Arc::new(PingPongShared {
        count: AtomicU64::new(0),
        stop: AtomicBool::new(false),
        stop_after: u64::MAX,
        wakers: Mutex::new([None, None]),
      });
      let pair = spawn_ping_pong_pair(&executor, &shared);

      // T3: queued on the shared FIFO behind the hot pair.
      let (t3_tx, t3_rx) = mpsc::channel::<()>();
      let sched = Arc::clone(&executor);
      let (t3_runnable, t3_task) = async_task::spawn(
        async move {
          t3_tx.send(()).unwrap();
        },
        move |r| sched.schedule(r),
      );
      executor.schedule(t3_runnable);

      t3_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("a hot LIFO pair starved the FIFO: the budget streak cap is broken");
      assert!(
        shared.count.load(Ordering::SeqCst) >= 32,
        "the pair must have been bouncing through the slot before T3 got its turn"
      );

      // Teardown: stop the pair and release whichever side is waiting.
      shared.stop.store(true, Ordering::SeqCst);
      let stored: Vec<_> =
        shared.wakers.lock().unwrap().iter_mut().filter_map(Option::take).collect();
      for waker in stored {
        waker.wake();
      }
      for task in pair {
        futures::executor::block_on(task);
      }
      futures::executor::block_on(t3_task);
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(15)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("LIFO streak-cap test did not complete ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_hot_lifo_pair_does_not_wedge_queued_blocking_job() {
    // Wake-path §8.7: runnables (slot included) outrank blocking work per
    // drain iteration -- unchanged from the FIFO-only drain -- but a hot LIFO
    // pair must not turn that priority into a permanent wedge. The budget
    // flush keeps the drain exiting, `finish_draining` keeps re-checking the
    // blocking queue, and the queued blocking job completes once the pair
    // finishes its (long) run on the SOLE worker.
    use std::sync::mpsc;
    use std::time::Duration;

    const BOUNCES: u64 = 2_000; // ~31 budget windows on one worker

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "lifo-blocking".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let shared = Arc::new(PingPongShared {
        count: AtomicU64::new(0),
        stop: AtomicBool::new(false),
        stop_after: BOUNCES,
        wakers: Mutex::new([None, None]),
      });
      let pair = spawn_ping_pong_pair(&executor, &shared);

      // Queued while the pair keeps the slot hot on the only worker.
      let blocking = executor.schedule_blocking(|| 11usize);

      assert_eq!(
        futures::executor::block_on(blocking).unwrap(),
        11,
        "the queued blocking job must complete despite the hot LIFO pair"
      );
      for task in pair {
        futures::executor::block_on(task);
      }
      assert!(
        shared.count.load(Ordering::SeqCst) >= BOUNCES,
        "the pair must have actually run its full bounce quota"
      );
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(15)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("LIFO blocking-fairness test did not complete ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn cooperative_exit_flushes_slot_work_spawned_by_final_poll() {
    // Codex round-1 finding 1: the awaited future's FINAL poll (the one that
    // returns Ready) can schedule same-executor work, which lands in THIS
    // worker's LIFO slot -- no FIFO push, no wake. The Ready exit of
    // `cooperative_block_on` must flush the slot: the caller may wait on that
    // child synchronously (and the enclosing drain frame can be arbitrarily
    // far away -- here it does not exist at all, via the fake on-pool
    // marker), so a stranded slot entry is an invisible, dead task.
    use std::sync::mpsc;
    use std::time::Duration;

    struct SpawnOnReady {
      executor: Arc<MultiThreadExecutor>,
      child_tx: Option<mpsc::Sender<u64>>,
    }
    impl Future for SpawnOnReady {
      type Output = ();
      fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let child_tx = self.child_tx.take().expect("polled once");
        let sched = Arc::clone(&self.executor);
        let (runnable, task) = async_task::spawn(
          async move {
            child_tx.send(21).unwrap();
          },
          move |r| sched.schedule(r),
        );
        // On-pool + same executor + no ambient owner: lands in the slot.
        self.executor.schedule(runnable);
        task.detach();
        Poll::Ready(())
      }
    }

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "exit-flush".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let (child_tx, child_rx) = mpsc::channel::<u64>();
      {
        // Fake on-pool marker (1793 technique): the cooperative branch runs on
        // THIS thread, with NO enclosing drain loop to bail the slot out.
        let _on_pool = OnPoolWorkerGuard::enter(executor.id);
        let mut future =
          std::pin::pin!(SpawnOnReady { executor: Arc::clone(&executor), child_tx: Some(child_tx) });
        executor.block_on(future.as_mut() as Pin<&mut dyn Future<Output = ()>>);
      }

      // Synchronous bounded wait, NOT via the executor: only the exit flush
      // (slot -> shared FIFO -> wake/ensure_drainer -> pool worker) can
      // complete the child.
      let value = child_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the child scheduled by the final poll was stranded in the LIFO slot");
      assert_eq!(value, 21);
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("cooperative-exit slot-flush test did not complete ({error})"),
    }
  }

  /// Perpetual same-executor chain: each link schedules its successor from
  /// the pool worker (so every link lands in the LIFO slot) until `stop` is
  /// raised. `hops` counts executed links.
  #[cfg(not(target_family = "wasm"))]
  fn spawn_hot_chain(
    executor: &Arc<MultiThreadExecutor>,
    stop: Arc<AtomicBool>,
    hops: Arc<AtomicU64>,
  ) {
    if stop.load(Ordering::SeqCst) {
      return;
    }
    let link_exec = Arc::clone(executor);
    let body = async move {
      hops.fetch_add(1, Ordering::SeqCst);
      spawn_hot_chain(&link_exec, stop, hops);
    };
    let sched = Arc::clone(executor);
    let (runnable, task) = async_task::spawn(body, move |r| sched.schedule(r));
    executor.schedule(runnable);
    task.detach();
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn cooperative_block_on_budget_prevents_slot_chain_starving_fifo() {
    // Codex round-1 finding 2: `cooperative_block_on`'s loop must mirror
    // `drain`'s RUNNABLE_BUDGET streak cap. `run_one` pops the slot before
    // the FIFO, so a hot chain (every link schedules its successor into the
    // slot) keeps run_one succeeding forever and the FIFO is never reached
    // from this thread. On a 1-worker pool (legal config) a FIFO task that
    // completes the awaited future then never runs: deadlock. The budgeted
    // loop must flush the slot every RUNNABLE_BUDGET runs so the FIFO task
    // gets its turn.
    use std::sync::mpsc;
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "coop-budget".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let stop = Arc::new(AtomicBool::new(false));
      let hops = Arc::new(AtomicU64::new(0));

      // M runs on the sole worker: it queues F (the task that completes the
      // awaited future) on the FIFO, heats the slot with the chain, then
      // block_on's the value F produces.
      let m_exec = Arc::clone(&executor);
      let m_stop = Arc::clone(&stop);
      let m_hops = Arc::clone(&hops);
      let m_body = async move {
        let (tx, rx) = oneshot::channel::<u64>();
        // Schedule F first: it takes the slot momentarily...
        let sched_f = Arc::clone(&m_exec);
        let (f_runnable, f_task) = async_task::spawn(
          async move {
            let _ = tx.send(7);
          },
          move |r| sched_f.schedule(r),
        );
        m_exec.schedule(f_runnable);
        f_task.detach();
        // ...then the chain starter displaces it to the FIFO and keeps the
        // slot hot from here on.
        spawn_hot_chain(&m_exec, Arc::clone(&m_stop), m_hops);

        let mut inner = std::pin::pin!(async move {
          assert_eq!(rx.await.unwrap(), 7, "F must run and complete the awaited future");
        });
        m_exec.block_on(inner.as_mut());
        m_stop.store(true, Ordering::SeqCst);
      };
      let sched_m = Arc::clone(&executor);
      let (m_runnable, m_task) = async_task::spawn(m_body, move |r| sched_m.schedule(r));
      executor.schedule(m_runnable);

      futures::executor::block_on(m_task);
      assert!(
        hops.load(Ordering::SeqCst) >= 64,
        "the chain must have been genuinely hot (>= one budget window) before F ran"
      );
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "cooperative block_on starved the FIFO behind a hot slot chain ({error}): the loop needs the RUNNABLE_BUDGET streak cap"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn blocking_closure_spawn_is_not_stranded_in_slot() {
    // Codex round-1 finding 3: ON_POOL_WORKER spans the whole drain frame,
    // INCLUDING blocking-closure bodies. A closure that spawns a same-
    // executor task and then waits for it synchronously would deadlock if the
    // child were slotted (the slot only drains after the closure returns --
    // which is what the child was supposed to unblock), even with idle
    // workers. `schedule` must bypass the slot while BLOCKING_OWNER is set
    // (straight-line blocking code) and push to the shared FIFO with a wake.
    use std::sync::mpsc;
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "blk-spawn-wait".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      // Closure spawns a child and BLOCKS on the channel the child must fill.
      // A spare worker exists; only a real FIFO push + wake can reach it.
      let blk_exec = Arc::clone(&executor);
      let handle = executor.schedule_blocking(move || {
        let (tx, rx) = mpsc::channel::<u64>();
        let sched = Arc::clone(&blk_exec);
        let (runnable, task) = async_task::spawn(
          async move {
            tx.send(21).unwrap();
          },
          move |r| sched.schedule(r),
        );
        blk_exec.schedule(runnable);
        task.detach();
        rx.recv_timeout(Duration::from_secs(5))
          .expect("child spawned by a blocking closure was stranded in the worker's LIFO slot")
      });
      assert_eq!(futures::executor::block_on(handle).unwrap(), 21);

      // Variant: the closure exits WITHOUT waiting; the child must still
      // complete promptly (no stranding on the closure-exit path either).
      let (v_tx, v_rx) = mpsc::channel::<u64>();
      let v_exec = Arc::clone(&executor);
      let v_handle = executor.schedule_blocking(move || {
        let sched = Arc::clone(&v_exec);
        let (runnable, task) = async_task::spawn(
          async move {
            v_tx.send(42).unwrap();
          },
          move |r| sched.schedule(r),
        );
        v_exec.schedule(runnable);
        task.detach();
        1u64
      });
      assert_eq!(futures::executor::block_on(v_handle).unwrap(), 1);
      assert_eq!(
        v_rx.recv_timeout(Duration::from_secs(5)).expect("fire-and-forget child must complete"),
        42
      );
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(15)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("blocking-closure spawn test did not complete ({error})"),
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
        park_deadline: None,
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
        park_deadline: None,
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
        park_deadline: None,
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
        park_deadline: None,
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
    use std::sync::{Barrier, mpsc};

    // Regression for RD-1 (B) MEDIUM: the over-cap escape must be both per-frame
    // (token-scoped) and per-executor (executor-scoped). This drives the SAME
    // production seam `cooperative_block_on` uses -- `try_owned_blocking_over_cap`
    // -- through the ambient `BLOCKING_OWNER` thread-local, so deleting the
    // `executor_id` gate inside it FAILS this test (the gate is exercised, not
    // restated). exec2's blocking cap is kept saturated by parked holders for the
    // whole test so `take_blocking` always returns None: no background drainer can
    // pop an injected job before we inspect/escape it, while the over-cap escape
    // (which bypasses the cap) still runs. This makes every assertion below
    // deterministic.
    let opts = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "rd1-scope".to_string(),
      park_deadline: None,
    };
    let exec1 =
      Arc::new(MultiThreadExecutor::new(&opts, Arc::new(RuntimeMetrics::default())).unwrap());
    let exec2 =
      Arc::new(MultiThreadExecutor::new(&opts, Arc::new(RuntimeMetrics::default())).unwrap());
    assert_ne!(exec1.id, exec2.id, "executors must get distinct ids");

    // Saturate exec2's blocking cap with parked holders. Each signals once it holds
    // a counted slot, then waits on the release barrier (held to the end of test).
    let holder_count = opts.max_blocking_tasks; // saturates the cap
    let (held_tx, held_rx) = mpsc::channel();
    let release = Arc::new(Barrier::new(holder_count + 1));
    let mut holder_handles = Vec::new();
    for _ in 0..holder_count {
      let held_tx = held_tx.clone();
      let release = Arc::clone(&release);
      holder_handles.push(exec2.schedule_blocking(move || {
        held_tx.send(()).unwrap();
        release.wait();
        0usize
      }));
    }
    for _ in 0..holder_count {
      held_rx.recv().unwrap();
    }
    // Cap is now fully saturated; `take_blocking` returns None from here on.
    assert_eq!(exec2.active_blocking.load(Ordering::Acquire), opts.max_blocking_tasks);

    // (1) `schedule_blocking` tags ONLY same-executor owners. With exec1's token
    // ambient, a job scheduled on exec2 is a foreign-executor job and must be
    // tagged `None`, so no over-cap escape can ever match it. The saturated cap
    // guarantees no drainer pops it before we read its tag.
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
    exec2.blocking_queue.lock().unwrap().clear();

    // (2) Drive the PRODUCTION executor-id gate through the ambient thread-local.
    // Inject a job tagged with exec1's FOREIGN token into exec2's queue. With that
    // SAME foreign token ambient, `try_owned_blocking_over_cap` must fail the
    // `executor_id` gate and leave the job untouched. Were the gate deleted, the
    // token-exact scan WOULD match the exec1-tagged job under the exec1 ambient
    // token and run it -- so this is an honest regression guard for the gate.
    let ran = Arc::new(AtomicBool::new(false));
    let ran_job = Arc::clone(&ran);
    exec2.blocking_queue.lock().unwrap().push_back(QueuedBlocking {
      owner: Some(token1),
      run: Box::new(move || ran_job.store(true, Ordering::SeqCst)),
    });
    {
      let _owner = BlockingOwnerGuard::enter(Some(token1));
      assert!(
        !exec2.try_owned_blocking_over_cap(),
        "a foreign-executor ambient token must fail the executor gate"
      );
    }
    assert!(!ran.load(Ordering::SeqCst), "foreign-tagged job must not have run");
    assert_eq!(
      exec2.blocking_queue.lock().unwrap().len(),
      1,
      "the gate-rejected job must be left in exec2's queue"
    );

    // Per-frame token scan: exec2's OWN token PASSES the executor gate but still
    // must not match the exec1-tagged job -- the scan is token-exact, not merely
    // executor-wide.
    {
      let _owner = BlockingOwnerGuard::enter(Some(exec2.fresh_owner_token()));
      assert!(
        !exec2.try_owned_blocking_over_cap(),
        "an exec2 token must not match an exec1-tagged job (token-exact scan)"
      );
    }
    assert!(!ran.load(Ordering::SeqCst), "token-mismatched job must not have run");
    exec2.blocking_queue.lock().unwrap().clear();

    // MATCHING ambient token: a job tagged with exec2's own frame is run by the
    // production seam (gate passes AND the token-exact scan matches), even over the
    // saturated cap -- the genuine nested-blocking escape (RD-1 (A)).
    let token2 = exec2.fresh_owner_token();
    let ran2 = Arc::new(AtomicBool::new(false));
    let ran2_job = Arc::clone(&ran2);
    exec2.blocking_queue.lock().unwrap().push_back(QueuedBlocking {
      owner: Some(token2),
      run: Box::new(move || ran2_job.store(true, Ordering::SeqCst)),
    });
    {
      let _owner = BlockingOwnerGuard::enter(Some(token2));
      assert!(
        exec2.try_owned_blocking_over_cap(),
        "a matching same-executor ambient token must run its own queued job over the cap"
      );
    }
    assert!(ran2.load(Ordering::SeqCst), "matching-token job must have run");

    // Release the holders and drain their join handles for a clean teardown.
    release.wait();
    for handle in holder_handles {
      assert_eq!(futures::executor::block_on(handle).unwrap(), 0usize);
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_block_on_cooperative_branch_is_executor_scoped() {
    // Regression for the pool-worker-marker scoping gate: `block_on` may take the
    // cooperative (queue-driving) branch ONLY for a re-entrant call OF THE SAME
    // executor whose worker set the marker. A worker of executor A that re-enters
    // `block_on` on executor B must take the PARKING branch instead -- it owns
    // none of B's accounting, so driving B's queue would make it an unaccounted
    // within-cap driver. This mirrors the `BlockingOwnerToken.executor_id` gate.
    //
    // Fully deterministic, no sleeps/threads: we set the ambient pool-worker
    // marker to exec1's id by entering the REAL `OnPoolWorkerGuard`, then call
    // `block_on` on each executor with the SAME probe shape and observe whether
    // the cooperative `run_one` driver actually ran a queued runnable.
    //
    // The probe future returns `Pending` once (self-waking so the parking branch
    // also makes progress) then `Ready`, guaranteeing the cooperative loop reaches
    // `run_one` at least once before the future completes. A single runnable is
    // pushed DIRECTLY onto the target executor's queue (bypassing `schedule`, so
    // no background drainer is ever spawned to steal it). After `block_on`
    // returns: the cooperative branch will have driven that runnable
    // (`runnable_polls == 1`, queue empty); the parking branch leaves it untouched
    // (`runnable_polls == 0`, still queued).
    struct PendThenReady {
      polls: usize,
    }
    impl Future for PendThenReady {
      type Output = ();
      fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.polls += 1;
        if self.polls >= 2 {
          Poll::Ready(())
        } else {
          // Self-wake so the parking branch (`futures::executor::block_on`)
          // re-polls to completion without any external waker; the cooperative
          // branch re-polls unconditionally each loop iteration regardless.
          cx.waker().wake_by_ref();
          Poll::Pending
        }
      }
    }

    // Push one already-resolvable runnable directly onto `target`'s queue (no
    // `schedule`, so no drainer is spawned) and detach its task.
    fn enqueue_runnable(target: &Arc<MultiThreadExecutor>) {
      let scheduler = Arc::clone(target);
      let (runnable, task) = async_task::spawn(async {}, move |r| scheduler.schedule(r));
      target.queue.lock().unwrap().push_back(runnable);
      task.detach();
    }

    let opts = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 2,
      thread_name_prefix: "rd-onpool-scope".to_string(),
      park_deadline: None,
    };
    let exec1 =
      Arc::new(MultiThreadExecutor::new(&opts, Arc::new(RuntimeMetrics::default())).unwrap());
    let exec2 =
      Arc::new(MultiThreadExecutor::new(&opts, Arc::new(RuntimeMetrics::default())).unwrap());
    assert_ne!(exec1.id, exec2.id, "executors must get distinct ids");

    // Simulate being on exec1's pool worker for the whole scenario.
    let _on_pool = OnPoolWorkerGuard::enter(exec1.id);

    // FOREIGN call: on exec1's worker, drive exec2.block_on. The marker
    // (Some(exec1.id)) does NOT equal exec2.id, so exec2 must PARK and never
    // touch its queue.
    enqueue_runnable(&exec2);
    {
      let mut probe = std::pin::pin!(PendThenReady { polls: 0 });
      exec2.block_on(probe.as_mut());
    }
    assert_eq!(
      exec2.metrics.runnable_polls.load(Ordering::Relaxed),
      0,
      "a foreign-executor on-pool marker must NOT drive exec2's queue (must park)"
    );
    assert_eq!(
      exec2.queue.lock().unwrap().len(),
      1,
      "the foreign call must leave exec2's runnable untouched on its queue"
    );

    // CONTROL call: on exec1's worker, drive exec1.block_on. The marker equals
    // exec1.id, so exec1 takes the COOPERATIVE branch and drives its queued
    // runnable via `run_one`.
    enqueue_runnable(&exec1);
    {
      let mut probe = std::pin::pin!(PendThenReady { polls: 0 });
      exec1.block_on(probe.as_mut());
    }
    assert_eq!(
      exec1.metrics.runnable_polls.load(Ordering::Relaxed),
      1,
      "a same-executor on-pool marker MUST drive exec1's queue cooperatively"
    );
    assert!(
      exec1.queue.lock().unwrap().is_empty(),
      "the cooperative branch must have drained exec1's queued runnable"
    );

    // Tidy: drop the untouched exec2 runnable so its task does not leak a waker.
    exec2.queue.lock().unwrap().clear();
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_rd2_long_lived_blocking_consumer_does_not_starve_async_producers() {
    // Regression for RD-2 (the scan_stage native-magic-string sourcemap bug): a
    // long-lived `while let Ok(msg) = rx.recv()` consumer that DEPENDS on async
    // runnables to feed its channel must live on a dedicated OS thread, NOT the
    // runtime's blocking pool.
    //
    // This is a TWO-TOPOLOGY guard. With `worker_threads == 1` there is exactly one
    // pool worker, so `max_drainers == max_blocking == 1`:
    //
    //   Topology A (PRE-FIX, the hazard): the consumer recv-loop is routed through
    //   `schedule_blocking`. The sole drainer takes it via `take_blocking` and then
    //   blocks forever in `rx.recv()`. `active_drainers` is now at `max_drainers`,
    //   so `ensure_drainer` cannot spawn a replacement; the pool-scheduled async
    //   producers (and their terminate signal) never get polled, the channel is
    //   never fed, and `recv()` never returns => hard deadlock. We OBSERVE the
    //   consumer fail to complete within a short bound. This is NOT a timing race:
    //   the producers are genuinely unreachable, so no extra time lets A complete --
    //   the bound only sets how long we wait before declaring the deadlock real.
    //   Reverting scan_stage to the `spawn_blocking` shape is exactly what A models
    //   as broken.
    //
    //   Topology B (FIXED, mirrors the scan_stage fix): the SAME workload, but the
    //   consumer recv-loop runs on a DEDICATED `std::thread::spawn`. The sole drainer
    //   is then free to poll the producers, which feed the channel; the consumer sums
    //   them and terminates on `None`. Everything COMPLETES and the sum is correct.
    //
    // The test passes ONLY when A is observed stuck AND B completes.
    use std::sync::mpsc;
    use std::time::Duration;

    const PRODUCER_COUNT: usize = 8;

    // Schedule PRODUCER_COUNT `Some(i)` producers followed by one `None` terminator,
    // all as detached POOL runnables (`async_task::spawn` + `executor.schedule`) --
    // NOT driven from the caller thread. FIFO scheduling on a single worker keeps
    // every `Some` ahead of the `None`, so a consumer that actually drains the
    // channel observes the full sum before terminating. `detach` keeps each future
    // (and its `tx` clone) alive on the queue instead of cancelling it.
    fn schedule_pool_workload(
      executor: &Arc<MultiThreadExecutor>,
      tx: &mpsc::Sender<Option<usize>>,
      count: usize,
    ) {
      for i in 0..count {
        let tx = tx.clone();
        let scheduler = Arc::clone(executor);
        let (runnable, task) =
          async_task::spawn(async move { tx.send(Some(i)).unwrap() }, move |r| {
            scheduler.schedule(r);
          });
        executor.schedule(runnable);
        task.detach();
      }
      let tx = tx.clone();
      let scheduler = Arc::clone(executor);
      let (runnable, task) =
        async_task::spawn(async move { tx.send(None).unwrap() }, move |r| scheduler.schedule(r));
      executor.schedule(runnable);
      task.detach();
    }

    let expected_sum: usize = (0..PRODUCER_COUNT).sum();

    // ---- Topology A: PRE-FIX shape must DEADLOCK -------------------------------
    // Run on a child thread so the suite can never hang: the child performs the
    // bounded deadlock observation itself and reports the outcome over a channel.
    // The pool worker it leaves wedged in `rx.recv()` is deliberately abandoned
    // (see the `forget` below) -- we must never join a genuinely-deadlocked thread.
    let (a_report_tx, a_report_rx) = mpsc::channel::<bool>();
    std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "rd2-pre-fix".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let (tx, rx) = mpsc::channel::<Option<usize>>();
      let (started_tx, started_rx) = mpsc::channel::<()>();
      let (done_tx, done_rx) = mpsc::channel::<()>();

      // The long-lived consumer routed through the BLOCKING pool (the hazard). It
      // signals the moment it occupies the sole drainer, then loops on `recv()`.
      let handle = executor.schedule_blocking(move || {
        started_tx.send(()).unwrap();
        let mut sum = 0usize;
        while let Ok(msg) = rx.recv() {
          match msg {
            Some(value) => sum += value,
            None => break,
          }
        }
        done_tx.send(()).unwrap();
        sum
      });
      // Never await this handle (`recv()` would block forever); the consumer
      // reports liveness via the channels above. Dropping it just discards the
      // unused result receiver.
      drop(handle);

      // Wait until the consumer is CONFIRMED to occupy the sole drainer, THEN
      // schedule the producers. They can only run if a second drainer exists --
      // which it cannot (`active_drainers == max_drainers == 1`) -- so they starve.
      started_rx.recv().unwrap();
      schedule_pool_workload(&executor, &tx, PRODUCER_COUNT);
      // Keep the producer channel open forever so `rx.recv()` blocks (rather than
      // returning Err on disconnect) even after this harness thread exits -- the
      // modeled hazard is starvation, not channel closure.
      std::mem::forget(tx);

      // Bounded observation: under the hazard the consumer never receives `None`,
      // so this times out. Not a race -- the producers are unreachable.
      let completed = done_rx.recv_timeout(Duration::from_millis(1500)).is_ok();

      // Deliberately abandon the executor: its sole pool worker is wedged forever
      // in `rx.recv()`, so dropping the pool (which may wait on its workers) could
      // hang. Leak it instead of joining a deadlocked thread.
      std::mem::forget(executor);
      let _ = a_report_tx.send(completed);
    });

    let a_completed = a_report_rx
      .recv_timeout(Duration::from_secs(10))
      .expect("Topology A harness did not report (unexpected hang outside the modeled deadlock)");
    assert!(
      !a_completed,
      "Topology A (pre-fix `schedule_blocking` consumer) was expected to DEADLOCK: the recv-loop \
       occupies the sole drainer and starves the pool-scheduled async producers, so the consumer \
       never receives its terminate signal. It completed instead -- this test no longer models the \
       RD-2 hazard."
    );

    // ---- Topology B: FIXED shape must COMPLETE ---------------------------------
    let (b_done_tx, b_done_rx) = mpsc::channel::<usize>();
    let b_runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "rd2-fixed".to_string(),
        park_deadline: None,
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let (tx, rx) = mpsc::channel::<Option<usize>>();

      // The SAME consumer recv-loop, now on a DEDICATED OS thread (the scan_stage
      // fix). It does not occupy a drainer, so the sole drainer is free to feed it.
      let consumer = std::thread::spawn(move || {
        let mut sum = 0usize;
        while let Ok(msg) = rx.recv() {
          match msg {
            Some(value) => sum += value,
            None => break,
          }
        }
        sum
      });

      schedule_pool_workload(&executor, &tx, PRODUCER_COUNT);

      let sum = consumer.join().unwrap();
      b_done_tx.send(sum).unwrap();
    });

    match b_done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(sum) => {
        b_runner.join().unwrap();
        assert_eq!(
          sum, expected_sum,
          "Topology B (dedicated-thread consumer) produced the wrong sum"
        );
      }
      Err(error) => panic!(
        "Topology B (fixed shape) failed to complete within the bound ({error}): a dedicated-thread \
         consumer must let the sole drainer feed the async producers"
      ),
    }
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

  // ---- RD-8: validate() gatekeeping + configure() immutability --------------
  // These build `RuntimeOptions`/`RuntimeController` LOCALLY and never touch the
  // global `RUNTIME` singleton, so they stay deterministic and order-independent.

  #[test]
  fn validate_rejects_zero_worker_threads() {
    let error = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 0,
      max_blocking_tasks: 1,
      thread_name_prefix: "rd8".to_string(),
      park_deadline: None,
    }
    .validate()
    .expect_err("worker_threads == 0 must be rejected");
    assert_eq!(error.to_string(), "worker_threads must be greater than zero");
  }

  #[test]
  fn validate_rejects_zero_max_blocking_tasks() {
    let error = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 0,
      thread_name_prefix: "rd8".to_string(),
      park_deadline: None,
    }
    .validate()
    .expect_err("max_blocking_tasks == 0 must be rejected");
    assert_eq!(error.to_string(), "max_blocking_tasks must be greater than zero");
  }

  #[test]
  fn validate_current_thread_coerces_worker_threads_to_one() {
    // CurrentThread forces `worker_threads` to 1; `max_blocking_tasks` is then
    // clamped to `.min(worker_threads)` == 1.
    let validated = RuntimeOptions {
      flavor: RuntimeFlavor::CurrentThread,
      worker_threads: 8,
      max_blocking_tasks: 8,
      thread_name_prefix: "rd8".to_string(),
      park_deadline: None,
    }
    .validate()
    .expect("CurrentThread options must validate");
    assert_eq!(validated.flavor, RuntimeFlavor::CurrentThread);
    assert_eq!(validated.worker_threads, 1);
    assert_eq!(validated.max_blocking_tasks, 1);
  }

  // Native-only: `validate()` rejects `MultiThread` under `cfg(target_family = "wasm")`
  // (after applying the clamp), so this success assertion is valid off-wasm only.
  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn validate_clamps_max_blocking_tasks_to_worker_threads() {
    // MultiThread keeps `worker_threads`; `max_blocking_tasks` is clamped down to
    // it via `.min(worker_threads)` when it exceeds the worker count.
    let validated = RuntimeOptions {
      flavor: RuntimeFlavor::MultiThread,
      worker_threads: 2,
      max_blocking_tasks: 8,
      thread_name_prefix: "rd8".to_string(),
      park_deadline: None,
    }
    .validate()
    .expect("MultiThread options must validate");
    assert_eq!(validated.worker_threads, 2);
    assert_eq!(validated.max_blocking_tasks, 2);
  }

  #[test]
  fn configure_after_backend_started_is_rejected() {
    // A local controller, not the global `RUNTIME`. Use a CurrentThread backend
    // so starting it is cheap and spawns no OS threads.
    let controller = RuntimeController::new();
    controller
      .configure(RuntimeOptions {
        flavor: RuntimeFlavor::CurrentThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "rd8".to_string(),
        park_deadline: None,
      })
      .expect("first configure before the backend exists must succeed");

    // Materialize the backend; further configuration must now be refused.
    let _backend = controller.backend();

    let error = controller
      .configure(RuntimeOptions::default())
      .expect_err("configure after the backend started must be rejected");
    assert_eq!(
      error.to_string(),
      "the async runtime is already running; configure it before the first async call"
    );
  }

  // ---- RD-9: MultiThread blocking cap + RUNNABLE_BUDGET drain + panic->JoinError --
  // All three construct a `MultiThreadExecutor`/`RuntimeMetrics` LOCALLY (never the
  // global `RUNTIME`) and run the workload on a child thread guarded by a
  // `recv_timeout`, so a regression that hangs is reported as a failure instead of
  // wedging the whole suite.

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_blocking_active_never_exceeds_max_blocking() {
    // Regression guard for the `take_blocking` cap check: it refuses to pop/run a
    // queued blocking job while `active_blocking >= max_blocking`. The guard is
    // exercised deterministically (no sleeps, no incidental worker overlap):
    //
    //   1. EXACTLY `max_blocking_tasks` HELD holders saturate the cap. Each holder
    //      occupies a counted slot (`take_blocking` increments `active_blocking`
    //      BEFORE running the closure), signals `held`, then parks on a release
    //      barrier -- so all slots are provably occupied before we proceed.
    //   2. While the cap is saturated, we queue MORE blocking jobs ("extras").
    //      With the cap intact `take_blocking` returns `None` for them, so the
    //      idle workers cannot pop them: no extra may signal `extra_started`.
    //   3. We release the holders, then assert EVERY job (holders + extras)
    //      completes with the right result and the recorded peak never exceeded
    //      the cap.
    //
    // Decisive property: if the `active >= self.max_blocking` guard were removed,
    // the two idle workers would pop the extras while both holders are still
    // parked, driving the recorded peak (`max_active_blocking_tasks`, a monotonic
    // fetch_max set at job entry) above the cap -- so this test fails on EVERY run.
    // The release barrier is sized `holder_count + 1` (<= max_blocking + 1), so it
    // can never self-deadlock on the over-cap extras.
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
        thread_name_prefix: "rd9-blk-cap".to_string(),
        park_deadline: None,
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, Arc::clone(&metrics)).unwrap());

      let holder_count = options.max_blocking_tasks; // saturates the cap exactly
      let extra_count = 4usize; // beyond-cap jobs that must NOT run while held

      // Holders: signal once they hold a counted slot, then park until released.
      let (held_tx, held_rx) = mpsc::channel();
      let release = Arc::new(Barrier::new(holder_count + 1));
      let mut holder_handles = Vec::new();
      for _ in 0..holder_count {
        let held_tx = held_tx.clone();
        let release = Arc::clone(&release);
        holder_handles.push(executor.schedule_blocking(move || {
          held_tx.send(()).unwrap();
          release.wait();
          0usize
        }));
      }
      // Wait until every slot is provably occupied (cap fully saturated).
      for _ in 0..holder_count {
        held_rx.recv().unwrap();
      }

      // Queue the beyond-cap extras. Each signals the instant it begins running.
      let (extra_started_tx, extra_started_rx) = mpsc::channel();
      let mut extra_handles = Vec::new();
      for i in 0..extra_count {
        let extra_started_tx = extra_started_tx.clone();
        extra_handles.push(executor.schedule_blocking(move || {
          extra_started_tx.send(i).unwrap();
          i * 2
        }));
      }

      // With the cap intact, `take_blocking` returns `None` while both slots are
      // held, so no extra can have started running yet.
      assert!(
        matches!(extra_started_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "a beyond-cap blocking job ran before the cap was released (cap check broken)"
      );

      // Release the holders so the queued extras can run within the cap.
      release.wait();

      for handle in holder_handles {
        assert_eq!(futures::executor::block_on(handle).unwrap(), 0usize);
      }
      // Each handle owns its own oneshot receiver, so awaiting in order returns
      // each extra's own result regardless of the order the pool ran them.
      for (i, handle) in extra_handles.into_iter().enumerate() {
        assert_eq!(futures::executor::block_on(handle).unwrap(), i * 2);
      }

      // `blocking_tasks_started` is incremented at job entry, so once every result
      // is in, all holder + extra starts are recorded.
      assert_eq!(
        metrics.blocking_tasks_started.load(Ordering::Relaxed),
        (holder_count + extra_count) as u64
      );

      *peak_slot.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
        Some(metrics.max_active_blocking_tasks.load(Ordering::Relaxed));
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("RD-9 blocking-cap test did not complete ({error})"),
    }
    let peak =
      peak_out.lock().unwrap_or_else(std::sync::PoisonError::into_inner).expect("peak captured");
    assert!(peak >= 1, "at least one blocking job must have run (peak {peak})");
    assert!(peak <= 2, "peak active blocking tasks {peak} exceeded max_blocking_tasks 2");
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_drain_budget_completes_large_runnable_batch() {
    // `drain` processes at most `RUNNABLE_BUDGET` (64) runnables per pass, then
    // yields and re-drains via `finish_draining` -> `ensure_drainer`. A batch of
    // MORE than 64 runnables must therefore exercise the budget yield/re-drain and
    // still complete every task, with the runnable counters settling back to 0.
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd9-budget".to_string(),
        park_deadline: None,
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, Arc::clone(&metrics)).unwrap());

      let task_count = 200usize; // > RUNNABLE_BUDGET (64): forces budget re-drain
      let completed = Arc::new(AtomicU64::new(0));
      let mut tasks = Vec::new();
      for _ in 0..task_count {
        let completed = Arc::clone(&completed);
        let scheduler = Arc::clone(&executor);
        let (runnable, task) = async_task::spawn(
          async move {
            completed.fetch_add(1, Ordering::Relaxed);
          },
          move |r| scheduler.schedule(r),
        );
        executor.schedule(runnable);
        tasks.push(task);
      }
      for task in tasks {
        futures::executor::block_on(task);
      }

      // Every immediately-ready future is polled exactly once; `runnable_polls`
      // (incremented at poll entry, before completion) is therefore exact here.
      assert_eq!(completed.load(Ordering::Relaxed), task_count as u64);
      assert_eq!(metrics.runnable_polls.load(Ordering::Relaxed), task_count as u64);

      // The active-runnable guard for the LAST task drops just after its Task
      // resolves (i.e. just after `block_on` returns), so give the counters a
      // brief, bounded moment to settle to 0 rather than asserting them racily.
      let deadline = Instant::now() + Duration::from_secs(5);
      loop {
        let active = metrics.active_runnables.load(Ordering::Relaxed);
        let queued = metrics.queued_runnables.load(Ordering::Relaxed);
        if active == 0 && queued == 0 {
          break;
        }
        assert!(
          Instant::now() < deadline,
          "runnable counters did not settle to 0: active={active} queued={queued}"
        );
        std::thread::yield_now();
      }
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(15)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("RD-9 drain-budget test did not complete ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_spawned_panic_surfaces_as_join_error() {
    // A panicking spawned future must surface as `Err(JoinError)` carrying the
    // panic payload's message. We mirror the production `spawn` wrapper LOCALLY
    // (catch_unwind the future, map the panic via `JoinError::from_panic`) and
    // drive the resulting `JoinHandle` on this executor -- without touching the
    // global `RUNTIME`. `JoinError::from_panic` downcasts a `&str` payload, so the
    // `panic!("x")` message round-trips as `"x"`.
    use std::sync::mpsc;
    use std::time::Duration;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "rd9-panic".to_string(),
        park_deadline: None,
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, metrics).unwrap());

      let scheduler = Arc::clone(&executor);
      let wrapped = async {
        AssertUnwindSafe(async { panic!("x") })
          .catch_unwind()
          .await
          .map_err(|panic| JoinError::from_panic(&*panic))
      };
      let (runnable, task) = async_task::spawn(wrapped, move |r| scheduler.schedule(r));
      executor.schedule(runnable);

      let result: Result<(), JoinError> =
        futures::executor::block_on(JoinHandle(JoinHandleInner::Task(task)));
      let _ = done_tx.send(result);
    });

    let result = match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(result) => {
        runner.join().unwrap();
        result
      }
      Err(error) => panic!("RD-9 panic->JoinError test did not complete ({error})"),
    };
    let error = result.expect_err("a panicking spawned future must surface as Err(JoinError)");
    assert_eq!(error.to_string(), "x");
  }

  #[test]
  fn runtime_metrics_reset_zeroes_every_counter() {
    // RD-11: `reset()` must return EVERY counter to zero. Operates on a LOCALLY
    // constructed instance only -- it never touches the process-global `RUNTIME`
    // / `metrics()` snapshot path (that singleton is shared across parallel tests
    // and would make this flaky). Each field is asserted individually so that a
    // future-added counter which `reset()` forgets to clear fails here loudly.
    //
    // NOTE: adding a counter to `RuntimeMetrics` requires updating BOTH the
    // drive-non-zero loop and the per-field zero assertions below.
    let metrics = RuntimeMetrics::default();

    // Drive every counter to a distinct non-zero value so reset() has something
    // to clear in each field (distinct values also guard against a field being
    // cleared by clobbering a neighbour).
    metrics.tasks_spawned.store(1, Ordering::Relaxed);
    metrics.tasks_completed.store(2, Ordering::Relaxed);
    metrics.tasks_panicked.store(3, Ordering::Relaxed);
    metrics.runnable_schedules.store(4, Ordering::Relaxed);
    metrics.runnable_polls.store(5, Ordering::Relaxed);
    metrics.queued_runnables.store(6, Ordering::Relaxed);
    metrics.max_queued_runnables.store(7, Ordering::Relaxed);
    metrics.active_runnables.store(8, Ordering::Relaxed);
    metrics.max_active_runnables.store(9, Ordering::Relaxed);
    metrics.blocking_tasks_scheduled.store(14, Ordering::Relaxed);
    metrics.blocking_tasks_started.store(10, Ordering::Relaxed);
    metrics.blocking_tasks_completed.store(11, Ordering::Relaxed);
    metrics.active_blocking_tasks.store(12, Ordering::Relaxed);
    metrics.max_active_blocking_tasks.store(13, Ordering::Relaxed);

    // Sanity: confirm the pre-reset state is genuinely non-zero everywhere, so a
    // green assertion below cannot be a vacuous "was already zero".
    assert_ne!(metrics.tasks_spawned.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.tasks_completed.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.tasks_panicked.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.runnable_schedules.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.runnable_polls.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.queued_runnables.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.max_queued_runnables.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.active_runnables.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.max_active_runnables.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.blocking_tasks_scheduled.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.blocking_tasks_started.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.blocking_tasks_completed.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.active_blocking_tasks.load(Ordering::Relaxed), 0);
    assert_ne!(metrics.max_active_blocking_tasks.load(Ordering::Relaxed), 0);

    metrics.reset();

    assert_eq!(metrics.tasks_spawned.load(Ordering::Relaxed), 0, "tasks_spawned");
    assert_eq!(metrics.tasks_completed.load(Ordering::Relaxed), 0, "tasks_completed");
    assert_eq!(metrics.tasks_panicked.load(Ordering::Relaxed), 0, "tasks_panicked");
    assert_eq!(metrics.runnable_schedules.load(Ordering::Relaxed), 0, "runnable_schedules");
    assert_eq!(metrics.runnable_polls.load(Ordering::Relaxed), 0, "runnable_polls");
    assert_eq!(metrics.queued_runnables.load(Ordering::Relaxed), 0, "queued_runnables");
    assert_eq!(metrics.max_queued_runnables.load(Ordering::Relaxed), 0, "max_queued_runnables");
    assert_eq!(metrics.active_runnables.load(Ordering::Relaxed), 0, "active_runnables");
    assert_eq!(metrics.max_active_runnables.load(Ordering::Relaxed), 0, "max_active_runnables");
    assert_eq!(
      metrics.blocking_tasks_scheduled.load(Ordering::Relaxed),
      0,
      "blocking_tasks_scheduled"
    );
    assert_eq!(metrics.blocking_tasks_started.load(Ordering::Relaxed), 0, "blocking_tasks_started");
    assert_eq!(
      metrics.blocking_tasks_completed.load(Ordering::Relaxed),
      0,
      "blocking_tasks_completed"
    );
    assert_eq!(metrics.active_blocking_tasks.load(Ordering::Relaxed), 0, "active_blocking_tasks");
    assert_eq!(
      metrics.max_active_blocking_tasks.load(Ordering::Relaxed),
      0,
      "max_active_blocking_tasks"
    );
  }

  // ---- Wake-path §6(d): self-detecting block_on deadlocks -------------------
  // The five shapes pinned by the task-4 brief: (1) a threadless CurrentThread
  // park decision with no pending wake panics with the typed diagnostic; (2) a
  // self-waking future (the PendThenReady shape) must NOT panic -- the panic
  // lives at the PARK DECISION, not at Pending-return (intel §8.6); (3) an
  // armed deadline fires on a genuinely dead cooperative MT park; (4) runtime
  // progress RESETS the deadline instead of firing (intel §8.5); (5) the
  // foreign/napi whole-build park is excluded from the deadline entirely.

  /// A future that never completes and never wakes anyone: once its driver
  /// parks, no wake can ever arrive. The deadlock-detection probe shape.
  struct NeverReady;
  impl Future for NeverReady {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
      Poll::Pending
    }
  }

  #[test]
  fn current_thread_threadless_park_with_no_pending_wake_panics_with_typed_diagnostic() {
    // Shape (1): on a threadless build a CT park decision with an empty queue
    // and no wake token pending is a PROVABLE deadlock (no other thread exists
    // to deliver the wake) -- `block_on` must panic immediately with the typed
    // `BlockOnDeadlock` diagnostic instead of parking forever. The workload
    // runs on a child thread bounded by recv_timeout so a missing detection
    // (a hang) fails the test rather than wedging the suite.
    use std::sync::mpsc;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      // Native stand-in for the threadless wasm build (intel §7(d)); no
      // deadline -- the certain case is not timing-based.
      let executor = Arc::new(CurrentThreadExecutor::with_detection(metrics, true, None));

      let payload = catch_unwind(AssertUnwindSafe(|| {
        let mut future = std::pin::pin!(NeverReady);
        executor.block_on(future.as_mut());
      }))
      .expect_err("a threadless park with an empty queue and no pending wake must panic");
      let diagnostic = payload
        .downcast_ref::<BlockOnDeadlock>()
        .expect("the panic payload must be the typed BlockOnDeadlock diagnostic");
      assert_eq!(diagnostic.kind, BlockOnDeadlockKind::CurrentThreadCertain);
      assert_eq!(diagnostic.park_deadline, None, "the certain case carries no deadline");
      let message = diagnostic.to_string();
      assert!(
        message.contains("block_on") && message.contains("JS"),
        "the diagnostic must name the block_on-awaiting-JS hazard, got: {message}"
      );
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "threadless CurrentThread park did not self-detect ({error}): block_on parked (or failed) instead of panicking with the diagnostic"
      ),
    }
  }

  #[test]
  fn current_thread_threadless_self_waking_pending_future_does_not_panic() {
    // Shape (2), pinning panic-at-PARK-DECISION (intel §8.6): a future that
    // returns `Pending` but wakes itself synchronously during the poll leaves
    // a wake token pending, so the loop must consume the token and re-poll --
    // NOT panic -- even on a threadless build. "Pending returned" alone is
    // never a deadlock.
    use std::sync::mpsc;

    struct PendThenReady {
      polls: usize,
    }
    impl Future for PendThenReady {
      type Output = ();
      fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.polls += 1;
        if self.polls >= 2 {
          Poll::Ready(())
        } else {
          cx.waker().wake_by_ref();
          Poll::Pending
        }
      }
    }

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let executor = Arc::new(CurrentThreadExecutor::with_detection(metrics, true, None));
      let mut future = std::pin::pin!(PendThenReady { polls: 0 });
      executor.block_on(future.as_mut());
      assert_eq!(future.polls, 2, "the self-woken future must have been re-polled to completion");
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "a self-waking Pending future must complete without panicking on a threadless build ({error})"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn current_thread_native_park_deadline_fires_with_typed_diagnostic() {
    // Threaded CT is NOT provably dead at the park decision (another OS thread
    // could deliver a wake later), so detection there is deadline-based and
    // opt-in. A park that outlives the armed deadline with ZERO runtime
    // progress must panic with the typed diagnostic.
    use std::sync::mpsc;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let metrics = Arc::new(RuntimeMetrics::default());
      let executor = Arc::new(CurrentThreadExecutor::with_detection(
        metrics,
        false,
        Some(Duration::from_millis(50)),
      ));

      let payload = catch_unwind(AssertUnwindSafe(|| {
        let mut future = std::pin::pin!(NeverReady);
        executor.block_on(future.as_mut());
      }))
      .expect_err("a dead native CT park must fire the armed deadline");
      let diagnostic = payload.downcast_ref::<BlockOnDeadlock>().expect("typed payload");
      assert_eq!(diagnostic.kind, BlockOnDeadlockKind::CurrentThreadDeadline);
      assert_eq!(diagnostic.park_deadline, Some(Duration::from_millis(50)));
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("native CurrentThread park deadline did not fire ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn current_thread_native_park_deadline_resets_on_runtime_progress() {
    // PROGRESS-BASED RESET (intel §8.5), CT side: metrics advancing while
    // parked means the runtime is alive (e.g. a slow JS plugin with other
    // work proceeding), so the deadline must re-arm instead of firing.
    // Progress is injected by advancing the very counter the reset reads --
    // the exact seam, isolated from wake delivery (production advances it via
    // real runnable polls).
    use std::sync::mpsc;

    let deadline = Duration::from_millis(60);
    let metrics = Arc::new(RuntimeMetrics::default());
    let executor =
      Arc::new(CurrentThreadExecutor::with_detection(Arc::clone(&metrics), false, Some(deadline)));

    let (value_tx, value_rx) = oneshot::channel::<usize>();
    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let mut output = None;
      {
        let mut future = std::pin::pin!(async {
          output = Some(value_rx.await.unwrap());
        });
        executor.block_on(future.as_mut());
      }
      assert_eq!(output, Some(9));
      let _ = done_tx.send(());
    });

    // Hold the park across several full deadline windows, each containing an
    // injected progress tick, then deliver the (legitimately late) wake.
    for _ in 0..8 {
      std::thread::sleep(Duration::from_millis(30));
      metrics.runnable_polls.fetch_add(1, Ordering::Relaxed);
    }
    value_tx.send(9).unwrap();

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "a CT park with ongoing runtime progress fired its deadline or lost its wake ({error})"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_cooperative_park_past_deadline_with_no_progress_panics_with_typed_diagnostic() {
    // Shape (3): the MT deadline lives on the Task-3 DriverParker park inside
    // `cooperative_block_on`. Drive the cooperative branch directly on this
    // thread via the fake on-pool marker (the 1793 technique) so the panic is
    // observable as a typed payload without rayon in between.
    use std::sync::mpsc;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "deadline-fire".to_string(),
        park_deadline: Some(Duration::from_millis(50)),
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let payload = {
        let _on_pool = OnPoolWorkerGuard::enter(executor.id);
        catch_unwind(AssertUnwindSafe(|| {
          let mut future = std::pin::pin!(NeverReady);
          executor.block_on(future.as_mut());
        }))
        .expect_err("a dead cooperative park must fire the armed deadline")
      };
      let diagnostic = payload.downcast_ref::<BlockOnDeadlock>().expect("typed payload");
      assert_eq!(diagnostic.kind, BlockOnDeadlockKind::MultiThreadCooperativeDeadline);
      assert_eq!(diagnostic.park_deadline, Some(Duration::from_millis(50)));
      let message = diagnostic.to_string();
      assert!(message.contains("block_on"), "diagnostic must name the hazard, got: {message}");
      assert_eq!(
        executor.parked_drivers.count.load(Ordering::SeqCst),
        0,
        "the firing driver must deregister itself before panicking"
      );
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!("MT cooperative park deadline did not fire ({error})"),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_cooperative_park_deadline_resets_on_executor_progress() {
    // Shape (4), MT side of the progress reset (intel §8.5): a legitimately
    // slow wake under an armed deadline must NOT fire while the executor's
    // metrics keep advancing. Topology: the sole worker is parked as a
    // cooperative driver (real pool thread, real DriverParker park); the test
    // injects progress ticks across several full deadline windows, then
    // delivers the late wake.
    use std::sync::mpsc;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let deadline = Duration::from_millis(60);
      let metrics = Arc::new(RuntimeMetrics::default());
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "deadline-reset".to_string(),
        park_deadline: Some(deadline),
      };
      let executor = Arc::new(MultiThreadExecutor::new(&options, Arc::clone(&metrics)).unwrap());

      // Park the sole worker as a cooperative driver awaiting a gate (same
      // topology as the miss-compensation test).
      let (gate_tx, gate_rx) = oneshot::channel::<usize>();
      let td_exec = Arc::clone(&executor);
      let completed = Arc::new(AtomicBool::new(false));
      let td_flag = Arc::clone(&completed);
      let td_body = async move {
        let mut inner = std::pin::pin!(async move {
          assert_eq!(gate_rx.await.unwrap(), 9);
        });
        td_exec.block_on(inner.as_mut());
        td_flag.store(true, Ordering::SeqCst);
      };
      let sched = Arc::clone(&executor);
      let (td_runnable, td_task) = async_task::spawn(td_body, move |r| sched.schedule(r));
      executor.schedule(td_runnable);
      wait_until("the sole driver parked", || {
        executor.parked_drivers.count.load(Ordering::SeqCst) == 1
      });

      // Outlive the armed deadline several times over, with every window
      // containing an injected progress tick (the counter the reset reads).
      for _ in 0..8 {
        std::thread::sleep(Duration::from_millis(30));
        metrics.runnable_polls.fetch_add(1, Ordering::Relaxed);
      }
      assert!(
        !completed.load(Ordering::SeqCst),
        "the driver must still be parked awaiting its late wake"
      );
      gate_tx.send(9).unwrap();
      futures::executor::block_on(td_task);
      assert!(completed.load(Ordering::SeqCst), "the late wake must complete the parked driver");
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(15)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "an MT cooperative park with ongoing executor progress fired its deadline or lost its wake ({error})"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_foreign_block_on_long_park_is_excluded_from_deadline() {
    // Shape (5), intel §8.5 (binding): the foreign/napi caller thread parks in
    // `futures::executor::block_on` for the ENTIRE build -- the normal
    // production shape. Even with an aggressively short deadline armed and
    // ZERO executor progress for many windows, that park must never fire.
    use std::sync::mpsc;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 2,
        max_blocking_tasks: 2,
        thread_name_prefix: "foreign-exempt".to_string(),
        park_deadline: Some(Duration::from_millis(40)),
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      let (value_tx, value_rx) = oneshot::channel::<usize>();
      let waker_thread = std::thread::spawn(move || {
        // Well past several deadline windows, with zero executor progress.
        std::thread::sleep(Duration::from_millis(300));
        let _ = value_tx.send(7);
      });

      let started = Instant::now();
      let mut output = None;
      {
        let mut future = std::pin::pin!(async {
          output = Some(value_rx.await.unwrap());
        });
        // This (non-pool) thread takes the parking branch: exempt by design.
        executor.block_on(future.as_mut());
      }
      assert_eq!(output, Some(7));
      assert!(
        started.elapsed() >= Duration::from_millis(250),
        "the park must genuinely have outlived the 40ms deadline many times over"
      );
      waker_thread.join().unwrap();
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => {
        panic!("the exempt foreign-thread whole-build park fired or lost its wake ({error})")
      }
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_wake_delivered_at_deadline_expiry_edge_is_not_reported_as_deadlock() {
    // Codex round-1 (confirmed race), pinned DETERMINISTICALLY via the
    // expiry test hook instead of a wall-clock lottery. Interleaving:
    //   (1) the cooperative park's deadline expires with genuinely zero
    //       progress (nothing else touches the executor before the park);
    //   (2) BEFORE the driver's expiry decision runs -- parker STILL
    //       REGISTERED -- a scheduler queues a blocking job; its `wake_one`
    //       pops this parker, stores the permit and reports the wake as
    //       DELIVERED (so no drainer is spawned). The wake was counted as
    //       delivered TO US, so the post-deregister permit re-check is the
    //       first-line catch this test pins (queued-work and, since the
    //       round-2 fix, the enqueue counter in the fingerprint are the
    //       further backstops);
    //   (3) the expiry decision runs. Pre-fix it deregistered and panicked
    //       `BlockOnDeadlock`, swallowing the delivered wake and killing a
    //       healthy build; it must instead treat the park as woken, run the
    //       job (which sends the gate) and complete the awaited future.
    use std::sync::mpsc;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "edge-wake".to_string(),
        park_deadline: Some(Duration::from_millis(40)),
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());

      // Until the racing job sends this gate, the awaited future is Pending.
      let (gate_tx, gate_rx) = oneshot::channel::<usize>();
      // (registered_before_wake, registered_after_wake) observed by the hook.
      let (hook_saw_tx, hook_saw_rx) = mpsc::channel::<(usize, usize)>();

      let hook_exec = Arc::clone(&executor);
      DEADLINE_EXPIRY_TEST_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
          let before = hook_exec.parked_drivers.count.load(Ordering::SeqCst);
          // The racing schedule: pushes the job, then `wake_one` pops the
          // still-registered parker (permit stored, wake counted as
          // delivered, no drainer).
          let handle = hook_exec.schedule_blocking(move || {
            let _ = gate_tx.send(5usize);
            0usize
          });
          drop(handle); // result unobserved; the gate send is the signal
          let after = hook_exec.parked_drivers.count.load(Ordering::SeqCst);
          hook_saw_tx.send((before, after)).unwrap();
        }));
      });

      // Fake on-pool marker (1793 technique): the cooperative branch -- and
      // therefore the thread-local hook -- runs on THIS thread.
      let _on_pool = OnPoolWorkerGuard::enter(executor.id);
      let mut output = None;
      {
        let mut future = std::pin::pin!(async {
          output = Some(gate_rx.await.unwrap());
        });
        executor.block_on(future.as_mut());
      }
      assert_eq!(
        output,
        Some(5),
        "the edge-delivered wake's job must have run and completed the future"
      );

      let (before, after) = hook_saw_rx.recv().expect("the expiry hook must have fired");
      assert_eq!(before, 1, "the hook must observe the parker STILL registered (race window)");
      assert_eq!(after, 0, "the racing wake_one must have popped the parker (wake DELIVERED)");
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "a wake delivered at the deadline-expiry edge was reported as a deadlock or lost ({error})"
      ),
    }
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn multi_thread_blocking_enqueue_between_queue_recheck_and_verdict_is_not_reported_as_deadlock() {
    // Codex round-2 (confirmed asymmetry): `runnable_schedules` is bumped at
    // ENQUEUE, so a runnable scheduled after the expiry decision's
    // `has_queued_work()` re-check is still caught by the final fingerprint
    // read -- but blocking work only advanced counters when a job STARTED, so
    // a `schedule_blocking` landing in that same window was completely
    // invisible: queue push done (deliverable work exists), wake gone out to
    // an EMPTY registry (we are deregistered; `ensure_drainer` is the only
    // fallback), and the pre-fix verdict panicked `BlockOnDeadlock` anyway.
    // Deterministic pinning via DEADLINE_VERDICT_TEST_HOOK (fires between the
    // queued-work re-check and the fingerprint verdict, on the driver thread
    // itself, so the enqueue is sequenced-before the verdict read):
    //   * `active_drainers` is saturated so `ensure_drainer` cannot start the
    //     job on the real worker -- `blocking_tasks_started` provably cannot
    //     advance behind our back, and pre-fix the verdict is DETERMINISTICALLY
    //     blind (panics every run); post-fix only the new enqueue counter
    //     (`blocking_tasks_scheduled`) can and must save the park.
    use std::sync::mpsc;

    let (done_tx, done_rx) = mpsc::channel();
    let runner = std::thread::spawn(move || {
      let options = RuntimeOptions {
        flavor: RuntimeFlavor::MultiThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        thread_name_prefix: "verdict-blocking".to_string(),
        park_deadline: Some(Duration::from_millis(40)),
      };
      let executor =
        Arc::new(MultiThreadExecutor::new(&options, Arc::new(RuntimeMetrics::default())).unwrap());
      executor.active_drainers.store(executor.max_drainers, Ordering::SeqCst);

      // Until the racing job sends this gate, the awaited future is Pending.
      let (gate_tx, gate_rx) = oneshot::channel::<usize>();
      // Registry size observed by the hook (must be 0: already deregistered,
      // so no permit can possibly be delivered for this enqueue).
      let (hook_saw_tx, hook_saw_rx) = mpsc::channel::<usize>();

      let hook_exec = Arc::clone(&executor);
      DEADLINE_VERDICT_TEST_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
          let handle = hook_exec.schedule_blocking(move || {
            let _ = gate_tx.send(11usize);
            0usize
          });
          drop(handle); // result unobserved; the gate send is the signal
          hook_saw_tx.send(hook_exec.parked_drivers.count.load(Ordering::SeqCst)).unwrap();
        }));
      });

      // Fake on-pool marker (1793 technique): the cooperative branch -- and
      // therefore the thread-local hook -- runs on THIS thread.
      let _on_pool = OnPoolWorkerGuard::enter(executor.id);
      let mut output = None;
      {
        let mut future = std::pin::pin!(async {
          output = Some(gate_rx.await.unwrap());
        });
        executor.block_on(future.as_mut());
      }
      assert_eq!(
        output,
        Some(11),
        "the enqueued job must have been run by the surviving driver via run_one"
      );

      let registered_at_hook = hook_saw_rx.recv().expect("the verdict hook must have fired");
      assert_eq!(
        registered_at_hook, 0,
        "the hook fires post-deregister: no permit can be delivered, only the fingerprint's          enqueue counter can catch this submission"
      );
      executor.active_drainers.store(0, Ordering::SeqCst);
      let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(Duration::from_secs(10)) {
      Ok(()) => runner.join().unwrap(),
      Err(error) => panic!(
        "a blocking enqueue landing between the queue re-check and the verdict was reported as a deadlock or lost ({error})"
      ),
    }
  }

  #[test]
  fn park_deadline_env_parsing_treats_unset_zero_and_garbage_as_disabled() {
    // The env knob mirrors `resolve_thread_count`'s treatment of unusable
    // values: never panic module init over a typo, just disable detection.
    assert_eq!(parse_park_deadline(None), None);
    assert_eq!(parse_park_deadline(Some("0".to_string())), None);
    assert_eq!(parse_park_deadline(Some("abc".to_string())), None);
    assert_eq!(parse_park_deadline(Some("-5".to_string())), None);
    assert_eq!(parse_park_deadline(Some(String::new())), None);
    assert_eq!(parse_park_deadline(Some("1500".to_string())), Some(Duration::from_millis(1500)));
  }

  #[test]
  fn block_on_deadlock_panic_payload_surfaces_through_join_error() {
    // A deadline firing inside a spawned task's poll unwinds into the spawn
    // wrapper's catch_unwind and becomes a JoinError; the typed diagnostic's
    // message must survive that trip (JoinError::from_panic downcasts it), or
    // the build error would read "async runtime task panicked" with no clue.
    let diagnostic = BlockOnDeadlock::multi_thread_cooperative(Duration::from_millis(75));
    let expected = diagnostic.to_string();
    let payload: Box<dyn Any + Send> = Box::new(diagnostic);
    assert_eq!(JoinError::from_panic(&*payload).to_string(), expected);
  }
}
