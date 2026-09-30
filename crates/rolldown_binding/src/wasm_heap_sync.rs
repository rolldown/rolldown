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
//! next interrupt check. A thread in a long wasm activation checks late (baseline Liftoff code in
//! a call-free loop only when its tier-up budget runs out or at a function entry; TurboFan code
//! once per loop pass), so it keeps its old size, and `memory.fill`, `memory.copy` and atomics
//! are bounds-checked against it. So a thread that gets a block from the new pages and fills or
//! copies it (memset, memcpy, calloc, realloc) traps with "memory access out of bounds". Plain
//! loads and stores are not affected: on hosts with the wasm trap handler (default Node on x64
//! and on most arm64 hosts, see Remaining gaps) they are checked by guard pages against the real
//! size.
//!
//! # The workaround
//!
//! `memory.grow(0)` on the stale thread makes V8 update the size for that thread and returns
//! it. `memory.size` does not help: it returns the same stale size and updates nothing
//! (measured: a `memory.size` refresh fails 10/10 in release, and in the two-worker probe it
//! read one page short in every trap). `scripts/wasi/check-v8-shared-memory-grow.mjs`
//! (`pnpm check:v8-shared-memory-grow`) shows whether a Node still has the bug. So after each
//! allocation we run `memory.grow(0)` when the block ends past what this thread last saw, or
//! when any thread has seen a larger memory.
//!
//! Rust allocations go through [`HeapSyncAlloc`] (the `#[global_allocator]`). `build.rs` links
//! with `--wrap` for calloc, realloc, aligned_alloc and posix_memalign, so C callers (emnapi,
//! wasi-libc) and std's `System` allocator come here too. `malloc` itself is not wrapped (see
//! `build.rs`). Each path takes the block from dlmalloc (which only writes chunk headers with
//! plain stores), refreshes, and only then lets memset or memcpy touch the block: calloc zeroes
//! and realloc copies here, after the refresh, instead of inside libc.
//!
//! # Scheduler handoff
//!
//! The allocator refresh covers blocks this thread allocates. A task can allocate on worker A,
//! yield, and resume on worker B, then fill or copy into the existing capacity without
//! allocating on B. [`refresh_if_behind`] closes that: it runs at every poll and blocking
//! closure start and refreshes when `MAX_SEEN_PAGES > LOCAL_PAGES`. The scheduler queue orders
//! A's allocation (and its `MAX_SEEN_PAGES` update) before B's poll, so B's Acquire load sees it.
//!
//! # Invariant
//!
//! `LOCAL_PAGES` never exceeds the size V8 checks on this thread: it is only set from the return
//! value of `memory.grow(0)` run on this thread, which made V8 reload at least that size, and
//! memory never shrinks. `MAX_SEEN_PAGES` is the largest `LOCAL_PAGES` any thread has stored.
//! Every block handed out here ends within its allocating thread's `LOCAL_PAGES`, so it ends
//! within `MAX_SEEN_PAGES` too. After any allocation through these paths, this thread covers
//! every block allocated through them that happens-before it.
//!
//! # Remaining gaps
//!
//! - Work that moves to another thread through the scheduler is covered: [`refresh_if_behind`]
//!   runs at the start of every task poll and blocking closure (the hook in
//!   `rolldown_utils::async_runtime`, registered in `async_runtime.rs` at module init). So a
//!   task that allocated on worker A and resumes on worker B refreshes on B before it touches
//!   that capacity. What is left: a block that reaches a running thread mid-poll (a channel
//!   message, an `Arc`, a threadsafe-function call on the JS thread) and is memset / memcpy'd /
//!   touched with atomics there before that thread's next allocation or poll boundary. The
//!   grow-ahead below makes growth rare (about 14 growth events per run instead of about 3000),
//!   which shrinks this window. Not observed in the measured runs; the V8 fix closes it.
//! - C code that calls `malloc` directly and then fills the block is not covered, because
//!   `malloc` is not wrapped.
//! - Hosts that run without the V8 wasm trap handler bounds-check plain stores against the cached
//!   size too. V8 13.6 (Node 24) builds the handler only for x64 and arm64 on Linux, Windows and
//!   macOS (plus x64 FreeBSD, loong64, riscv64); Node 22 lacks it on Windows arm64 and Node 20
//!   on every arm64 host but macOS. Node turns it off with `--disable-wasm-trap-handler` and
//!   exposes no runtime check for it. Measured with Node's
//!   `--wasm-enforce-bounds-checks` (hosts without the handler should behave the same, not
//!   measured): dlmalloc's own chunk-header store into pages another thread grew traps inside
//!   dlmalloc (reached through `posix_memalign` or `malloc`), before any refresh and while it
//!   holds the dlmalloc lock. When the trap does not end the process, the lock is never
//!   released, the main thread and the other workers spin on `sched_yield` (about 400% CPU), and
//!   the process HANGS. The loader's worker-crash latch cannot help: it runs on the main thread's
//!   event loop, which never gets control back. Measured on direct MultiThread (4 workers)
//!   bundle runs: the release-wasi artifact fails 110/110 (76 hang, 34 trap), the dev-profile
//!   artifact 7/110 (1/10 in a control); without the flag 10/10 release runs pass. `free` and the
//!   unwrapped `malloc` that emnapi calls from JS write chunk headers too. Closing this needs a
//!   refresh after dlmalloc takes its lock (a dlmalloc or `sbrk` hook, or an allocator that owns
//!   the lock); a refresh in these wrappers before the allocation does not (see design.md).
//!
//! Remove this module once the Node versions we support ship the V8 fix
//! (v8/v8@34241014663390c72e08c123faef6fedf395be8e).
use std::{
  alloc::{GlobalAlloc, Layout, System},
  cell::Cell,
  ffi::c_void,
  sync::atomic::{AtomicUsize, Ordering},
};

/// Wasm page size in bytes.
const PAGE: u64 = 65536;

/// Largest memory size (pages) any thread has refreshed to.
static MAX_SEEN_PAGES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
  /// Memory size (pages) this thread's V8 bounds were last refreshed to.
  static LOCAL_PAGES: Cell<usize> = const { Cell::new(0) };
}

/// When a thread is the first to see the heap grow, it extends dlmalloc's top by this much in
/// one `memory.grow` (malloc then free), so the heap grows in a few large steps instead of about
/// one page at a time. Every growth event costs one refresh on every thread, so fewer events
/// mean fewer refreshes (measured: 3015 -> 14 per CurrentThread run, 11600 -> 60 MultiThread).
/// 16 MiB is a small share of the 4 GiB maximum and of the 1 GiB initial threaded memory.
const GROW_AHEAD: usize = 16 << 20;

/// Whether this thread must refresh before touching a block that ends at byte `end`.
///
/// `end` is `u64` because a block may end exactly at 4 GiB, and pages are compared as `u64`
/// because 65536 pages * 64 KiB overflows the 32-bit `usize`.
#[inline]
const fn needs_refresh(end: u64, local_pages: usize, max_seen_pages: usize) -> bool {
  end > local_pages as u64 * PAGE || max_seen_pages > local_pages
}

// The page math at the edges. Checked when this module compiles (threaded WASI only).
const _: () = {
  assert!(needs_refresh(1, 0, 0));
  assert!(!needs_refresh(PAGE, 1, 1));
  assert!(needs_refresh(PAGE + 1, 1, 1));
  assert!(needs_refresh(PAGE, 1, 2));
  assert!(!needs_refresh(65536 * PAGE, 65536, 65536));
};

#[inline]
fn block_end(ptr: *mut c_void, size: usize) -> u64 {
  ptr as usize as u64 + size as u64
}

#[inline]
fn refresh_if_stale(end: u64) {
  let local = LOCAL_PAGES.with(Cell::get);
  if needs_refresh(end, local, MAX_SEEN_PAGES.load(Ordering::Acquire)) {
    refresh();
  }
}

/// Refresh when another thread has seen a larger memory than this thread.
///
/// Registered with `rolldown_utils::async_runtime::set_thread_handoff_hook` at module init,
/// so it runs at the start of every task poll and blocking closure (where work migrates onto
/// this thread). Hot path: one thread-local read and one atomic load.
#[inline]
pub fn refresh_if_behind() {
  let local = LOCAL_PAGES.with(Cell::get);
  if MAX_SEEN_PAGES.load(Ordering::Acquire) > local {
    refresh();
  }
}

/// Run `memory.grow(0)` on this thread and record the size it returns.
/// Returns `(now, previous MAX_SEEN_PAGES)`.
#[inline]
fn grow_zero() -> (usize, usize) {
  // `memory.grow(0)` returns the current size and makes V8 reload this thread's bounds.
  let now = core::arch::wasm32::memory_grow::<0>(0);
  LOCAL_PAGES.with(|c| c.set(now));
  (now, MAX_SEEN_PAGES.fetch_max(now, Ordering::AcqRel))
}

#[cold]
fn refresh() {
  let (now, prev) = grow_zero();
  if now > prev && prev != 0 {
    // First thread to see this growth: grow ahead once. malloc is not wrapped, so this does
    // not re-enter; the block comes from the top and merges back into it on free.
    unsafe {
      let p = malloc(GROW_AHEAD);
      if !p.is_null() {
        free(p);
      }
    }
    grow_zero();
  }
}

unsafe extern "C" {
  fn malloc(size: usize) -> *mut c_void;
  fn __real_aligned_alloc(align: usize, size: usize) -> *mut c_void;
  fn __real_posix_memalign(out: *mut *mut c_void, align: usize, size: usize) -> i32;
  fn free(ptr: *mut c_void);
  fn malloc_usable_size(ptr: *mut c_void) -> usize;
}

#[inline]
fn after(ptr: *mut c_void, size: usize) -> *mut c_void {
  if !ptr.is_null() {
    refresh_if_stale(block_end(ptr, size));
  }
  ptr
}

#[inline]
unsafe fn synced_malloc(size: usize) -> *mut c_void {
  after(unsafe { malloc(size) }, size)
}

pub struct HeapSyncAlloc;

unsafe impl GlobalAlloc for HeapSyncAlloc {
  #[inline]
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    // `System.alloc` calls `malloc` (not wrapped) or `posix_memalign` (wrapped).
    after(unsafe { System.alloc(layout) }.cast(), layout.size()).cast()
  }

  #[inline]
  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    // Zero after the refresh in `alloc`, not inside libc calloc.
    let ptr = unsafe { self.alloc(layout) };
    if !ptr.is_null() {
      unsafe { ptr.write_bytes(0, layout.size()) };
    }
    ptr
  }

  #[inline]
  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    unsafe { System.dealloc(ptr, layout) }
  }

  #[inline]
  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    // std's `System.realloc` calls libc `realloc` (wrapped below) or allocates the new block
    // with `posix_memalign` (wrapped) before it copies.
    unsafe { System.realloc(ptr, layout, new_size) }
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_aligned_alloc(align: usize, size: usize) -> *mut c_void {
  after(unsafe { __real_aligned_alloc(align, size) }, size)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_posix_memalign(
  out: *mut *mut c_void,
  align: usize,
  size: usize,
) -> i32 {
  let rc = unsafe { __real_posix_memalign(out, align, size) };
  if rc == 0 {
    after(unsafe { *out }, size);
  }
  rc
}

/// Replaces libc calloc: its memset would run before any refresh.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_calloc(count: usize, size: usize) -> *mut c_void {
  let Some(bytes) = count.checked_mul(size) else { return std::ptr::null_mut() };
  let ptr = unsafe { synced_malloc(bytes) };
  if !ptr.is_null() {
    unsafe { ptr.cast::<u8>().write_bytes(0, bytes) };
  }
  ptr
}

/// Replaces libc realloc: its memcpy would run before any refresh.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
  if ptr.is_null() {
    return unsafe { synced_malloc(size) };
  }
  if size == 0 {
    unsafe { free(ptr) };
    return std::ptr::null_mut();
  }
  let old = unsafe { malloc_usable_size(ptr) };
  if old >= size {
    // Kept in place. The block may sit in pages another thread grew, and the caller is about
    // to write into it, so cover it on this thread first.
    refresh_if_stale(block_end(ptr, size));
    return ptr;
  }
  // The old block may sit in pages another thread grew; refresh before copying out of it.
  refresh_if_stale(block_end(ptr, old));
  let new_ptr = unsafe { synced_malloc(size) };
  if !new_ptr.is_null() {
    unsafe {
      std::ptr::copy_nonoverlapping(ptr.cast::<u8>(), new_ptr.cast::<u8>(), old);
      free(ptr);
    }
  }
  new_ptr
}
