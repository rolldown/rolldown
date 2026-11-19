//! A separate mimalloc heap for the arenas of module ASTs.
//!
//! Module ASTs are large, and they live from the parse until the end of the
//! build. Their arena chunks are put in their own mimalloc heap, so that they
//! do not share mimalloc pages with the many short-lived allocations of the
//! rest of the build. This lowers fragmentation and peak RSS.
//!
//! This needs mimalloc v3: a v3 heap can allocate and free from any thread.
//! (A v2 heap belongs to the thread that created it.)

use std::{
  alloc::{GlobalAlloc, Layout},
  ffi::c_void,
  sync::OnceLock,
};

use libmimalloc_sys2::{mi_free, mi_heap_malloc_aligned, mi_heap_new, mi_heap_t};

/// A mimalloc heap which is never deleted.
struct AstHeap(*mut mi_heap_t);

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
