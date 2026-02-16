use std::collections::VecDeque;

use rolldown_common::{
  Chunk, ChunkKind, ConcatenateWrappedModuleKind, EcmaViewMeta, ImportKind, ModuleGroup, ModuleIdx,
  WrapKind,
};
use rolldown_utils::IndexBitSet;
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::FxHashMap;

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
        // self.inline_entry_chunk_wrapping(chunk);
        self.concatenate_wrapping_modules(chunk);
        // Needs to investigate how to merge these two optimization
      }
    }
  }

  /// Use https://repl.rolldown.rs/#eNptkcFugzAMhl/FyoVWYhndZVKqHneutDOHBjAVEyQohI0J8e6zoaFirYREEv+2P/8eRSnUKCpT4CC/Oj4boe73WOR8bVrrPIyltROUzjYQyVdN8Sg1qcmt6WyNsrbX3aWxhSSZgksM9N+zIDU4zAVY6UHDCQ5JkhypOgrlXY9TLJyt68L+GEmisrpKv8I8iWyw/G+LMMLnTXZufUV9IJCG9AgC7AxCldRDzgnG1AAQLrqqQeN1rZYngM67KvcfA+Y9i8+uQKeA6Tk+pYa+ddICS93XoZGg+diuMNF83lqr78ZmwdhbrbI3OXdkP3f7Bceh751hG4/bxqs40+6fWC9ShuEWAWY+LzDP1kRJ2xVHhBHRXsVEhb4p6yATmbxk6LV8exfTH7g7ybo=
  /// as an example.
  /// A entry chunk always has pattern like when strict_execution_order_is_enabled:
  /// ```js
  /// // ...snip dependencies wrapping
  /// var init_entry_module = function() {
  ///   init_other();
  /// };
  /// init_entry_module();
  /// ```
  /// We could inline all reachable modules of the entry chunk, except module is a boundary module.
  /// Here is the definition of boundary module:
  /// - A module that has symbol used by other chunk, we can't safely eager eval it.
  /// - A module imported side effects module that has WrapKind::Cjs
  /// - A commonjs module, a commonjs module can't not be safely eager eval.
  /// - A es module required by other module
  /// - All reachable modules of a boundary module in current chunk.
  // NOTE: Using #[allow] because these methods are conditionally used
  #[allow(clippy::allow_attributes)]
  #[allow(dead_code)]
  fn inline_entry_chunk_wrapping(&mut self, chunk: &Chunk) {
    // All modules in entry chunk must be reachable from a entry module.
    let modules_len = self.link_output.module_table.modules.len();
    let mut boundary_module = IndexBitSet::new(modules_len);
    for symbol_ref in chunk.exports_to_other_chunks.keys() {
      boundary_module.set_bit(self.link_output.symbol_db.canonical_ref_for(*symbol_ref).owner);
    }

    let mut chunk_modules_set = IndexBitSet::new(modules_len);
    chunk_modules_set.extend(chunk.modules.iter().copied());
    for idx in &chunk.modules {
      let Some(normal_module) = self.link_output.module_table[*idx].as_normal() else {
        continue;
      };
      if normal_module.exports_kind.is_commonjs() {
        boundary_module.set_bit(*idx);
      }
      let mut bailout_importer = false;
      normal_module
        .import_records
        .iter()
        .filter_map(|rec| rec.resolved_module.map(|module_idx| (rec, module_idx)))
        .for_each(|(rec, module_idx)| {
          if chunk_modules_set.has_bit(module_idx) {
            // 1. `import('./esm.js')` when `code_splitting` is disabled
            // 2. `require('./esm.js')`
            if rec.kind == ImportKind::Require || rec.kind == ImportKind::DynamicImport {
              boundary_module.set_bit(module_idx);
            }
          }
          let Some(normal) = self.link_output.module_table[module_idx].as_normal() else {
            return;
          };
          if self.link_output.metas[normal.idx].wrap_kind() == WrapKind::Cjs
            && normal.meta.contains(EcmaViewMeta::ExecutionOrderSensitive)
          {
            bailout_importer = true;
          }
        });

      if bailout_importer {
        boundary_module.set_bit(*idx);
      }
    }

    self.expand_boundary(&mut boundary_module, &chunk_modules_set);
    for module_idx in &chunk.modules {
      if boundary_module.has_bit(*module_idx) {
        // If the module is a boundary module, we can't inline it.
        continue;
      }
      if self.link_output.metas[*module_idx].wrap_kind() == WrapKind::Esm {
        self.link_output.metas[*module_idx].update_wrap_kind(WrapKind::None);
      }
    }
  }

  // NOTE: Using #[allow] because these methods are conditionally used
  #[allow(clippy::allow_attributes)]
  #[allow(dead_code)]
  fn expand_boundary(
    &self,
    boundary_modules: &mut IndexBitSet<ModuleIdx>,
    chunk_modules: &IndexBitSet<ModuleIdx>,
  ) {
    let modules_len = self.link_output.module_table.modules.len();
    let mut visited = IndexBitSet::new(modules_len);
    let mut q = std::mem::replace(boundary_modules, IndexBitSet::new(modules_len))
      .into_iter()
      .collect::<VecDeque<_>>();
    while let Some(module_idx) = q.pop_front() {
      if visited.has_bit(module_idx) {
        continue;
      }
      visited.set_bit(module_idx);
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      module.import_records.iter().filter_map(|rec| rec.resolved_module).for_each(|importee_idx| {
        if chunk_modules.has_bit(importee_idx) {
          q.push_back(importee_idx);
        }
      });
    }
    *boundary_modules = visited;
  }

  fn concatenate_wrapping_modules(&mut self, chunk: &mut Chunk) {
    // Those modules could only be a group root.
    let modules_len = self.link_output.module_table.modules.len();
    let mut root_only = IndexBitSet::new(modules_len);
    root_only.extend(chunk.exports_to_other_chunks.keys().map(|symbol| symbol.owner));

    let mut module_to_exec_order = FxHashMap::with_capacity(chunk.modules.len());
    for module_idx in &chunk.modules {
      module_to_exec_order
        .insert(*module_idx, self.link_output.module_table[*module_idx].exec_order());
      let meta = &self.link_output.metas[*module_idx];
      if matches!(meta.wrap_kind(), WrapKind::Cjs | WrapKind::None) || meta.required_by_other_module
      {
        root_only.set_bit(*module_idx);
      }
    }
    let mut module_groups = vec![];
    let mut module_idx_to_group_idx = FxHashMap::default();
    // higher exec_order usually means module is more closed to entry point
    // lower exec_order means module is more closed to the leaf node of the chunk.
    let mut visited = IndexBitSet::new(modules_len);
    for module_idx in chunk.modules.iter().rev() {
      if visited.has_bit(*module_idx) {
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
    root_only: &IndexBitSet<ModuleIdx>,
    visited: &mut IndexBitSet<ModuleIdx>,
    entry: ModuleIdx,
  ) -> ModuleGroup {
    // This function is used to expand module group, so that we can concatenate
    // all modules in the group.
    let modules_len = self.link_output.module_table.modules.len();
    let mut q = VecDeque::from_iter([entry]);
    let mut groups = IndexBitSet::new(modules_len);
    while let Some(module_idx) = q.pop_front() {
      if visited.has_bit(module_idx) {
        continue;
      }
      visited.set_bit(module_idx);
      groups.set_bit(module_idx);
      for dep in self.link_output.metas[module_idx]
        .dependencies
        .iter()
        .filter(|idx| module_to_exec_order.contains_key(idx) && !root_only.has_bit(**idx))
      {
        q.push_back(*dep);
      }
    }

    let mut prune_set = IndexBitSet::new(modules_len);
    for module_idx in groups.index_of_one() {
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      module
        .importers_idx
        .iter()
        .filter(|importer_idx| {
          // If the importer is not in the group, we can prune it.
          !groups.has_bit(**importer_idx) && module_to_exec_order.contains_key(importer_idx)
        })
        .for_each(|importer_idx| {
          prune_set.set_bit(*importer_idx);
        });
    }

    for module_idx in prune_set {
      groups.clear_bit(module_idx);
      visited.clear_bit(module_idx);
    }

    ModuleGroup { modules: groups.into_iter().collect(), entry }
  }
}
