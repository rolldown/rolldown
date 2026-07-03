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

  /// Whether the chunk entry's `init_*` wrapper can only ever be invoked by the single
  /// immediate call the chunk itself emits right after the chunk content
  /// (`render_wrapped_entry_chunk`). In that case wrapping the entry's module group is pure
  /// overhead — the closure runs exactly once, immediately — so its members can execute in
  /// place, unwrapped, matching the output plain tree-shaking produces.
  fn entry_group_inlinable(&self, chunk: &Chunk) -> Option<ModuleIdx> {
    let ChunkKind::EntryPoint { module: entry_idx, .. } = chunk.kind else {
      return None;
    };
    let entry_meta = &self.link_output.metas[entry_idx];
    // Only a wrapped-ESM entry has the immediately-invoked wrapper shape.
    if !matches!(entry_meta.wrap_kind(), WrapKind::Esm) {
      return None;
    }
    // A TLA-tainted group is invoked as `await init_*()`; keep it wrapped so the `await`s
    // stay inside an async closure regardless of output format.
    if entry_meta.is_tla_or_contains_tla_dependency {
      return None;
    }
    // `require()` importers initialize the entry through `(init_x(), __toCommonJS(...))`.
    if entry_meta.required_by_other_module {
      return None;
    }
    // Another chunk imports `init_x` to control when the entry executes.
    if entry_meta.wrapper_ref.is_some_and(|r| chunk.exports_to_other_chunks.contains_key(&r)) {
      return None;
    }
    let entry_module = self.link_output.module_table[entry_idx].as_normal()?;
    // Any importer may lower to an `init_x()` call site (e.g. an entry statically imported
    // by another entry, a cycle back into the entry, or a same-chunk dynamic import).
    (entry_module.importers_idx.is_empty() && entry_module.dynamic_importers.is_empty())
      .then_some(entry_idx)
  }

  fn concatenate_wrapping_modules(&mut self, chunk: &mut Chunk) {
    let inlinable_entry = self.entry_group_inlinable(chunk);
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

      if inlinable_entry == Some(group.entry) {
        // The group's `init_*` wrapper would be declared and then invoked exactly once,
        // immediately, at the end of this chunk. Drop the wrappers so every member is
        // finalized and rendered as a plain module executing in place (each becomes its own
        // single-module group, interleaved by exec order via the sort below).
        for idx in group.modules {
          let meta = &mut self.link_output.metas[idx];
          meta.update_wrap_kind(WrapKind::None);
          meta.wrapper_stmt_info = None;
          meta.wrapper_ref = None;
          meta.wrapper_inlined = true;
          module_idx_to_group_idx.insert(idx, module_groups.len());
          module_groups.push(ModuleGroup::new(vec![idx], idx));
        }
        continue;
      }

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
