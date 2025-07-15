use std::collections::VecDeque;

use rolldown_common::{Chunk, ChunkKind, EcmaViewMeta, ImportKind, ModuleIdx, WrapKind};
use rustc_hash::FxHashSet;

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[allow(clippy::too_many_lines)]
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn on_demand_wrapping(&mut self, chunk_graph: &mut ChunkGraph) {
    // Currently, hmr is strongly rely on wrapping function to update module exports
    if !self.options.experimental.is_on_demand_wrapping_enabled() || self.options.is_hmr_enabled() {
      // If strict execution order is not enabled, we don't need to do on-demand wrapping.
      return;
    }
    for chunk in chunk_graph.chunk_table.iter_mut() {
      if matches!(chunk.kind, ChunkKind::EntryPoint { .. }) {
        self.inline_entry_chunk_wrapping(chunk);
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
  /// We could inline all reachable modules of the entry chunk, except module is a bundry module.
  /// Here is the definition of boundary module:
  /// - A module that has symbol used by other chunk, we can't safely eager eval it.
  /// - A module imported side effects module that has WrapKind::Cjs
  /// - A commonjs module, a commonjs module can't not be safely eager eval.
  /// - A es module required by other module
  /// - All reachable modules of a boundary module in current chunk.
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
          && normal.meta.contains(EcmaViewMeta::HAS_ANALYZED_SIDE_EFFECT)
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
}
