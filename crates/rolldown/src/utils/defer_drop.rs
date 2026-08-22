//! Drop heavy values on a dedicated serial worker instead of the caller's
//! thread or the shared Rayon pool.
//!
//! Call-site rules:
//!
//! - [`drain`] must run at every entry that starts scan/link/render work on
//!   the shared rayon pool, so deferred drops can never overlap the next
//!   build: `BundleFactory::build_bundle` plus the HMR partial-scan entries in
//!   `impl_bundler_hmr.rs`, which bypass it.
//! - Only defer values the caller would otherwise have held alive through the
//!   overlapped region (the hooks and write tail after `generate()`).
//!   Deferring anything it frees earlier — the per-module AST arenas, the
//!   non-incremental `symbol_db` — extends that value's memory window and
//!   spikes peak RSS.
//! - Enqueue once per build at the build boundary, never per item or in a
//!   loop.

#[cfg(not(target_family = "wasm"))]
use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  sync::{
    Condvar, LazyLock, Mutex, PoisonError,
    mpsc::{Sender, channel},
  },
};

/// `spawn_drop` closures enqueued but not yet finished dropping their value.
#[cfg(not(target_family = "wasm"))]
static PENDING: Mutex<usize> = Mutex::new(0);
#[cfg(not(target_family = "wasm"))]
static PENDING_IS_ZERO: Condvar = Condvar::new();

#[cfg(not(target_family = "wasm"))]
type DropJob = Box<dyn FnOnce() + Send + 'static>;

/// Contain a panicking user `Drop` impl so it cannot kill the dedicated
/// worker (or unwind into the caller on the fallback paths).
#[cfg(not(target_family = "wasm"))]
fn run_drop_safely(drop_job: impl FnOnce()) {
  if let Err(payload) = catch_unwind(AssertUnwindSafe(drop_job)) {
    // Destroying the caught payload runs a user destructor too, outside any
    // unwind, so it needs its own boundary.
    if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(move || drop(payload))) {
      // Containment bottoms out here: a payload that cannot be destroyed is
      // leaked rather than allowed to escape and kill the worker.
      std::mem::forget(nested_payload);
    }
  }
}

/// Own serial worker rather than the caller's Rayon registry: a one-worker
/// build may start its next rebuild on the same Rayon worker that queued the
/// previous drop, so queueing the drop there and then waiting in `drain()`
/// deadlocks that worker against its own queue.
#[cfg(not(target_family = "wasm"))]
static DROP_QUEUE: LazyLock<Option<Sender<DropJob>>> = LazyLock::new(|| {
  let (sender, receiver) = channel::<DropJob>();
  let worker =
    std::thread::Builder::new().name("rolldown-deferred-drop".to_string()).spawn(move || {
      while let Ok(job) = receiver.recv() {
        let _guard = PendingGuard;
        run_drop_safely(job);
      }
    });
  worker.ok().map(|_| sender)
});

/// Decrements `PENDING` on drop, so a panicking deferred `Drop` cannot wedge
/// `drain()` forever. Construct it *before* the destruction it accounts for,
/// never after, so an escaping panic unwinds through it instead of skipping it.
#[cfg(not(target_family = "wasm"))]
struct PendingGuard;

#[cfg(not(target_family = "wasm"))]
impl Drop for PendingGuard {
  fn drop(&mut self) {
    let mut pending = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    *pending -= 1;
    if *pending == 0 {
      PENDING_IS_ZERO.notify_all();
    }
  }
}

/// Drop `value` on the dedicated deferred-drop worker.
///
/// See the module docs for the invariants call sites must uphold.
pub fn spawn_drop<T: Send + 'static>(value: T) {
  // `drain()` may run on the browser main thread, where `Condvar::wait` lowers
  // to an illegal `memory.atomic.wait`. Drop inline so there is never a
  // cross-build wait to perform on wasm.
  #[cfg(target_family = "wasm")]
  drop(value);
  #[cfg(not(target_family = "wasm"))]
  {
    if let Some(sender) = &*DROP_QUEUE {
      *PENDING.lock().unwrap_or_else(PoisonError::into_inner) += 1;
      let job: DropJob = Box::new(move || drop(value));
      if let Err(error) = sender.send(job) {
        // Worker gone: finish the drop inline, still contained.
        let _guard = PendingGuard;
        run_drop_safely(error.0);
      }
    } else {
      // Thread spawn failed. Deferral is only an optimization, so fall back to
      // an inline drop instead of failing the build.
      run_drop_safely(|| drop(value));
    }
  }
}

/// Block until every pending deferred drop has finished. A no-op in steady
/// state; see the module docs for where it must be called.
pub fn drain() {
  // wasm drops inline in `spawn_drop`, so nothing is ever pending; a
  // `Condvar::wait` here would crash on the browser main thread.
  #[cfg(not(target_family = "wasm"))]
  {
    let mut pending = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    while *pending > 0 {
      pending = PENDING_IS_ZERO.wait(pending).unwrap_or_else(PoisonError::into_inner);
    }
  }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
  use std::{
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    time::Duration,
  };

  use super::{drain, spawn_drop};

  struct NotifyOnDrop(SyncSender<()>);

  impl Drop for NotifyOnDrop {
    fn drop(&mut self) {
      self.0.send(()).unwrap();
    }
  }

  #[test]
  fn deferred_drop_does_not_depend_on_the_callers_one_worker_rayon_pool() {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let (dropped_tx, dropped_rx) = sync_channel(1);
    let (queued_tx, queued_rx) = sync_channel(1);
    let (release_tx, release_rx): (SyncSender<()>, Receiver<()>) = sync_channel(0);

    pool.spawn(move || {
      spawn_drop(NotifyOnDrop(dropped_tx));
      queued_tx.send(()).unwrap();
      // Occupy the sole Rayon worker: a drop wrongly queued into this registry
      // cannot run until the gate is released.
      release_rx.recv().unwrap();
    });

    queued_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    dropped_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("deferred drop was queued behind its caller in the one-worker Rayon pool");
    release_tx.send(()).unwrap();
    drain();
  }

  /// A panic payload whose own `Drop` panics.
  struct HostilePayload;

  impl Drop for HostilePayload {
    fn drop(&mut self) {
      panic!("hostile panic payload destructor");
    }
  }

  /// A deferred value whose `Drop` panics with a [`HostilePayload`].
  struct PanicWithHostilePayload;

  impl Drop for PanicWithHostilePayload {
    fn drop(&mut self) {
      std::panic::panic_any(HostilePayload);
    }
  }

  // `PENDING` and the worker are process-global, so an unretired count here
  // wedges `drain()` for every other test in this binary too.
  #[test]
  fn a_panicking_panic_payload_destructor_cannot_wedge_drain() {
    spawn_drop(PanicWithHostilePayload);

    let (drained_tx, drained_rx) = sync_channel(1);
    std::thread::spawn(move || {
      drain();
      let _ = drained_tx.send(());
    });

    drained_rx
      .recv_timeout(Duration::from_secs(10))
      .expect("drain() hung: the drop worker died before retiring its pending count");

    // A dead worker silently demotes every later deferred drop to an inline
    // drop on its caller.
    let (worker_tx, worker_rx) = sync_channel(1);
    spawn_drop(ReportDroppingThread(worker_tx));
    drain();
    assert_eq!(
      worker_rx.recv_timeout(Duration::from_secs(10)).ok().flatten().as_deref(),
      Some("rolldown-deferred-drop"),
      "the deferred-drop worker did not survive the hostile panic payload"
    );
  }

  struct ReportDroppingThread(SyncSender<Option<String>>);

  impl Drop for ReportDroppingThread {
    fn drop(&mut self) {
      let _ = self.0.send(std::thread::current().name().map(ToString::to_string));
    }
  }

  /// A third-level payload whose own `Drop` panics again — it runs when the
  /// *inner* `catch_unwind`'s `Err` is destroyed, outside both unwind
  /// boundaries in `run_drop_safely`.
  struct DoublyHostilePayload;

  impl Drop for DoublyHostilePayload {
    fn drop(&mut self) {
      panic!("doubly hostile panic payload destructor");
    }
  }

  /// A panic payload whose `Drop` panics with a [`DoublyHostilePayload`].
  struct HostilePayloadNestingAnotherHostilePayload;

  impl Drop for HostilePayloadNestingAnotherHostilePayload {
    fn drop(&mut self) {
      std::panic::panic_any(DoublyHostilePayload);
    }
  }

  /// A deferred value whose `Drop` panics with the nested hostile payload.
  struct PanicWithNestedHostilePayload;

  impl Drop for PanicWithNestedHostilePayload {
    fn drop(&mut self) {
      std::panic::panic_any(HostilePayloadNestingAnotherHostilePayload);
    }
  }

  #[test]
  fn a_nested_hostile_panic_payload_cannot_kill_the_worker() {
    spawn_drop(PanicWithNestedHostilePayload);

    // The guard retires the count even if the worker dies, so drain() proves
    // nothing on its own — but it must complete before the probe below, or the
    // probe races the hostile drop.
    let (drained_tx, drained_rx) = sync_channel(1);
    std::thread::spawn(move || {
      drain();
      let _ = drained_tx.send(());
    });
    drained_rx
      .recv_timeout(Duration::from_secs(10))
      .expect("drain() hung: the drop worker died before retiring its pending count");

    // A dead worker would demote later deferred drops to inline drops, letting
    // the same nested payload unwind into a build.
    let (worker_tx, worker_rx) = sync_channel(1);
    spawn_drop(ReportDroppingThread(worker_tx));
    drain();
    assert_eq!(
      worker_rx.recv_timeout(Duration::from_secs(10)).ok().flatten().as_deref(),
      Some("rolldown-deferred-drop"),
      "the deferred-drop worker did not survive the nested hostile panic payload"
    );
  }
}
