use std::collections::VecDeque;

use rolldown_common::{
  Chunk, ChunkKind, ConcatenateWrappedModuleKind, ModuleGroup, ModuleIdx, WrapKind,
};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn on_demand_wrapping(&mut self, chunk_graph: &mut ChunkGraph) {
    // Currently, hmr is strongly rely on wrapping function to update module exports
    if !self.options.is_strict_execution_order_enabled()
      || !self.options.experimental.is_on_demand_wrapping_enabled()
      || self.options.is_dev_mode_enabled()
    {
      // If strict execution order is not enabled, we don't need to do on-demand wrapping.
      return;
    }
    for chunk in chunk_graph.chunk_table.iter_mut() {
      if matches!(chunk.kind, ChunkKind::EntryPoint { .. }) {
        self.concatenate_wrapping_modules(chunk);
      }
    }
  }

  fn concatenate_wrapping_modules(&mut self, chunk: &mut Chunk) {
    // Those modules could only be a group root.
    let mut root_only = chunk
      .exports_to_other_chunks
      .keys()
      .map(|symbol| symbol.owner)
      .collect::<FxHashSet<ModuleIdx>>();

    let mut module_to_exec_order = FxHashMap::with_capacity(chunk.modules.len());
    for module_idx in &chunk.modules {
      module_to_exec_order
        .insert(*module_idx, self.link_output.module_table[*module_idx].exec_order());
      let meta = &self.link_output.metas[*module_idx];
      if matches!(meta.wrap_kind(), WrapKind::Cjs | WrapKind::None) || meta.required_by_other_module
      {
        root_only.insert(*module_idx);
      }
    }
    let mut module_groups = vec![];
    let mut module_idx_to_group_idx = FxHashMap::default();
    // higher exec_order usually means module is more closed to entry point
    // lower exec_order means module is more closed to the leaf node of the chunk.
    let mut visited = FxHashSet::default();
    for module_idx in chunk.modules.iter().rev() {
      if visited.contains(module_idx) {
        continue;
      }
      if matches!(self.link_output.metas[*module_idx].wrap_kind(), WrapKind::Cjs | WrapKind::None) {
        // If the module is a cjs or none wrapped module, we can't concatenate it.
        let group = ModuleGroup::new(vec![*module_idx], *module_idx);
        module_idx_to_group_idx.insert(*module_idx, module_groups.len());
        module_groups.push(group);
        continue;
      }

      let mut group =
        self.expand_module_group(&module_to_exec_order, &root_only, &mut visited, *module_idx);
      let len = group.modules.len();

      for idx in &group.modules {
        if *idx != group.entry {
          self.link_output.metas[*idx].concatenated_wrapped_module_kind =
            ConcatenateWrappedModuleKind::Inner;
        } else if len != 1 {
          self.link_output.metas[*idx].concatenated_wrapped_module_kind =
            ConcatenateWrappedModuleKind::Root;
        }
        module_idx_to_group_idx.insert(*idx, module_groups.len());
      }
      group.modules.sort_by_cached_key(|item| module_to_exec_order[item]);

      module_groups.push(group);
    }
    module_groups.sort_by_cached_key(|group| module_to_exec_order[&group.entry]);
    chunk.module_idx_to_group_idx = module_idx_to_group_idx;
    chunk.module_groups = module_groups;
  }

  fn expand_module_group(
    &self,
    module_to_exec_order: &FxHashMap<ModuleIdx, u32>,
    root_only: &FxHashSet<ModuleIdx>,
    visited: &mut FxHashSet<ModuleIdx>,
    entry: ModuleIdx,
  ) -> ModuleGroup {
    // This function is used to expand module group, so that we can concatenate
    // all modules in the group.
    let mut q = VecDeque::from_iter([entry]);
    let mut groups = FxHashSet::default();
    while let Some(module_idx) = q.pop_front() {
      if !visited.insert(module_idx) {
        continue;
      }
      groups.insert(module_idx);
      for dep in self.link_output.metas[module_idx]
        .dependencies
        .iter()
        .filter(|idx| module_to_exec_order.contains_key(idx) && !root_only.contains(idx))
      {
        q.push_back(*dep);
      }
    }

    let mut prune_set = FxHashSet::default();
    for module_idx in &groups {
      let Some(module) = self.link_output.module_table[*module_idx].as_normal() else {
        continue;
      };
      module
        .importers_idx
        .iter()
        .filter(|importer_idx| {
          // If the importer is not in the group, we can prune it.
          !groups.contains(importer_idx) && module_to_exec_order.contains_key(importer_idx)
        })
        .for_each(|importer_idx| {
          prune_set.insert(*importer_idx);
        });
    }

    for module_idx in prune_set {
      groups.remove(&module_idx);
      visited.remove(&module_idx);
    }

    ModuleGroup { modules: groups.into_iter().collect(), entry }
  }
}
