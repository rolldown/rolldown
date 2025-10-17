use std::cmp::Ordering;

use arcstr::ArcStr;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  Chunk, ChunkIdx, ChunkModulesOrderBy, ChunkTable, EcmaViewMeta, Module, ModuleIdx, RuntimeHelper,
  SymbolRef,
};
use rustc_hash::FxHashMap;

use crate::{SharedOptions, stages::link_stage::LinkStageOutput};

/// Represents the sort priority for modules within a chunk.
///
/// Sort order (lowest to highest):
/// 1. Runtime modules (rolldown:*) - sorted by exec_order
/// 2. Side-effects-free leaf modules - sorted by id
/// 3. Normal modules - sorted by exec_order
#[derive(Debug, PartialEq, Eq)]
enum ModuleSortKey<'a> {
  /// Runtime modules (rolldown:*) sorted by execution order
  Runtime(u32),
  /// Side-effects-free leaf modules sorted by id
  SideEffectsFree(&'a str),
  /// Normal modules sorted by execution order
  Normal(u32),
}

impl Ord for ModuleSortKey<'_> {
  fn cmp(&self, other: &Self) -> Ordering {
    match (self, other) {
      (ModuleSortKey::Runtime(a), ModuleSortKey::Runtime(b))
      | (ModuleSortKey::Normal(a), ModuleSortKey::Normal(b)) => a.cmp(b),
      (ModuleSortKey::Runtime(_), _)
      | (ModuleSortKey::SideEffectsFree(_), ModuleSortKey::Normal(_)) => Ordering::Less,
      (_, ModuleSortKey::Runtime(_))
      | (ModuleSortKey::Normal(_), ModuleSortKey::SideEffectsFree(_)) => Ordering::Greater,

      (ModuleSortKey::SideEffectsFree(a), ModuleSortKey::SideEffectsFree(b)) => a.cmp(b),
    }
  }
}

impl PartialOrd for ModuleSortKey<'_> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

/// Creates a sort key for a module based on its type and properties.
fn module_sort_key(module: &Module) -> ModuleSortKey<'_> {
  if module.id().starts_with("rolldown:") {
    return ModuleSortKey::Runtime(module.exec_order());
  }

  if let Some(normal) = module.as_normal() {
    let is_side_effects_free = normal.import_records.is_empty()
      && !normal.meta.contains(EcmaViewMeta::ExecutionOrderSensitive);

    if is_side_effects_free {
      return ModuleSortKey::SideEffectsFree(module.id());
    }
  }

  ModuleSortKey::Normal(module.exec_order())
}

#[derive(Debug)]
pub struct ChunkGraph {
  pub chunk_table: ChunkTable,
  pub sorted_chunk_idx_vec: Vec<ChunkIdx>,
  /// Module to chunk that contains the module
  pub module_to_chunk: IndexVec<ModuleIdx, Option<ChunkIdx>>,
  pub entry_module_to_entry_chunk: FxHashMap<ModuleIdx, ChunkIdx>,
  /// If the namespace is not merged, `Key` == `Value`.
  /// If the namespace is merged, `Key` is the original namespace symbol, and `Value` is the linked namespace symbol.
  pub finalized_cjs_ns_map_idx_vec: IndexVec<ChunkIdx, FxHashMap<SymbolRef, SymbolRef>>,
  pub chunk_idx_to_reference_ids: FxHashMap<ChunkIdx, Vec<ArcStr>>,
}

impl ChunkGraph {
  pub fn new(modules_len: usize) -> Self {
    Self {
      chunk_table: ChunkTable::default(),
      module_to_chunk: index_vec![None; modules_len],
      sorted_chunk_idx_vec: Vec::new(),
      entry_module_to_entry_chunk: FxHashMap::default(),
      finalized_cjs_ns_map_idx_vec: index_vec![],
      chunk_idx_to_reference_ids: FxHashMap::default(),
    }
  }

  #[expect(unused)]
  pub fn sorted_chunks(&self) -> impl Iterator<Item = &Chunk> {
    self.sorted_chunk_idx_vec.iter().map(move |&id| &self.chunk_table.chunks[id])
  }

  pub fn add_chunk(&mut self, chunk: Chunk) -> ChunkIdx {
    let idx = self.chunk_table.push(chunk);
    self.finalized_cjs_ns_map_idx_vec.push(FxHashMap::default());
    idx
  }

  pub fn add_module_to_chunk(
    &mut self,
    module_idx: ModuleIdx,
    chunk_idx: ChunkIdx,
    depended_runtime_helper: RuntimeHelper,
  ) {
    self.chunk_table.chunks[chunk_idx].modules.push(module_idx);
    self.module_to_chunk[module_idx] = Some(chunk_idx);
    self.chunk_table.chunks[chunk_idx].depended_runtime_helper.insert(depended_runtime_helper);
  }

  pub fn sort_chunk_modules(&mut self, link_output: &LinkStageOutput, options: &SharedOptions) {
    // Sort modules in each chunk by execution order
    self.chunk_table.iter_mut().for_each(|chunk| {
      if matches!(
        options.experimental.chunk_modules_order.unwrap_or_default(),
        ChunkModulesOrderBy::ExecOrder
      ) {
        chunk.modules.sort_unstable_by_key(|idx| link_output.module_table[*idx].exec_order());
        return;
      }

      chunk.modules.sort_by_key(|idx| module_sort_key(&link_output.module_table[*idx]));
    });
  }
}
