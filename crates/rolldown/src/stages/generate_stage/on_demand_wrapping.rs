use std::collections::VecDeque;

use rolldown_common::{
  Chunk, ChunkKind, ConcatenateWrappedModuleKind, EcmaViewMeta, ImportKind, ModuleGroup, ModuleIdx,
  WrapKind,
};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn on_demand_wrapping(&mut self, chunk_graph: &mut ChunkGraph) {
    // Currently, hmr is strongly rely on wrapping function to update module exports
    if !self.options.experimental.strict_execution_order.unwrap_or_default()
      || !self.options.experimental.is_on_demand_wrapping_enabled()
      || self.options.is_hmr_enabled()
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
  #[allow(unused)]
  fn inline_entry_chunk_wrapping(&mut self, chunk: &Chunk) {
    // All modules in entry chunk must be reachable from a entry module.
    let mut boundary_module = chunk
      .exports_to_other_chunks
      .keys()
      .map(|symbol_ref| self.link_output.symbol_db.canonical_ref_for(*symbol_ref).owner)
      .collect::<FxHashSet<_>>();

    let chunk_modules_set = chunk.modules.iter().copied().collect::<FxHashSet<_>>();
    for idx in &chunk.modules {
      let Some(normal_module) = self.link_output.module_table[*idx].as_normal() else {
        continue;
      };
      if normal_module.exports_kind.is_commonjs() {
        boundary_module.insert(*idx);
      }
      let mut bailout_importer = false;
      for rec in &normal_module.import_records {
        if chunk_modules_set.contains(&rec.resolved_module) {
          // 1. `import('./esm.js')` when `inline_dynamic_imports` is enabled
          // 2. `require('./esm.js')`
          if rec.kind == ImportKind::Require || rec.kind == ImportKind::DynamicImport {
            boundary_module.insert(rec.resolved_module);
          }
        }
        let Some(normal) = self.link_output.module_table[rec.resolved_module].as_normal() else {
          continue;
        };
        if self.link_output.metas[normal.idx].wrap_kind == WrapKind::Cjs
          && normal.meta.contains(EcmaViewMeta::ExecutionOrderSensitive)
        {
          bailout_importer = true;
        }
      }

      if bailout_importer {
        boundary_module.insert(*idx);
      }
    }

    self.expand_boundary(&mut boundary_module, &chunk_modules_set);
    for module_idx in &chunk.modules {
      if boundary_module.contains(module_idx) {
        // If the module is a boundary module, we can't inline it.
        continue;
      }
      if self.link_output.metas[*module_idx].wrap_kind == WrapKind::Esm {
        self.link_output.metas[*module_idx].wrap_kind = WrapKind::None;
      }
    }
  }

  #[allow(unused)]
  fn expand_boundary(
    &self,
    boundary_modules: &mut FxHashSet<ModuleIdx>,
    chunk_modules: &FxHashSet<ModuleIdx>,
  ) {
    let mut visited = FxHashSet::default();
    let mut q = std::mem::take(boundary_modules).into_iter().collect::<VecDeque<_>>();
    while let Some(module_idx) = q.pop_front() {
      if visited.contains(&module_idx) {
        continue;
      }
      visited.insert(module_idx);
      let Some(module) = self.link_output.module_table[module_idx].as_normal() else {
        continue;
      };
      for dep in &module.import_records {
        let importee_idx = dep.resolved_module;
        if chunk_modules.contains(&importee_idx) {
          q.push_back(importee_idx);
        }
      }
    }
    *boundary_modules = visited;
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
      if matches!(self.link_output.metas[*module_idx].wrap_kind, WrapKind::Cjs | WrapKind::None) {
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
      if matches!(self.link_output.metas[*module_idx].wrap_kind, WrapKind::Cjs | WrapKind::None) {
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
