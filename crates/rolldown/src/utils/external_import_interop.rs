use oxc_index::IndexVec;
use rolldown_common::{
  ChunkIdx, ExternalInteropUse, ImportRecordIdx, ModuleIdx, NamedImport, NormalModule,
  PostChunkOptimizationOperation, Specifier, SymbolRef,
};
use rustc_hash::FxHashMap;

use crate::{chunk_graph::ChunkGraph, stages::link_stage::LinkStageOutput};

/// Check if a specific import specifier needs the `__toESM` helper.
/// Only namespace imports (`import * as foo`) and default imports (`import foo`)
/// need the `__toESM` helper. Named imports (`import { foo }`) do not need it.
pub fn specifier_needs_interop(specifier: &Specifier) -> bool {
  matches!(specifier, Specifier::Star)
    || matches!(specifier, Specifier::Literal(name) if name.as_str() == "default")
}

/// Check if the named imports from an external module need the `__toESM` helper.
pub fn external_import_needs_interop(
  named_imports: &[(rolldown_common::ModuleIdx, NamedImport)],
) -> bool {
  named_imports.iter().any(|(_, import)| specifier_needs_interop(&import.imported))
}

/// Check if an import record from a module needs the `__toESM` helper.
/// Only namespace imports (`import * as foo`) and default imports (`import foo`)
/// need the `__toESM` helper. Named imports (`import { foo }`) do not need it.
pub fn import_record_needs_interop(module: &NormalModule, rec_idx: ImportRecordIdx) -> bool {
  module
    .named_imports
    .values()
    .any(|import| import.record_idx == rec_idx && specifier_needs_interop(&import.imported))
}

/// Interop the inclusion pass recorded for an external module, attributed to one chunk.
///
/// See internal-docs/runtime-helpers/implementation.md.
///
/// [`external_import_needs_interop`] only sees the static imports written by modules that live in
/// the chunk being rendered. That misses every reference whose importing module was tree-shaken
/// away — a re-export shim (`import d from 'external'; export { d }`) is dropped as soon as the
/// external is side-effect free, yet the `<external_ns>.default` it produced still needs the
/// wrapper (issue #10069). Callers OR this into their own `named_imports`-derived answer.
///
/// A recorded observation only forces *this* chunk to wrap when the module that made it lands here.
/// A chunk that merely holds a named import of the same external reads the CommonJS object directly
/// and must be left alone: reading a name through the wrapper does yield the same value
/// (`__copyProps` installs forwarding getters), but `__toESM` returns a fresh object
/// (`__create(__getProtoOf(mod))`) and eagerly walks
/// `__getOwnPropertyNames`/`__getOwnPropertyDescriptor`. Wrapping a named-only chunk would change
/// namespace identity, turn data properties into accessors, and — for a CommonJS export implemented
/// as a Proxy — fire extra `ownKeys`/`getOwnPropertyDescriptor` traps at require time.
///
/// Observers that ended up without a live chunk cannot be attributed, so they fall back to wrapping
/// every chunk that emits the external. That case is real: the observation can be recorded against
/// a module tree-shaking later drops. Narrowing it away would under-approximate, which is exactly
/// what produced #10069 — a silently wrong bundle — so over-wrapping stays the fallback.
pub fn chunk_recorded_external_interop(
  link_output: &LinkStageOutput,
  assignments: ChunkAssignments<'_>,
  chunk_idx: ChunkIdx,
  external_namespace_ref: SymbolRef,
) -> Option<ExternalInteropUse> {
  let canonical_ref = link_output.symbol_db.canonical_ref_for(external_namespace_ref);
  let use_ = link_output.used_external_symbols.interop_use(&canonical_ref)?;
  let Some(observers) = link_output.used_external_symbols.interop_observers(&canonical_ref) else {
    return Some(use_);
  };
  if observers.iter().any(|observer| assignments.live_chunk_of(*observer).is_none()) {
    return Some(use_);
  }
  observers
    .iter()
    .any(|observer| assignments.live_chunk_of(*observer) == Some(chunk_idx))
    .then_some(use_)
}

/// Where modules ended up after chunking, as far as attributing an external observation needs it.
///
/// Deconflicting runs inside a `par_iter_mut` over the chunk table and so cannot hold a
/// `&ChunkGraph`; it borrows the two relevant fields directly instead.
#[derive(Clone, Copy)]
pub struct ChunkAssignments<'a> {
  module_to_chunk: &'a IndexVec<ModuleIdx, Option<ChunkIdx>>,
  chunk_operations: &'a FxHashMap<ChunkIdx, PostChunkOptimizationOperation>,
}

impl<'a> ChunkAssignments<'a> {
  pub fn new(
    module_to_chunk: &'a IndexVec<ModuleIdx, Option<ChunkIdx>>,
    chunk_operations: &'a FxHashMap<ChunkIdx, PostChunkOptimizationOperation>,
  ) -> Self {
    Self { module_to_chunk, chunk_operations }
  }

  pub fn from_graph(chunk_graph: &'a ChunkGraph) -> Self {
    Self::new(&chunk_graph.module_to_chunk, &chunk_graph.post_chunk_optimization_operations)
  }

  /// Mirrors [`ChunkGraph::chunk_is_live`]: order-wrap lowering can remove a chunk after modules
  /// were assigned to it, and a removed chunk cannot carry the wrapper.
  fn live_chunk_of(&self, module_idx: ModuleIdx) -> Option<ChunkIdx> {
    self.module_to_chunk.get(module_idx).copied().flatten().filter(|chunk_idx| {
      self.chunk_operations.get(chunk_idx) != Some(&PostChunkOptimizationOperation::Removed)
    })
  }
}
