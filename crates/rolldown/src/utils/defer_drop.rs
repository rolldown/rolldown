//! Drop heavy values on a dedicated serial worker instead of the caller's
//! thread or the shared Rayon pool.
//!
//! Freeing the link stage output (module_table, metas, stmt_infos, ...)
//! takes ~15ms of hot-thread time on a 20k-module build, after the output is
//! already produced. Shipping the drop to a maintenance worker takes the
//! free() off the critical path without making the next one-worker rebuild
//! wait on work queued behind itself.
//!
//! Deferred drops cannot pile up across builds: this is ENFORCED, not
//! assumed. Every pending drop is counted, and [`drain`] blocks until the
//! count is zero. It is called at every entry point that starts scan/link/
//! render work on the shared rayon pool: `BundleFactory::build_bundle` (full
//! and incremental builds) and the three HMR partial-scan entries in
//! `impl_bundler_hmr.rs`, which bypass `build_bundle`. In steady state it is
//! a no-op (a single uncontended lock check): the frees take ~20ms while any
//! build that could produce the next pair takes hundreds of ms. Even in the
//! worst case, drain blocks for no longer than main spent doing the same
//! frees inline on the same thread.
//!
//! Within a single build the count stays bounded by construction: exactly one
//! object is enqueued, once per build, at the build boundary (right after
//! `generate()` returns). There is no per-item or per-iteration use.
//!
//! Within the SAME build, the free intentionally overlaps the
//! `render_error`/`generateBundle` hooks and the write tail — that overlap IS
//! the optimization. This never extends a memory window relative to main:
//! main held `link_stage_output` alive until `bundle_up` scope end, i.e.
//! through those same hooks; here it is freed concurrently DURING them
//! (measured peak RSS is flat). Only values main itself kept alive through
//! the overlapped region are eligible — that is why the non-incremental
//! `symbol_db`, which main frees inline BEFORE the hooks, is NOT deferred.
//! The dropped value is exclusively owned, so overlap can only cost bounded
//! background CPU, never correctness.
//!
//! The counter is process-global rather than per-bundler on purpose: with
//! concurrent bundler instances, the worst case is one instance waiting (at
//! most the ~20ms free time) for another instance's drops, or a few ms of
//! background free work overlapping a concurrent build — accepted as far
//! cheaper than per-instance plumbing for a non-correctness concern.
//!
//! Do NOT use [`spawn_drop`] for values that are still live before the output
//! exists (e.g. the per-module AST arenas, which main intentionally frees
//! before chunk instantiation/minify allocate — deferring them would extend
//! their memory window and spike peak RSS), or for anything enqueued in a
//! loop.

#[cfg(not(target_family = "wasm"))]
use std::{
  panic::{AssertUnwindSafe, catch_unwind},
  sync::{
    Condvar, LazyLock, Mutex, PoisonError,
    mpsc::{Sender, channel},
  },
};

/// Number of `spawn_drop` closures that have been enqueued but not yet
/// finished dropping their value.
#[cfg(not(target_family = "wasm"))]
static PENDING: Mutex<usize> = Mutex::new(0);
#[cfg(not(target_family = "wasm"))]
static PENDING_IS_ZERO: Condvar = Condvar::new();

#[cfg(not(target_family = "wasm"))]
type DropJob = Box<dyn FnOnce() + Send + 'static>;

#[cfg(not(target_family = "wasm"))]
fn run_drop_safely(drop_job: impl FnOnce()) {
  if let Err(payload) = catch_unwind(AssertUnwindSafe(drop_job))
    && let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
  {
    // The original unwind is contained. Quarantine only the nested payload
    // produced by a hostile panic-payload destructor.
    std::mem::forget(nested_payload);
  }
}

/// Deferred drops use their own serial worker instead of inheriting the
/// caller's Rayon registry. A one-worker build may begin its next rebuild on
/// the same Rayon worker that queued the previous drop; putting the drop back
/// on that pool and then waiting in `drain()` deadlocks the worker against its
/// own queue.
#[cfg(not(target_family = "wasm"))]
static DROP_QUEUE: LazyLock<Option<Sender<DropJob>>> = LazyLock::new(|| {
  let (sender, receiver) = channel::<DropJob>();
  let worker =
    std::thread::Builder::new().name("rolldown-deferred-drop".to_string()).spawn(move || {
      while let Ok(job) = receiver.recv() {
        // Retire the pending count only after the value and any caught panic
        // payload have finished destruction.
        run_drop_safely(job);
        drop(PendingGuard);
      }
    });
  worker.ok().map(|_| sender)
});

/// Decrements `PENDING` on drop, so the count goes down even if the deferred
/// value's `Drop` impl panics — a panic must not wedge `drain()` forever.
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
  // On wasm the thread that later calls `drain()` may be the browser main
  // thread, where the matching `Condvar::wait` lowers to `memory.atomic.wait`
  // and is illegal ("Atomics.wait cannot be called in this context"). Drop
  // inline so there is never a cross-build wait to perform there.
  #[cfg(target_family = "wasm")]
  drop(value);
  #[cfg(not(target_family = "wasm"))]
  {
    if let Some(sender) = &*DROP_QUEUE {
      *PENDING.lock().unwrap_or_else(PoisonError::into_inner) += 1;
      let job: DropJob = Box::new(move || drop(value));
      if let Err(error) = sender.send(job) {
        // The worker should be process-lived. If it ever exits unexpectedly,
        // preserve correctness by completing this drop synchronously without
        // exposing a user-defined Drop panic that the worker path contains.
        run_drop_safely(error.0);
        drop(PendingGuard);
      }
    } else {
      // Thread creation can fail under resource pressure. Deferred destruction
      // is an optimization, so preserve correctness with the same panic
      // containment instead of failing the build or poisoning initialization.
      run_drop_safely(|| drop(value));
    }
  }
}

/// Block until every pending deferred drop has finished.
///
/// Called once at build entry (`BundleFactory::build_bundle`) so a queued
/// watch rebuild can never overlap the previous build's frees on the shared
/// rayon pool. Expected to be a no-op in steady state; see the module docs.
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
    panic::panic_any,
    sync::mpsc::{Receiver, SyncSender, sync_channel},
    time::{Duration, Instant},
  };

  use super::{PENDING, drain, spawn_drop};

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
      // Keep the sole Rayon worker occupied. A deferred drop accidentally
      // queued into this registry cannot run until this gate is released.
      release_rx.recv().unwrap();
    });

    queued_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    dropped_rx
      .recv_timeout(Duration::from_secs(1))
      .expect("deferred drop was queued behind its caller in the one-worker Rayon pool");
    release_tx.send(()).unwrap();
    drain();
  }

  #[test]
  fn hostile_panic_payload_does_not_kill_worker_or_strand_pending_count() {
    const CHILD_ENV: &str = "ROLLDOWN_TEST_DEFER_DROP_HOSTILE_PAYLOAD_CHILD";

    if std::env::var_os(CHILD_ENV).is_some() {
      struct BlockingPanicPayload {
        entered: SyncSender<()>,
        release: Receiver<()>,
      }

      impl Drop for BlockingPanicPayload {
        fn drop(&mut self) {
          self.entered.send(()).unwrap();
          self.release.recv().unwrap();
          panic!("intentional deferred-drop panic payload destructor panic");
        }
      }

      struct PanicOnDrop(Option<BlockingPanicPayload>);

      impl Drop for PanicOnDrop {
        fn drop(&mut self) {
          panic_any(self.0.take().unwrap());
        }
      }

      let (payload_entered_tx, payload_entered_rx) = sync_channel(0);
      let (release_payload_tx, release_payload_rx) = sync_channel(0);
      spawn_drop(PanicOnDrop(Some(BlockingPanicPayload {
        entered: payload_entered_tx,
        release: release_payload_rx,
      })));
      payload_entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("the worker must begin destroying the caught panic payload");

      let (drained_tx, drained_rx) = sync_channel(1);
      std::thread::spawn(move || {
        drain();
        drained_tx.send(()).unwrap();
      });
      assert!(
        drained_rx.recv_timeout(Duration::from_millis(100)).is_err(),
        "drain must wait until caught panic payload destruction finishes"
      );

      release_payload_tx.send(()).unwrap();
      drained_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("drain must finish after the caught panic payload is contained");

      let (second_dropped_tx, second_dropped_rx) = sync_channel(1);
      spawn_drop(NotifyOnDrop(second_dropped_tx));
      second_dropped_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("the worker must survive the hostile payload and execute the queued drop");

      let deadline = Instant::now() + Duration::from_secs(1);
      loop {
        let pending = *PENDING.lock().unwrap();
        if pending == 0 {
          break;
        }
        assert!(
          Instant::now() < deadline,
          "a queued job was discarded when the worker exited, leaving PENDING at {pending}"
        );
        std::thread::yield_now();
      }
      return;
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
      .arg("--exact")
      .arg(
        "utils::defer_drop::tests::hostile_panic_payload_does_not_kill_worker_or_strand_pending_count",
      )
      .arg("--nocapture")
      .env(CHILD_ENV, "1")
      .output()
      .expect("the deferred-drop subprocess must start");
    assert!(
      output.status.success(),
      "deferred-drop panic payload destruction must stay contained; status={:?}\nstdout={}\nstderr={}",
      output.status.code(),
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }
}
