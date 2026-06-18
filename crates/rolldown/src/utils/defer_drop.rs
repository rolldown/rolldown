//! Drop heavy values on rayon worker threads instead of the caller's thread.
//!
//! Freeing the link stage output (module_table, metas, stmt_infos, ...)
//! takes ~15ms of hot-thread time on a 20k-module build, after the output is
//! already produced. The rayon workers are idle by then, so shipping the
//! drop to one of them takes the free() off the critical path.
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

use std::sync::{Condvar, Mutex, PoisonError};

/// Number of `spawn_drop` closures that have been enqueued but not yet
/// finished dropping their value.
static PENDING: Mutex<usize> = Mutex::new(0);
static PENDING_IS_ZERO: Condvar = Condvar::new();

/// Decrements `PENDING` on drop, so the count goes down even if the deferred
/// value's `Drop` impl panics — a panic must not wedge `drain()` forever.
struct PendingGuard;

impl Drop for PendingGuard {
  fn drop(&mut self) {
    let mut pending = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
    *pending -= 1;
    if *pending == 0 {
      PENDING_IS_ZERO.notify_all();
    }
  }
}

/// Drop `value` on a rayon worker thread instead of the caller's thread.
///
/// See the module docs for the invariants call sites must uphold.
pub fn spawn_drop<T: Send + 'static>(value: T) {
  *PENDING.lock().unwrap_or_else(PoisonError::into_inner) += 1;
  rayon::spawn(move || {
    let _guard = PendingGuard;
    drop(value);
  });
}

/// Block until every pending deferred drop has finished.
///
/// Called once at build entry (`BundleFactory::build_bundle`) so a queued
/// watch rebuild can never overlap the previous build's frees on the shared
/// rayon pool. Expected to be a no-op in steady state; see the module docs.
pub fn drain() {
  let mut pending = PENDING.lock().unwrap_or_else(PoisonError::into_inner);
  while *pending > 0 {
    pending = PENDING_IS_ZERO.wait(pending).unwrap_or_else(PoisonError::into_inner);
  }
}
