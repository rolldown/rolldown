//! A [`GlobalAlloc`] wrapper around mimalloc that tracks live bytes, peak
//! bytes, and allocation counts for the Rust side of rolldown.
//!
//! V8 never allocates through the Rust global allocator, so these counters are
//! free of JS-heap and GC noise, unlike `process.memoryUsage()`. Consumers only
//! install the wrapper behind their `tracking_allocator` feature
//! (`rolldown_binding`, `bench`) and keep plain [`MiMalloc`] by default.
//!
//! # Design: batched per-thread counting
//!
//! A single set of global atomics updated on every allocation serializes all
//! worker threads on the same cache lines — measured at +30% to +200% on the
//! `bundle@*` benchmarks. Instead, each thread accumulates deltas in plain
//! thread-local cells and flushes them into the globals once per
//! [`FLUSH_BYTES`] of drift or [`FLUSH_OPS`] operations, cutting cross-core
//! traffic by roughly the batch factor.
//!
//! The trade-off is bounded staleness rather than exactness:
//!
//! - `live_bytes`/`peak_bytes` lag reality by at most `threads × FLUSH_BYTES`.
//! - counts lag by at most `threads × FLUSH_OPS`.
//! - an exit guard flushes a thread's pending deltas when the thread exits, so
//!   a retired thread leaves no drift. The few operations that run while the
//!   guard registers, or after its destruction during thread teardown, skip
//!   batching and update the globals directly. [`stats`] still clamps a small
//!   transient negative drift of live bytes to zero.
//!
//! Callers measure whole builds, where megabyte-scale slack is noise-level.
//!
//! The thread-locals are const-initialized and `Drop`-free, so accessing them
//! inside the allocator never allocates and never fails, even during thread
//! teardown.
//!
//! `tasks/track_memory_allocations` uses the simpler unbatched counting for
//! single-threaded CI snapshots, where exactness matters and contention does
//! not exist.

use std::{
  alloc::{GlobalAlloc, Layout},
  cell::Cell,
  sync::atomic::{AtomicIsize, AtomicUsize, Ordering::Relaxed},
};

use mimalloc_safe::MiMalloc;

/// Flush a thread's pending byte delta once its magnitude reaches this.
const FLUSH_BYTES: isize = 64 * 1024;
/// Flush a thread's pending counts once this many operations accumulate.
const FLUSH_OPS: usize = 1024;

// Signed: interleaved flushes can transiently dip below zero (e.g. thread A
// allocates, thread B frees that memory and flushes first).
static LIVE_BYTES: AtomicIsize = AtomicIsize::new(0);
static PEAK_BYTES: AtomicIsize = AtomicIsize::new(0);
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static REALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

thread_local! {
  static PENDING_BYTES: Cell<isize> = const { Cell::new(0) };
  static PENDING_ALLOCS: Cell<usize> = const { Cell::new(0) };
  static PENDING_REALLOCS: Cell<usize> = const { Cell::new(0) };
  static PENDING_OPS: Cell<usize> = const { Cell::new(0) };
  /// Re-entrancy latch for [`arm_exit_flush`]: registering the exit guard can
  /// itself allocate, and that inner call must not recurse into registration.
  static ARMING: Cell<bool> = const { Cell::new(false) };
  /// Armed on a thread's first tracked operation. Dropping it at thread exit
  /// flushes the thread's pending deltas, so a retired thread leaves no drift.
  static EXIT_FLUSH: ExitFlush = const { ExitFlush };
}

/// Flushes the thread's pending deltas when the thread exits.
struct ExitFlush;

impl Drop for ExitFlush {
  fn drop(&mut self) {
    flush();
  }
}

/// Returns `true` when the exit guard is usable on this thread. `false` means
/// the guard is mid-registration or already destroyed; the caller must then
/// apply its delta straight to the globals, because the pending cells may
/// never flush again on this thread.
fn arm_exit_flush() -> bool {
  ARMING.with(|arming| {
    if arming.get() {
      return false;
    }
    arming.set(true);
    let armed = EXIT_FLUSH.try_with(|_| ()).is_ok();
    arming.set(false);
    armed
  })
}

/// Forwards every call to [`MiMalloc`] and maintains the counters returned by
/// [`stats`]. Install it with `#[global_allocator]`.
pub struct TrackingAllocator;

fn record(bytes_delta: isize, is_realloc: bool) {
  if !arm_exit_flush() {
    if is_realloc {
      REALLOC_COUNT.fetch_add(1, Relaxed);
    } else {
      ALLOC_COUNT.fetch_add(1, Relaxed);
    }
    let live = LIVE_BYTES.fetch_add(bytes_delta, Relaxed) + bytes_delta;
    PEAK_BYTES.fetch_max(live, Relaxed);
    return;
  }
  if is_realloc {
    PENDING_REALLOCS.with(|c| c.set(c.get() + 1));
  } else {
    PENDING_ALLOCS.with(|c| c.set(c.get() + 1));
  }
  let pending = PENDING_BYTES.with(|c| {
    let pending = c.get() + bytes_delta;
    c.set(pending);
    pending
  });
  let ops = PENDING_OPS.with(|c| {
    let ops = c.get() + 1;
    c.set(ops);
    ops
  });
  if pending.abs() >= FLUSH_BYTES || ops >= FLUSH_OPS {
    flush();
  }
}

fn flush() {
  let bytes = PENDING_BYTES.with(|c| c.replace(0));
  let allocs = PENDING_ALLOCS.with(|c| c.replace(0));
  let reallocs = PENDING_REALLOCS.with(|c| c.replace(0));
  PENDING_OPS.with(|c| c.set(0));

  if allocs > 0 {
    ALLOC_COUNT.fetch_add(allocs, Relaxed);
  }
  if reallocs > 0 {
    REALLOC_COUNT.fetch_add(reallocs, Relaxed);
  }
  let live = LIVE_BYTES.fetch_add(bytes, Relaxed) + bytes;
  PEAK_BYTES.fetch_max(live, Relaxed);
}

// SAFETY: every method forwards to `MiMalloc`, which is a sound `GlobalAlloc`;
// the counters never touch the returned memory.
unsafe impl GlobalAlloc for TrackingAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    let ptr = unsafe { MiMalloc.alloc(layout) };
    if !ptr.is_null() {
      record(isize::try_from(layout.size()).unwrap_or(isize::MAX), false);
    }
    ptr
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    let ptr = unsafe { MiMalloc.alloc_zeroed(layout) };
    if !ptr.is_null() {
      record(isize::try_from(layout.size()).unwrap_or(isize::MAX), false);
    }
    ptr
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    let new_ptr = unsafe { MiMalloc.realloc(ptr, layout, new_size) };
    if !new_ptr.is_null() {
      let delta = isize::try_from(new_size).unwrap_or(isize::MAX)
        - isize::try_from(layout.size()).unwrap_or(isize::MAX);
      record(delta, true);
    }
    new_ptr
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    unsafe { MiMalloc.dealloc(ptr, layout) };
    // A free is not a new operation for the counts, but it must move the byte
    // balance and can trigger a flush.
    let size = isize::try_from(layout.size()).unwrap_or(isize::MAX);
    if !arm_exit_flush() {
      LIVE_BYTES.fetch_sub(size, Relaxed);
      return;
    }
    let pending = PENDING_BYTES.with(|c| {
      let pending = c.get() - size;
      c.set(pending);
      pending
    });
    if pending.abs() >= FLUSH_BYTES {
      flush();
    }
  }
}

/// A snapshot of the counters. Meaningful only while [`TrackingAllocator`] is
/// installed as the global allocator.
#[derive(Debug, Clone, Copy)]
pub struct MemoryStats {
  /// Bytes currently allocated and not yet freed, since process start.
  /// Lags reality by at most `threads × FLUSH_BYTES` (unflushed deltas).
  pub live_bytes: usize,
  /// Highest flushed `live_bytes` seen since process start or the last
  /// [`reset`].
  pub peak_bytes: usize,
  /// Successful `alloc`/`alloc_zeroed` calls since the last [`reset`].
  pub alloc_count: usize,
  /// Successful `realloc` calls since the last [`reset`].
  pub realloc_count: usize,
}

/// Snapshot the counters, flushing the calling thread's pending deltas first.
/// Other threads' unflushed deltas stay invisible until they flush.
pub fn stats() -> MemoryStats {
  flush();
  MemoryStats {
    live_bytes: usize::try_from(LIVE_BYTES.load(Relaxed)).unwrap_or(0),
    peak_bytes: usize::try_from(PEAK_BYTES.load(Relaxed)).unwrap_or(0),
    alloc_count: ALLOC_COUNT.load(Relaxed),
    realloc_count: REALLOC_COUNT.load(Relaxed),
  }
}

/// Start a new measuring window: the peak restarts from the current live bytes
/// and the counts restart from zero. `live_bytes` is never reset, so it always
/// reflects allocations since process start. Concurrent allocations during the
/// reset can leak into or out of the window; callers measure around a build,
/// where that slack is noise-level.
pub fn reset() {
  flush();
  PEAK_BYTES.store(LIVE_BYTES.load(Relaxed), Relaxed);
  ALLOC_COUNT.store(0, Relaxed);
  REALLOC_COUNT.store(0, Relaxed);
}
