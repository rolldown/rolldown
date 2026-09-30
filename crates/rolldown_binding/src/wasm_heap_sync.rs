//! Threaded WASI only: keep every thread's view of the shared memory size current.
//!
//! See internal-docs/wasi-shared-memory-grow/design.md for the evidence and the rejected
//! alternatives, and internal-docs/wasi-shared-memory-grow/implementation.md for the wiring.
//!
//! # What V8 does
//!
//! All threads of the `wasm32-wasip1-threads` build share one wasm memory, and wasi-libc's
//! dlmalloc grows it with `memory.grow` (in `sbrk`) from whichever thread runs out of heap. V8
//! refreshes the memory size cached for the running activation on that thread only. Optimized
//! code on every other thread keeps its old size, and `memory.fill`, `memory.copy` and atomics
//! are bounds-checked against it. So a thread that gets a block from the new pages and fills or
//! copies it (memset, memcpy, calloc, realloc) traps with "memory access out of bounds". Plain
//! loads and stores are not affected: on hosts with the wasm trap handler (default Node on
//! arm64 and x64) they are checked by guard pages against the real size.
//!
//! # The workaround
//!
//! `memory.grow(0)` on the stale thread makes V8 reload the size for that thread and returns
//! it. `memory.size` does not help: it returns the new size but leaves the cached bounds alone
//! (measured: a `memory.size` refresh fails 10/10 in release). So after each allocation we run
//! `memory.grow(0)` when the block ends past what this thread last saw, or when any thread has
//! seen a larger memory.
//!
//! Rust allocations go through [`HeapSyncAlloc`] (the `#[global_allocator]`). `build.rs` links
//! with `--wrap` for calloc, realloc, aligned_alloc and posix_memalign, so C callers (emnapi,
//! wasi-libc) and std's `System` allocator come here too. `malloc` itself is not wrapped (see
//! `build.rs`). Each path takes the block from dlmalloc (which only writes chunk headers with
//! plain stores), refreshes, and only then lets memset or memcpy touch the block: calloc zeroes
//! and realloc copies here, after the refresh, instead of inside libc.
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
//! - A block handed to another thread that then runs memcpy / memset / atomics on it before its
//!   own next allocation still sees a stale size. The grow-ahead below makes growth rare (about
//!   14 growth events per run instead of about 3000), which shrinks this window. Not observed in
//!   the measured runs.
//! - C code that calls `malloc` directly and then fills the block is not covered, because
//!   `malloc` is not wrapped.
//! - Hosts that run without the trap handler (`--wasm-enforce-bounds-checks`, some 32-bit
//!   hosts) bounds-check plain stores against the cached size too, so dlmalloc's own header
//!   writes can trap before any refresh (measured 1/6 with the flag).
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
