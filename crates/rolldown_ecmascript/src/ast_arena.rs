//! The allocator for the arenas of module ASTs.
//!
//! An embedder (for example `rolldown_binding`) can register a chunk allocator with
//! [`set_ast_chunk_allocator`]. After that, the arenas of module ASTs get their memory from it,
//! instead of from the global allocator. This lets the embedder keep the AST memory in a separate heap.
//!
//! If no chunk allocator is registered (wasm, tests, and other embedders), the arenas use the global
//! allocator, as before.

use std::{alloc::GlobalAlloc, sync::OnceLock};

use oxc::allocator::Allocator;

static AST_CHUNK_ALLOC: OnceLock<&'static (dyn GlobalAlloc + Sync)> = OnceLock::new();

/// Register the allocator that the arenas of module ASTs get their chunks from.
///
/// Only the first call has an effect. Call this before the first module is parsed.
/// Arenas that were created before the call keep using the global allocator.
pub fn set_ast_chunk_allocator(chunk_alloc: &'static (dyn GlobalAlloc + Sync)) {
  let _ = AST_CHUNK_ALLOC.set(chunk_alloc);
}

/// Create an arena for a module AST.
///
/// If `capacity` is 0, no memory is allocated until the first allocation into the arena.
pub fn ast_allocator(capacity: usize) -> Allocator {
  match AST_CHUNK_ALLOC.get() {
    Some(&chunk_alloc) => Allocator::with_capacity_in(capacity, chunk_alloc),
    None => Allocator::with_capacity(capacity),
  }
}
