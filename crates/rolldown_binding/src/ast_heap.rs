//! A separate mimalloc heap for the arenas of module ASTs.
//!
//! Module ASTs are large, and they live from the parse until the end of the
//! build. Their arena chunks are put in their own mimalloc heap, so that they
//! do not share mimalloc pages with the many short-lived allocations of the
//! rest of the build. This lowers fragmentation and peak RSS.
//!
//! This needs mimalloc v3: a v3 heap can allocate and free from any thread.
//! (A v2 heap belongs to the thread that created it.)
//!
//! A thread which allocates from the heap gets its own thread-local part of
//! the heap (a "theap"). Call [`release_current_thread`] before such a thread
//! exits. Otherwise mimalloc 3.7.0 leaks the theap on macOS.
//!
//! When rolldown is idle, `memory_reclaim` gives up the pages of each thread's
//! theap of this heap, so that freed AST memory goes back to the OS too.

use std::{
  alloc::{GlobalAlloc, Layout},
  cell::Cell,
  ffi::c_void,
  marker::{PhantomData, PhantomPinned},
  ptr::NonNull,
  sync::OnceLock,
};

use libmimalloc_sys2::{mi_free, mi_heap_malloc_aligned, mi_heap_new, mi_heap_t};

/// Opaque `mi_theap_t` from `mimalloc.h`: the thread-local part of a mimalloc heap.
#[repr(C)]
pub struct MiTheap {
  _data: (),
  _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

// `libmimalloc-sys2` 0.1.67 (mimalloc 3.7.0) has no bindings for these functions.
// Declarations from `mimalloc.h:398` and `mimalloc.h:441` of mimalloc 3.7.0.
unsafe extern "C" {
  /// Get the calling thread's theap of `heap`. Creates the theap if the thread
  /// has none yet.
  fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut MiTheap;
  /// Owner thread only: abandon the pages of `theap` and delete it.
  /// The heap and the live blocks in it are kept.
  fn mi_theap_delete(theap: *mut MiTheap);
}

/// A mimalloc heap which is never deleted.
struct AstHeap(*mut mi_heap_t);

thread_local! {
  /// Whether the calling thread has a theap of the AST heap.
  ///
  /// mimalloc creates the theap when the thread first allocates from the heap.
  /// A free does not create a theap.
  static HAS_THEAP: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: A mimalloc v3 heap can be used to allocate from any thread, and the
// memory can be freed from any thread ("Heaps: first-class. Can allocate from
// any thread (and be free'd from any thread)" in `mimalloc.h`). The heap is
// never deleted, so the pointer stays valid for the life of the process.
unsafe impl Send for AstHeap {}
// SAFETY: See `Send` above. `AstHeap` has no other state.
unsafe impl Sync for AstHeap {}

// SAFETY: `alloc` returns null or a block of at least `layout.size()` bytes
// aligned to `layout.align()`, from `mi_heap_malloc_aligned`. `dealloc` frees
// the block with `mi_free`, which accepts a block from any mimalloc heap.
unsafe impl GlobalAlloc for AstHeap {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    // `mi_heap_malloc_aligned` creates a theap if the calling thread has none.
    HAS_THEAP.set(true);
    // SAFETY: `self.0` is a live heap that was returned by `mi_heap_new`.
    let ptr = unsafe { mi_heap_malloc_aligned(self.0, layout.size(), layout.align()) }.cast::<u8>();
    #[cfg(feature = "tracking_allocator")]
    if !ptr.is_null() {
      rolldown_tracking_allocator::record_alloc(layout.size());
    }
    ptr
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    // SAFETY: The caller guarantees that `ptr` was returned by `alloc` above,
    // so it was allocated by mimalloc.
    unsafe { mi_free(ptr.cast::<c_void>()) };
    #[cfg(feature = "tracking_allocator")]
    rolldown_tracking_allocator::record_dealloc(layout.size());
    #[cfg(not(feature = "tracking_allocator"))]
    let _ = layout;
  }
}

static AST_HEAP: OnceLock<AstHeap> = OnceLock::new();

/// Get the calling thread's theap of the AST heap, or `None` if the thread has
/// not allocated from the AST heap.
///
/// Unlike `mi_heap_theap`, this does not create a theap. `memory_reclaim`
/// calls this on every thread that it reaches, and many of them never allocate
/// from the AST heap.
pub fn current_thread_theap() -> Option<NonNull<MiTheap>> {
  if !HAS_THEAP.get() {
    return None;
  }
  let ast_heap = AST_HEAP.get()?;
  // SAFETY: `ast_heap.0` is a live heap that was returned by `mi_heap_new`.
  // The AST heap is never deleted, so it is always still live here.
  // (`mi_heap_theap` must not be called after `mi_heap_delete`.)
  // The calling thread has a theap of the heap, so `mi_heap_theap` returns it
  // and does not create one.
  NonNull::new(unsafe { mi_heap_theap(ast_heap.0) })
}

/// Delete the calling thread's theap of the AST heap.
///
/// Call this on every thread which can allocate from the AST heap, just
/// before the thread exits. Memory which the thread allocated stays valid.
///
/// Without this, mimalloc 3.7.0 on macOS leaks the theap when the thread exits
/// (and fails an assertion in debug builds). `libmimalloc-sys2` builds mimalloc
/// with `MI_TLS_MODEL=LOCAL` on macOS. dyld frees the `__thread` storage before
/// mimalloc's thread exit handler runs, so the handler does not release the
/// reference that its thread-local cache holds on the theap.
pub fn release_current_thread() {
  let Some(theap) = current_thread_theap() else {
    return;
  };
  // SAFETY: `current_thread_theap` returns the calling thread's theap, so
  // `mi_theap_delete` runs on the owner thread, as it requires.
  unsafe { mi_theap_delete(theap.as_ptr()) };
  HAS_THEAP.set(false);
}

/// Create the AST heap, and register it as the chunk allocator for the arenas
/// of module ASTs.
///
/// If mimalloc cannot create the heap, the arenas use the global allocator.
pub fn init() {
  if AST_HEAP.get().is_some() {
    return;
  }
  // SAFETY: `mi_heap_new` has no preconditions.
  let heap = unsafe { mi_heap_new() };
  if heap.is_null() {
    return;
  }
  let ast_heap = AST_HEAP.get_or_init(|| AstHeap(heap));
  rolldown_ecmascript::set_ast_chunk_allocator(ast_heap);
}
