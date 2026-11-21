//! Returning memory to the OS after a bulk free.
//!
//! A build frees most of its memory in a few large drops: the per-module ASTs after codegen and
//! the link stage output after `generate()`. Those blocks were allocated by rayon workers and are
//! often freed from another thread. mimalloc puts a block freed from another thread on its
//! page's deferred-free list and only processes that list when the owning thread allocates or
//! collects again. Rayon workers sit idle while the JavaScript output hooks run, so pages that
//! are already empty stay committed until the build ends, and peak memory stays at its
//! high-water mark.
//!
//! Rolldown does not know which global allocator its embedder installed, so the embedder
//! registers the collect function here (the Node.js binding registers mimalloc's
//! `mi_collect`). Without a registered hook, [`collect_after_bulk_free`] does nothing.

use std::sync::OnceLock;

static COLLECT_HOOK: OnceLock<fn()> = OnceLock::new();

/// Registers the function that returns the calling thread's freed memory to the OS. Only the
/// first registration takes effect. The hook runs on every rayon worker, so it must be cheap
/// and must not block.
pub fn set_collect_hook(collect: fn()) {
  let _ = COLLECT_HOOK.set(collect);
}

/// Runs the registered collect hook on the calling thread and schedules it once on every
/// worker of the current rayon pool. Call it right after a large drop.
///
/// The rayon jobs are only scheduled (`spawn_broadcast`), never awaited: a blocking
/// `broadcast` from the deferred-drop worker could deadlock against a rayon worker that waits
/// in `defer_drop::drain()`.
pub fn collect_after_bulk_free() {
  let Some(&collect) = COLLECT_HOOK.get() else {
    return;
  };
  #[cfg(not(target_family = "wasm"))]
  rayon::spawn_broadcast(move |_| collect());
  collect();
}
