//! Returns memory that mimalloc keeps for idle worker threads to the OS.
//! See internal-docs/memory-reclaim/implementation.md

#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
))]
mod imp {
  use std::{
    cell::Cell,
    sync::{
      Arc, Condvar, Mutex, Once, OnceLock, PoisonError,
      atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
  };

  use std::ffi::{c_int, c_long, c_void};

  use napi::{
    Env, Status,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    tokio::runtime::Handle,
  };

  const DEFAULT_DELAY: Duration = Duration::from_millis(500);
  /// A thread that stays busy past this is skipped and retried later.
  const THREAD_DEADLINE: Duration = Duration::from_millis(100);
  /// The release between builds may overlap the next build, so it waits less for a busy thread.
  const RELEASE_DEADLINE: Duration = Duration::from_millis(10);
  /// A round that misses a thread runs again after 1, 2, 4 and 8 s.
  const RETRIES: u32 = 4;

  unsafe extern "C" {
    fn mi_theap_get_default() -> *mut c_void;
    fn mi_theap_quiescent_abandon(theap: *mut c_void);
    fn mi_arenas_purge();
    fn mi_version() -> c_int;
    fn mi_option_set(option: c_int, value: c_long);
  }

  struct Reclaimer {
    events: Mutex<u64>,
    changed: Condvar,
    delay: Duration,
    release_between_builds: bool,
    tokio: Handle,
    max_blocking_threads: usize,
  }

  static DELAY: OnceLock<Duration> = OnceLock::new();
  static RECLAIMER: OnceLock<Reclaimer> = OnceLock::new();
  static NEXT_ROUND: AtomicU64 = AtomicU64::new(1);

  type JsThreadCallback = ThreadsafeFunction<(), (), (), Status, false, true>;
  /// One per JS thread: the main thread and each worker thread that loaded rolldown.
  static JS_THREADS: Mutex<Vec<JsThreadCallback>> = Mutex::new(Vec::new());

  thread_local! {
    static RECLAIMED_ROUND: Cell<u64> = const { Cell::new(0) };
    static JS_THREAD_REGISTERED: Cell<bool> = const { Cell::new(false) };
  }

  /// A JS thread holds build data too, such as plugin results, so each round posts to it. The
  /// function is weak, so it never keeps Node alive.
  pub fn register_js_thread(env: &Env) {
    if RECLAIMER.get().is_none() || JS_THREAD_REGISTERED.get() {
      return;
    }
    JS_THREAD_REGISTERED.set(true);
    let callback = env
      .create_function::<(), ()>("rolldownMemoryReclaim", collect_on_js_thread)
      .and_then(|function| {
        function.build_threadsafe_function().callee_handled::<false>().weak::<true>().build()
      });
    if let Ok(callback) = callback {
      JS_THREADS.lock().unwrap_or_else(PoisonError::into_inner).push(callback);
    }
  }

  unsafe extern "C" fn collect_on_js_thread(
    _env: napi::sys::napi_env,
    _info: napi::sys::napi_callback_info,
  ) -> napi::sys::napi_value {
    release_thread_pages();
    // This runs after the round's purge.
    purge();
    std::ptr::null_mut()
  }

  fn reclaim_js_threads() {
    // A thread whose environment is gone answers `Closing`, so it leaves the list.
    JS_THREADS
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .retain(|callback| callback.call((), ThreadsafeFunctionCallMode::NonBlocking) == Status::Ok);
  }

  /// Runs at load, before the async runtime starts any thread. Returns whether reclaim is on.
  pub fn configure() -> bool {
    // `mi_theap_quiescent_abandon` is new in mimalloc 3.6.
    // SAFETY: plain call with no arguments.
    if unsafe { mi_version() } < 30600 {
      return false;
    }
    let delay = match std::env::var("ROLLDOWN_MEMORY_RECLAIM_DELAY").as_deref() {
      Ok("off") => return false,
      Ok(value) => value.parse().map_or(DEFAULT_DELAY, Duration::from_millis),
      Err(_) => DEFAULT_DELAY,
    };
    let _ = DELAY.set(delay);
    set_allocator_options();
    true
  }

  pub fn init(tokio: Handle, max_blocking_threads: usize) {
    let Some(&delay) = DELAY.get() else {
      return;
    };
    let _ = RECLAIMER.set(Reclaimer {
      events: Mutex::new(0),
      changed: Condvar::new(),
      delay,
      release_between_builds: std::env::var_os("ROLLDOWN_MEMORY_RECLAIM_BETWEEN_BUILDS")
        .is_some_and(|value| value != "0" && !value.is_empty()),
      tokio,
      max_blocking_threads,
    });
    rolldown::set_memory_released_hook(notify);
  }

  /// Options are plain globals, so they are set once at load, before the async runtime starts any
  /// thread, and never change afterwards. An option the user set through the environment wins.
  fn set_allocator_options() {
    /// Option numbers in mimalloc 3.6. `libmimalloc-sys2` does not export them, so the version
    /// check keeps a mimalloc bump from setting other options.
    const PAGE_COMMIT_ON_DEMAND: c_int = 40;
    #[cfg(target_os = "linux")]
    const ARENA_PURGE_MIN_SIZE: c_int = 44;
    // SAFETY: plain call with no arguments.
    if unsafe { mi_version() } / 100 != 306 {
      return;
    }
    // Without overcommit, as on Windows, the OS charges commit for every page a thread claims,
    // even untouched ones. 2 commits pages on demand only on such systems.
    set_unless_user_defined(PAGE_COMMIT_ON_DEMAND, 2, &["page_commit_on_demand"]);
    // With transparent huge pages, mimalloc purges in 2 MiB units, which skips most ranges.
    #[cfg(target_os = "linux")]
    set_unless_user_defined(
      ARENA_PURGE_MIN_SIZE,
      64,
      &["arena_purge_min_size", "minimal_purge_size"],
    );
  }

  fn set_unless_user_defined(option: c_int, value: c_long, names: &[&str]) {
    let is_user_defined = names.iter().any(|name| {
      std::env::var_os(format!("MIMALLOC_{}", name.to_uppercase())).is_some()
        || std::env::var_os(format!("mimalloc_{name}")).is_some()
    });
    if !is_user_defined {
      // SAFETY: nothing reads the options concurrently yet, see `set_allocator_options`.
      unsafe { mi_option_set(option, value) };
    }
  }

  pub fn notify() {
    static SPAWN: Once = Once::new();
    let Some(reclaimer) = RECLAIMER.get() else {
      return;
    };
    SPAWN.call_once(|| {
      // Without the thread memory is only reclaimed later, so a failure here is not fatal.
      let _ = std::thread::Builder::new()
        .name("rolldown-reclaim".to_string())
        .spawn(|| run(RECLAIMER.get().expect("set before the thread starts")));
    });
    *reclaimer.events.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    reclaimer.changed.notify_one();
  }

  fn run(reclaimer: &Reclaimer) {
    let mut handled = 0;
    loop {
      let mut events = reclaimer.events.lock().unwrap_or_else(PoisonError::into_inner);
      while *events == handled {
        events = reclaimer.changed.wait(events).unwrap_or_else(PoisonError::into_inner);
      }
      drop(events);
      for retry in 0..=RETRIES {
        let backoff =
          if retry == 0 { Duration::ZERO } else { Duration::from_secs(1 << (retry - 1)) };
        handled = wait_until_quiet(reclaimer, handled, reclaimer.delay + backoff);
        if reclaim(reclaimer) {
          break;
        }
      }
    }
  }

  /// Returns once nothing notified for `quiet` and no build runs. Builds that run back to back
  /// never leave a quiet window, so each thread keeps the most it ever held. For such processes,
  /// `ROLLDOWN_MEMORY_RECLAIM_BETWEEN_BUILDS` releases the pool threads without a purge on each
  /// notification that comes while no build runs. It is off by default, see
  /// internal-docs/memory-reclaim/design.md
  fn wait_until_quiet(reclaimer: &Reclaimer, mut seen: u64, quiet: Duration) -> u64 {
    loop {
      let events = reclaimer.events.lock().unwrap_or_else(PoisonError::into_inner);
      let (events, wait) = reclaimer
        .changed
        .wait_timeout_while(events, quiet, |events| *events == seen)
        .unwrap_or_else(PoisonError::into_inner);
      seen = *events;
      drop(events);
      if rolldown::has_active_build() {
        continue;
      }
      if wait.timed_out() {
        return seen;
      }
      if reclaimer.release_between_builds {
        release_pool_threads(reclaimer, RELEASE_DEADLINE);
      }
    }
  }

  /// Returns whether it reached every pool thread.
  fn reclaim(reclaimer: &Reclaimer) -> bool {
    let reached = release_pool_threads(reclaimer, THREAD_DEADLINE);
    reclaim_js_threads();
    purge();
    reached
  }

  /// Only the thread that owns a page can settle the frees other threads queued on it, so every
  /// thread that allocates for a build has to run this itself.
  fn release_pool_threads(reclaimer: &Reclaimer, deadline: Duration) -> bool {
    let workers_reached = reclaim_tokio_workers(&reclaimer.tokio, deadline);
    // Module sources are read on blocking threads, so their pages hold much of a build.
    let blocking_reached =
      reclaim_blocking_threads(&reclaimer.tokio, reclaimer.max_blocking_threads, deadline);
    rayon::broadcast(|_| release_thread_pages());
    workers_reached && blocking_reached
  }

  /// Settles this thread's pages and gives up the ones still in use. The free that later empties
  /// such a page returns it on whatever thread it runs, so this thread is not needed again.
  fn release_thread_pages() {
    // SAFETY: called on the owner thread, outside any other mimalloc call.
    unsafe { mi_theap_quiescent_abandon(mi_theap_get_default()) };
  }

  fn purge() {
    // SAFETY: mimalloc allows this from any thread.
    unsafe { mi_arenas_purge() };
  }

  /// One round reaches each thread once. A thread waits until every thread has taken its task, so
  /// the remaining tasks have to go to other threads. It settles only after that wait. Settling
  /// takes a global mimalloc lock, so threads settle one at a time, and a thread that waited after
  /// settling was held until the last one was done. The deadline keeps a busy or stuck thread from
  /// holding up the rest, which an unbounded barrier would do.
  struct Round {
    id: u64,
    pending: Mutex<usize>,
    all_reached: Condvar,
    deadline: Instant,
  }

  impl Round {
    fn new(threads: usize, deadline: Duration) -> Arc<Self> {
      Arc::new(Self {
        id: NEXT_ROUND.fetch_add(1, Ordering::Relaxed),
        pending: Mutex::new(threads),
        all_reached: Condvar::new(),
        deadline: Instant::now() + deadline,
      })
    }

    fn settle_this_thread(&self) {
      if RECLAIMED_ROUND.get() == self.id {
        return;
      }
      RECLAIMED_ROUND.set(self.id);
      let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
      *pending -= 1;
      if *pending == 0 {
        self.all_reached.notify_all();
      } else {
        let timeout = self.deadline.saturating_duration_since(Instant::now());
        pending = self
          .all_reached
          .wait_timeout_while(pending, timeout, |pending| *pending > 0)
          .unwrap_or_else(PoisonError::into_inner)
          .0;
      }
      drop(pending);
      release_thread_pages();
    }

    fn all_reached(&self) -> bool {
      *self.pending.lock().unwrap_or_else(PoisonError::into_inner) == 0
    }
  }

  fn reclaim_tokio_workers(tokio: &Handle, deadline: Duration) -> bool {
    let workers = tokio.metrics().num_workers();
    let round = Round::new(workers, deadline);
    let tasks: Vec<_> = (0..workers)
      .map(|_| {
        let round = Arc::clone(&round);
        tokio.spawn(async move { round.settle_this_thread() })
      })
      .collect();
    for task in tasks {
      let _ = tokio.block_on(task);
    }
    round.all_reached()
  }

  fn reclaim_blocking_threads(tokio: &Handle, threads: usize, deadline: Duration) -> bool {
    let round = Round::new(threads, deadline);
    let tasks: Vec<_> = (0..threads)
      .map(|_| {
        let round = Arc::clone(&round);
        tokio.spawn_blocking(move || round.settle_this_thread())
      })
      .collect();
    for task in tasks {
      let _ = tokio.block_on(task);
    }
    round.all_reached()
  }
}

#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
))]
pub use imp::{configure, init, notify, register_js_thread};

#[cfg(not(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
)))]
pub fn notify() {}

#[cfg(not(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
)))]
pub fn register_js_thread(_env: &napi::Env) {}

#[cfg(all(
  not(target_family = "wasm"),
  any(feature = "default_global_allocator", target_env = "ohos")
))]
pub fn configure() -> bool {
  false
}

#[cfg(all(
  not(target_family = "wasm"),
  any(feature = "default_global_allocator", target_env = "ohos")
))]
pub fn init(_tokio: napi::tokio::runtime::Handle, _max_blocking_threads: usize) {}
