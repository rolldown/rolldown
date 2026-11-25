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
  /// A round that misses a thread runs again after 1, 2, 4 and 8 s.
  const RETRIES: u32 = 4;

  unsafe extern "C" {
    fn mi_theap_get_default() -> *mut c_void;
    fn mi_theap_quiescent_abandon(theap: *mut c_void);
    fn mi_arenas_purge();
    fn mi_version() -> c_int;
    fn mi_option_set_default(option: c_int, value: c_long);
  }

  /// The mimalloc whose `mimalloc.h` the declarations above and the option numbers below match.
  /// Debug builds, which CI tests with, stop on any other version, so an update checks them again.
  const CHECKED_MIMALLOC: c_int = 30700;

  struct Reclaimer {
    events: Mutex<u64>,
    changed: Condvar,
    delay: Duration,
    tokio: Handle,
    max_blocking_threads: usize,
  }

  static DELAY: OnceLock<Duration> = OnceLock::new();
  static RECLAIMER: OnceLock<Reclaimer> = OnceLock::new();
  static NEXT_ROUND: AtomicU64 = AtomicU64::new(1);

  /// One per JS thread: the main thread and each worker thread that loaded rolldown.
  static JS_THREADS: Mutex<Vec<ThreadsafeFunction<(), (), (), Status, false, true>>> =
    Mutex::new(Vec::new());

  thread_local! {
    static RECLAIMED_ROUND: Cell<u64> = const { Cell::new(0) };
    static JS_THREAD_REGISTERED: Cell<bool> = const { Cell::new(false) };
  }

  unsafe extern "C" fn collect_on_js_thread(
    _env: napi::sys::napi_env,
    _info: napi::sys::napi_callback_info,
  ) -> napi::sys::napi_value {
    release_thread_pages();
    purge_arenas();
    std::ptr::null_mut()
  }

  pub fn register_js_thread(env: &Env) {
    if RECLAIMER.get().is_none() || JS_THREAD_REGISTERED.get() {
      return;
    }
    JS_THREAD_REGISTERED.set(true);
    if let Ok(callback) = env
      .create_function::<(), ()>("rolldownMemoryReclaim", collect_on_js_thread)
      .and_then(|function| {
        function.build_threadsafe_function().callee_handled::<false>().weak::<true>().build()
      })
    {
      JS_THREADS.lock().unwrap_or_else(PoisonError::into_inner).push(callback);
    }
  }

  pub fn configure() -> bool {
    // SAFETY: plain call with no arguments.
    let version = unsafe { mi_version() };
    debug_assert_eq!(
      version, CHECKED_MIMALLOC,
      "mimalloc changed, check the declarations and option numbers in memory_reclaim.rs"
    );
    if version < 30600 {
      return false;
    }
    let delay = match std::env::var("ROLLDOWN_MEMORY_RECLAIM_DELAY").as_deref() {
      Ok("off") => return false,
      Ok(value) => value.parse().map_or(DEFAULT_DELAY, Duration::from_millis),
      Err(_) => DEFAULT_DELAY,
    };
    let _ = DELAY.set(delay);
    set_allocator_options(version);
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
      tokio,
      max_blocking_threads,
    });
    rolldown::set_memory_released_hook(notify);
  }

  fn set_allocator_options(version: c_int) {
    // The Rust bindings have no option numbers, so they come from `mimalloc.h`.
    const PAGE_COMMIT_ON_DEMAND: c_int = 40;
    #[cfg(target_os = "linux")]
    const ARENA_PURGE_MIN_SIZE: c_int = 44;
    if version != CHECKED_MIMALLOC {
      return;
    }
    // A default does not replace a value set through a `MIMALLOC_*` environment variable.
    // SAFETY: plain calls with option numbers from `mimalloc.h`.
    unsafe {
      // Without overcommit, as on Windows, the OS charges commit for every page a thread claims,
      // even untouched ones. 2 commits pages on demand only on such systems.
      mi_option_set_default(PAGE_COMMIT_ON_DEMAND, 2);
      // With transparent huge pages, mimalloc purges in 2 MiB units, which skips most ranges.
      #[cfg(target_os = "linux")]
      mi_option_set_default(ARENA_PURGE_MIN_SIZE, 64);
    }
  }

  pub fn notify() {
    static SPAWN: Once = Once::new();
    let Some(reclaimer) = RECLAIMER.get() else {
      return;
    };
    SPAWN.call_once(|| {
      // Without the thread memory is only reclaimed later, so a failure here is not fatal.
      let _ = std::thread::Builder::new().name("rolldown-reclaim".to_string()).spawn(|| {
        let reclaimer = RECLAIMER.get().expect("set before the thread starts");
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
      });
    });
    *reclaimer.events.lock().unwrap_or_else(PoisonError::into_inner) += 1;
    reclaimer.changed.notify_one();
  }

  /// Returns once nothing notified for `quiet` and no build runs. Builds that run back to back
  /// never leave a quiet window, so memory drops only after the last one.
  fn wait_until_quiet(reclaimer: &Reclaimer, mut seen: u64, quiet: Duration) -> u64 {
    loop {
      let events = reclaimer.events.lock().unwrap_or_else(PoisonError::into_inner);
      let (events, timed_out) = if rolldown::has_active_build() {
        let events = reclaimer
          .changed
          .wait_while(events, |events| *events == seen)
          .unwrap_or_else(PoisonError::into_inner);
        (events, false)
      } else {
        let (events, wait) = reclaimer
          .changed
          .wait_timeout_while(events, quiet, |events| *events == seen)
          .unwrap_or_else(PoisonError::into_inner);
        (events, wait.timed_out())
      };
      seen = *events;
      drop(events);
      if rolldown::has_active_build() {
        continue;
      }
      if timed_out {
        return seen;
      }
    }
  }

  fn reclaim(reclaimer: &Reclaimer) -> bool {
    let workers_reached = reclaim_tokio_workers(&reclaimer.tokio, THREAD_DEADLINE);
    let blocking_reached =
      reclaim_blocking_threads(&reclaimer.tokio, reclaimer.max_blocking_threads, THREAD_DEADLINE);
    rayon::broadcast(|_| release_thread_pages());
    purge_arenas();
    // mimalloc skips a purge while another thread purges. Each JS thread purges its own pages, so it
    // is called only after this purge ends.
    JS_THREADS
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .retain(|callback| callback.call((), ThreadsafeFunctionCallMode::NonBlocking) == Status::Ok);
    workers_reached && blocking_reached
  }

  fn purge_arenas() {
    // SAFETY: mimalloc allows this from any thread.
    unsafe { mi_arenas_purge() };
  }

  fn release_thread_pages() {
    // SAFETY: called on the owner thread, outside any other mimalloc call.
    unsafe { mi_theap_quiescent_abandon(mi_theap_get_default()) };
  }

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
