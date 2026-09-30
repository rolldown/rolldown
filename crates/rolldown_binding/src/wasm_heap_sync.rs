//! Threaded WASI only: keep every thread's view of the shared memory size current.
//!
//! See internal-docs/wasi-shared-memory-grow/design.md for the evidence and the rejected
//! alternatives, and internal-docs/wasi-shared-memory-grow/implementation.md for the wiring.
//!
//! # What V8 does
//!
//! All threads of the `wasm32-wasip1-threads` build share one wasm memory, and wasi-libc's
//! dlmalloc grows it with `memory.grow` (in `sbrk`) from whichever thread runs out of heap. V8
//! updates the memory size on that thread only and asks every other thread to catch up at its
//! next interrupt check. A thread in a long wasm activation checks late, so it keeps its old
//! size, and `memory.fill`, `memory.copy`, Liftoff atomics and atomic wait/notify are
//! bounds-checked against it. Returning to JS and entering wasm again does not refresh it by
//! itself; a JS call, a Liftoff or non-leaf TurboFan call, or a futex wait that sleeps does.
//! Plain loads and stores and TurboFan atomics are checked by guard pages against the real size
//! on hosts with the wasm trap handler; on hosts without it (V8 builds the handler only for x64
//! and arm64 on Linux, Windows and macOS, plus a few more; Node's `--disable-wasm-trap-handler`
//! turns it off) they are checked against the old size too, so even dlmalloc's own chunk-header
//! stores into pages another thread grew trap.
//!
//! `memory.grow(0)` on the stale thread makes V8 update the size for that thread and returns
//! it. `memory.size` does not help: it returns the same stale size and updates nothing.
//! `scripts/wasi/check-v8-shared-memory-grow.mjs` (`pnpm check:v8-shared-memory-grow`) shows
//! whether a Node still has the bug.
//!
//! # The fix: refresh under the allocator lock
//!
//! `build.rs` links with `--wrap` for every dlmalloc entry point and for `sbrk`. Every entry
//! (the C callers through `__wrap_*`, Rust through [`HeapSyncAlloc`], JS through the renamed
//! `malloc` / `free` exports) runs the real dlmalloc call inside [`locked`]:
//!
//! ```text
//! LOCK (spin; sched_yield every 64 spins; never memory.atomic.wait)
//!   MAX_SEEN_PAGES > LOCAL_PAGES ? memory.grow(0)     catch up with every published growth
//!   __real_xxx()                                      dlmalloc writes chunk headers
//!     └─ __wrap_sbrk: hand out [__heap_end, memory.size) first, else memory.grow(n),
//!                     then memory.grow(0): LOCAL_PAGES = MAX_SEEN_PAGES = new size
//! UNLOCK (Release)                                    the next holder's Acquire sees it
//! ```
//!
//! Only `sbrk` grows the memory, and it only runs under `LOCK`, so the thread that grows
//! publishes the new size before any other thread can enter dlmalloc, and that thread
//! refreshes before dlmalloc touches a byte. calloc's memset and realloc's memcpy run after the
//! refresh too. wasi-libc runs them after dlmalloc has released its own lock, but still inside
//! `LOCK`, so `LOCK` is wider than dlmalloc's lock: a large zeroed allocation or realloc copy
//! holds every other thread's allocator calls (kept on purpose, see
//! internal-docs/wasi-shared-memory-grow/design.md, principle 1). The lock spins like
//! dlmalloc's, so it is safe on a browser main thread.
//!
//! # The break
//!
//! wasi-libc's `sbrk` starts at `memory.size`, the memory the loader created (1 GiB on Node),
//! and leaves the pages between `__heap_end` (the module's own initial memory, where dlmalloc's
//! first segment ends) and that size unused. [`__wrap_sbrk`] keeps its own break that starts
//! at `__heap_end`, so dlmalloc uses those pages first: no thread grows the memory until the heap
//! passes about 960 MiB, and the heap reaches about 1.94 GiB before a heap pointer crosses 2^31,
//! where Node's `node:wasi` rejects it (`EINVAL`, os error 28) in every WASI call that takes a
//! pointer. When it does grow, it grows at least [`GROW_AHEAD`] at once.
//!
//! # Scheduler handoff
//!
//! A task can allocate on worker A, yield, and resume on worker B, then fill or copy into the
//! existing capacity without allocating on B. [`refresh_if_behind`] runs at every poll and
//! blocking closure start (outside the lock) and refreshes when `MAX_SEEN_PAGES > LOCAL_PAGES`.
//!
//! # Invariant
//!
//! `LOCAL_PAGES` never exceeds the size V8 checks on this thread: it is only set from the return
//! value of `memory.grow(0)` run on this thread, and memory never shrinks. `MAX_SEEN_PAGES` is
//! the largest `LOCAL_PAGES` any thread has stored. dlmalloc only hands out memory below the
//! break, and the break never passes the `LOCAL_PAGES` of the thread that moved it, which
//! publishes it to `MAX_SEEN_PAGES` before it releases `LOCK`. So after taking `LOCK`, every
//! block dlmalloc returns lies within this thread's `LOCAL_PAGES`; [`after`] checks that and
//! counts a violation (`rolldown_heap_sync_stat(2)`, expected 0).
//!
//! # Remaining gaps
//!
//! - A block that reaches a running thread mid-poll (a channel message, an `Arc`, a
//!   threadsafe-function call on the JS thread) and is touched there before that thread's next
//!   allocation or poll boundary, when another thread grew the memory in between. Below the
//!   break's reserve nothing grows, so this needs a heap above about 960 MiB; not observed.
//! - A thread that crashes while it holds `LOCK` leaves the others spinning, as a crash inside
//!   dlmalloc's own lock always did.
//!
//! Remove this module once the Node versions we support ship the V8 fix
//! (v8/v8@34241014663390c72e08c123faef6fedf395be8e).
use std::{
  alloc::{GlobalAlloc, Layout},
  cell::Cell,
  ffi::c_void,
  sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
};

/// Wasm page size in bytes.
const PAGE: u64 = 65536;

/// Largest memory size (pages) any thread has refreshed to.
static MAX_SEEN_PAGES: AtomicUsize = AtomicUsize::new(0);

/// The allocator lock. Held around every call into dlmalloc; see [`locked`].
static LOCK: AtomicBool = AtomicBool::new(false);

/// dlmalloc's break, owned by [`__wrap_sbrk`]. 0 until the first `sbrk` call. Only read and
/// written under `LOCK` (and dlmalloc's own lock), so `Relaxed` is enough.
static BRK: AtomicUsize = AtomicUsize::new(0);

thread_local! {
  /// Memory size (pages) this thread's V8 bounds were last refreshed to.
  static LOCAL_PAGES: Cell<usize> = const { Cell::new(0) };
}

/// When the break must pass the current memory, grow at least this much in one `memory.grow`,
/// so the heap grows in a few large steps instead of 64 KiB-2 MiB ones. Every growth costs one
/// refresh on every thread, and V8 changes the page permissions of the whole memory on each
/// grow. 16 MiB is a small share of the 4 GiB maximum.
const GROW_AHEAD: usize = 16 << 20;

/// Counters read by tests through the `rolldown_heap_sync_stat` export. Each one is only
/// written on a cold path.
mod stat {
  /// `memory.grow(n > 0)` calls made by `__wrap_sbrk`.
  pub const GROWS: usize = 0;
  /// Refreshes run after taking `LOCK`.
  pub const LOCK_REFRESHES: usize = 1;
  /// Blocks that ended past this thread's refreshed size after dlmalloc returned them. The
  /// invariant says 0; [`super::after`] refreshes and counts one if it ever happens.
  pub const LATE_REFRESHES: usize = 2;
  /// The break, in pages (rounded up).
  pub const BREAK_PAGES: usize = 3;
  /// `__heap_end`, in pages: where the break starts.
  pub const HEAP_END_PAGES: usize = 4;
  pub const COUNT: usize = 5;
}

static STATS: [AtomicU32; stat::COUNT] = [const { AtomicU32::new(0) }; stat::COUNT];

#[inline]
fn bump(index: usize) {
  STATS[index].fetch_add(1, Ordering::Relaxed);
}

/// Test-only view of the heap-sync counters (see [`stat`]); `u32::MAX` for an unknown index.
/// Exported from the wasm module (a `#[no_mangle]` function in a cdylib), not through napi.
#[unsafe(no_mangle)]
pub extern "C" fn rolldown_heap_sync_stat(index: u32) -> u32 {
  STATS.get(index as usize).map_or(u32::MAX, |counter| counter.load(Ordering::Relaxed))
}

/// Whether a block that ends at byte `end` lies past a thread's refreshed size of `local_pages`.
///
/// `end` is `u64` because a block may end exactly at 4 GiB, and pages are compared as `u64`
/// because 65536 pages * 64 KiB overflows the 32-bit `usize`.
#[inline]
const fn ends_past(end: u64, local_pages: usize) -> bool {
  end > local_pages as u64 * PAGE
}

// The page math at the edges. Checked when this module compiles (threaded WASI only).
const _: () = {
  assert!(ends_past(1, 0));
  assert!(!ends_past(PAGE, 1));
  assert!(ends_past(PAGE + 1, 1));
  assert!(!ends_past(65536 * PAGE, 65536));
};

/// Run `memory.grow(0)` on this thread and record the size it returns.
#[inline]
fn grow_zero() -> usize {
  // `memory.grow(0)` returns the current size and makes V8 reload this thread's bounds.
  let now = core::arch::wasm32::memory_grow::<0>(0);
  LOCAL_PAGES.with(|c| c.set(now));
  MAX_SEEN_PAGES.fetch_max(now, Ordering::AcqRel);
  now
}

/// Refresh when another thread has seen a larger memory than this thread.
///
/// Registered with `rolldown_utils::async_runtime::set_thread_handoff_hook` at module init,
/// so it runs at the start of every task poll and blocking closure (where work migrates onto
/// this thread). Runs outside `LOCK`. Hot path: one thread-local read and one atomic load.
#[inline]
pub fn refresh_if_behind() {
  if MAX_SEEN_PAGES.load(Ordering::Acquire) > LOCAL_PAGES.with(Cell::get) {
    grow_zero();
  }
}

unsafe extern "C" {
  fn __real_malloc(size: usize) -> *mut c_void;
  fn __real_free(ptr: *mut c_void);
  fn __real_calloc(count: usize, size: usize) -> *mut c_void;
  fn __real_realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
  fn __real_posix_memalign(out: *mut *mut c_void, align: usize, size: usize) -> i32;
  fn __real_aligned_alloc(align: usize, size: usize) -> *mut c_void;
  fn __real_malloc_usable_size(ptr: *mut c_void) -> usize;
  fn __real___libc_malloc(size: usize) -> *mut c_void;
  fn __real___libc_free(ptr: *mut c_void);
  fn __real___libc_calloc(count: usize, size: usize) -> *mut c_void;
  fn sched_yield() -> i32;
  /// End of the module's own initial memory (wasm-ld), where dlmalloc's first segment ends.
  static __heap_end: u8;
}

#[inline]
fn lock() {
  if LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
    lock_slow();
  }
}

/// Spin like dlmalloc's own lock: `sched_yield` every 64 spins, never `memory.atomic.wait`
/// (which traps on a browser main thread).
#[cold]
fn lock_slow() {
  let mut spins: u32 = 0;
  loop {
    while LOCK.load(Ordering::Relaxed) {
      spins = spins.wrapping_add(1);
      if spins.is_multiple_of(64) {
        unsafe { sched_yield() };
      } else {
        core::hint::spin_loop();
      }
    }
    if LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
      return;
    }
  }
}

/// Take `LOCK`, catch up with every growth published before it, run `f` (one real dlmalloc
/// call), release. Not re-entrant: `f` must not call a `__wrap_*` symbol. dlmalloc's only calls
/// out of its object are `sbrk` (our [`__wrap_sbrk`], which does not lock) and `sched_yield`.
#[inline]
fn locked<R>(f: impl FnOnce() -> R) -> R {
  lock();
  let local = LOCAL_PAGES.with(Cell::get);
  // `local == 0`: this thread has never refreshed; do it once so `after` has a real bound.
  if local == 0 || MAX_SEEN_PAGES.load(Ordering::Acquire) > local {
    lock_refresh();
  }
  let result = f();
  LOCK.store(false, Ordering::Release);
  result
}

#[cold]
fn lock_refresh() {
  bump(stat::LOCK_REFRESHES);
  grow_zero();
}

/// Check the invariant on a block dlmalloc just returned (under `LOCK`): it must end within
/// this thread's refreshed size. If it ever does not, refresh and count it.
#[inline]
fn after(ptr: *mut c_void, size: usize) -> *mut c_void {
  if !ptr.is_null() && ends_past(ptr as usize as u64 + size as u64, LOCAL_PAGES.with(Cell::get)) {
    late_refresh();
  }
  ptr
}

#[cold]
fn late_refresh() {
  bump(stat::LATE_REFRESHES);
  grow_zero();
}

/// Wasm page size in bytes, for address math in `usize` (32 bits on wasm32).
const PAGE_BYTES: usize = 1 << 16;

/// Pages needed to cover every byte below `addr`.
#[inline]
const fn pages_below(addr: usize) -> usize {
  addr.div_ceil(PAGE_BYTES)
}

#[inline]
fn store_pages(index: usize, pages: usize) {
  STATS[index].store(u32::try_from(pages).unwrap_or(u32::MAX), Ordering::Relaxed);
}

/// dlmalloc's `MORECORE`. dlmalloc calls it only from inside its entry points, which run only
/// under `LOCK`, so this never takes the lock and is never entered twice at once.
///
/// It hands out `[__heap_end, memory.size)` before it grows: those pages exist on every thread
/// from instantiation, so they never need a refresh. When the break must pass the current
/// memory it grows by at least [`GROW_AHEAD`], then refreshes this thread and publishes the new
/// size before the caller releases `LOCK`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_sbrk(increment: isize) -> *mut c_void {
  const FAIL: *mut c_void = usize::MAX as *mut c_void;
  let mut brk = BRK.load(Ordering::Relaxed);
  if brk == 0 {
    // Page-aligned by wasm-ld (it is the end of the module's initial memory); rounding up
    // only matters if that ever changes, and dlmalloc copes with a non-contiguous break.
    let start = pages_below(&raw const __heap_end as usize) * PAGE_BYTES;
    if start == 0 {
      return FAIL;
    }
    store_pages(stat::HEAP_END_PAGES, start / PAGE_BYTES);
    brk = start;
    BRK.store(brk, Ordering::Relaxed);
  }
  if increment == 0 {
    return brk as *mut c_void;
  }
  // wasm memory cannot shrink. dlmalloc only asks for less on a failure path it ignores.
  let Ok(increment) = usize::try_from(increment) else { return FAIL };
  let Some(new_brk) = brk.checked_add(increment) else { return FAIL };
  let current_pages = grow_zero();
  let needed_pages = pages_below(new_brk);
  if needed_pages > current_pages {
    let need = needed_pages - current_pages;
    let ahead = need.max(GROW_AHEAD / PAGE_BYTES);
    if core::arch::wasm32::memory_grow::<0>(ahead) == usize::MAX
      && (ahead == need || core::arch::wasm32::memory_grow::<0>(need) == usize::MAX)
    {
      return FAIL;
    }
    bump(stat::GROWS);
    // Refresh this thread and publish the new size before the caller releases `LOCK`.
    grow_zero();
  }
  BRK.store(new_brk, Ordering::Relaxed);
  store_pages(stat::BREAK_PAGES, needed_pages);
  brk as *mut c_void
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_malloc(size: usize) -> *mut c_void {
  locked(|| after(unsafe { __real_malloc(size) }, size))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_free(ptr: *mut c_void) {
  if !ptr.is_null() {
    locked(|| unsafe { __real_free(ptr) });
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_calloc(count: usize, size: usize) -> *mut c_void {
  locked(|| after(unsafe { __real_calloc(count, size) }, count.saturating_mul(size)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
  locked(|| after(unsafe { __real_realloc(ptr, size) }, size))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_posix_memalign(
  out: *mut *mut c_void,
  align: usize,
  size: usize,
) -> i32 {
  locked(|| {
    let rc = unsafe { __real_posix_memalign(out, align, size) };
    if rc == 0 {
      after(unsafe { *out }, size);
    }
    rc
  })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_aligned_alloc(align: usize, size: usize) -> *mut c_void {
  locked(|| after(unsafe { __real_aligned_alloc(align, size) }, size))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_malloc_usable_size(ptr: *mut c_void) -> usize {
  // Reads the chunk header, which may sit in pages another thread grew.
  locked(|| unsafe { __real_malloc_usable_size(ptr) })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap___libc_malloc(size: usize) -> *mut c_void {
  locked(|| after(unsafe { __real___libc_malloc(size) }, size))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap___libc_free(ptr: *mut c_void) {
  if !ptr.is_null() {
    locked(|| unsafe { __real___libc_free(ptr) });
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap___libc_calloc(count: usize, size: usize) -> *mut c_void {
  locked(|| after(unsafe { __real___libc_calloc(count, size) }, count.saturating_mul(size)))
}

/// The module's `malloc` export for `@emnapi/core`. `--wrap=malloc` turns napi-build's
/// `--export=malloc` into `__wrap_malloc`; scripts/wasi/rename-wasm-allocator-exports.mjs adds
/// the `malloc` export for this function after the link, and
/// scripts/wasi/check-wasi-dist-files.mjs checks that `malloc` and this name share one index.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rolldown_heap_sync_malloc(size: usize) -> *mut c_void {
  locked(|| after(unsafe { __real_malloc(size) }, size))
}

/// The module's `free` export for `@emnapi/core`; see [`rolldown_heap_sync_malloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rolldown_heap_sync_free(ptr: *mut c_void) {
  if !ptr.is_null() {
    locked(|| unsafe { __real_free(ptr) });
  }
}

/// Rust's global allocator: the real dlmalloc entry points under `LOCK`, chosen like std's
/// `System` on wasm32 (malloc / calloc / realloc when the alignment is at most 8 and at most the
/// size, posix_memalign otherwise).
pub struct HeapSyncAlloc;

/// std's `MIN_ALIGN` on wasm32.
const MIN_ALIGN: usize = 8;

/// posix_memalign under `LOCK`; null on failure.
#[inline]
fn aligned_alloc_locked(layout: Layout) -> *mut u8 {
  let mut out = std::ptr::null_mut();
  // posix_memalign needs a multiple of the pointer size.
  let align = layout.align().max(size_of::<usize>());
  let rc = locked(|| {
    let rc = unsafe { __real_posix_memalign(&raw mut out, align, layout.size()) };
    if rc == 0 {
      after(out, layout.size());
    }
    rc
  });
  if rc == 0 { out.cast() } else { std::ptr::null_mut() }
}

unsafe impl GlobalAlloc for HeapSyncAlloc {
  #[inline]
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
      locked(|| after(unsafe { __real_malloc(layout.size()) }, layout.size())).cast()
    } else {
      aligned_alloc_locked(layout)
    }
  }

  #[inline]
  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
      locked(|| after(unsafe { __real_calloc(layout.size(), 1) }, layout.size())).cast()
    } else {
      let ptr = aligned_alloc_locked(layout);
      if !ptr.is_null() {
        // Within this thread's refreshed size: `after` checked it under `LOCK`.
        unsafe { ptr.write_bytes(0, layout.size()) };
      }
      ptr
    }
  }

  #[inline]
  unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
    locked(|| unsafe { __real_free(ptr.cast()) });
  }

  #[inline]
  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    if layout.align() <= MIN_ALIGN && layout.align() <= new_size {
      locked(|| after(unsafe { __real_realloc(ptr.cast(), new_size) }, new_size)).cast()
    } else {
      // std's `realloc_fallback`: allocate, copy, free. The copy runs outside `LOCK`; both
      // blocks lie within this thread's refreshed size (the new one was checked by `after`,
      // and the old one was handed out before this thread's last refresh under `LOCK`).
      let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
      let new_ptr = aligned_alloc_locked(new_layout);
      if !new_ptr.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size)) };
        locked(|| unsafe { __real_free(ptr.cast()) });
      }
      new_ptr
    }
  }
}
