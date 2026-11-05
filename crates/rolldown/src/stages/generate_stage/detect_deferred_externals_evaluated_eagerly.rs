use rolldown_common::{ChunkIdx, ImportKind, Module, ModuleIdx, OutputFormat};
use rolldown_error::{BuildDiagnostic, EventKindSwitcher};
use rolldown_utils::{index_bitset::IndexBitSet, node_builtin::is_node_like_builtin};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_graph::ChunkGraph;

use super::GenerateStage;

impl GenerateStage<'_> {
  /// `DEFERRED_EXTERNAL_EVALUATED_EAGERLY`. ESM, IIFE and UMD output load each external module at
  /// the start of the chunk. Thus, an external module that a module of a deferred closure
  /// (`import defer`, `import.defer()`) imports evaluates when the chunk loads, not when the
  /// deferred module evaluates. CommonJS output loads it in the init of the module instead
  /// (`lazy_external_import_stmt`), and gets no warning.
  /// - One warning for each external module, with one example path.
  /// - No warning when a module of the same chunk outside of the deferred closures imports the
  ///   external module: it evaluates at the start in the spec too. A module of a deferred closure
  ///   that also has an eager importer still counts as deferred.
  /// - No warning for an external module without side effects, or for a builtin module of Node.js
  ///   or Bun. The exemption of builtins only reduces noise: some builtins have a visible effect.
  /// - The direct import of an external module in the defer phase has its own rule
  ///   (`check_deferred_externals`).
  pub fn detect_deferred_externals_evaluated_eagerly(&mut self, chunk_graph: &ChunkGraph) {
    let format = match self.options.format {
      OutputFormat::Esm => "ESM",
      OutputFormat::Iife => "IIFE",
      OutputFormat::Umd => "UMD",
      OutputFormat::Cjs => return,
    };
    if !self.options.experimental.is_import_defer_enabled()
      || !self.options.checks.contains(EventKindSwitcher::DeferredExternalEvaluatedEagerly)
    {
      return;
    }
    let module_table = &self.link_output.module_table;
    let metas = &self.link_output.metas;
    // The external modules that a module outside of the deferred closures imports, per chunk.
    let mut eager_imports: FxHashMap<ChunkIdx, FxHashSet<ModuleIdx>> = FxHashMap::default();
    let mut reported = FxHashSet::default();
    let mut warnings = vec![];
    for module in module_table.modules.iter().filter_map(Module::as_normal) {
      let meta = &metas[module.idx];
      if !meta.wrapped_by_import_defer || !meta.is_included {
        continue;
      }
      let Some(chunk_idx) = chunk_graph.module_to_chunk[module.idx] else { continue };
      for rec in &module.import_records {
        if rec.kind != ImportKind::Import || rec.is_defer_phase() {
          continue;
        }
        let Some(external_idx) = rec.resolved_module else { continue };
        let Module::External(external) = &module_table[external_idx] else { continue };
        if reported.contains(&external_idx)
          || !external.side_effects.has_side_effects()
          || is_node_like_builtin(external.id.as_str())
        {
          continue;
        }
        let eager = eager_imports.entry(chunk_idx).or_insert_with(|| {
          chunk_graph.chunk_table[chunk_idx]
            .modules
            .iter()
            .filter(|&&idx| !metas[idx].wrapped_by_import_defer)
            .filter_map(|&idx| module_table[idx].as_normal())
            .flat_map(|module| module.import_records.iter())
            .filter(|rec| rec.kind == ImportKind::Import && !rec.is_defer_phase())
            .filter_map(|rec| rec.resolved_module)
            .collect()
        });
        if eager.contains(&external_idx) {
          continue;
        }
        reported.insert(external_idx);
        let Some((deferring, deferred)) = self.import_defer_example_path(module.idx) else {
          continue;
        };
        warnings.push(
          BuildDiagnostic::deferred_external_evaluated_eagerly(
            external.id.as_arc_str().clone(),
            module_table[deferring].id().as_arc_str().clone(),
            module_table[deferred].id().as_arc_str().clone(),
            module.id.as_arc_str().clone(),
            format,
          )
          .with_severity_warning(),
        );
      }
    }
    self.link_output.diagnostics.extend(warnings);
  }

  /// One deferred import that has `module_idx` in its static dependency closure, as
  /// `(module with the deferred import, deferred module)`. The first one in module order.
  fn import_defer_example_path(&self, module_idx: ModuleIdx) -> Option<(ModuleIdx, ModuleIdx)> {
    let module_table = &self.link_output.module_table;
    let static_deps = |idx: ModuleIdx| {
      module_table[idx]
        .as_normal()
        .into_iter()
        .flat_map(|module| module.import_records.iter())
        .filter(|rec| rec.kind.is_static())
        .filter_map(|rec| rec.resolved_module)
    };
    // One set for all walks: a module that an earlier walk visited cannot reach `module_idx`,
    // because that walk would have returned.
    let mut visited: IndexBitSet<ModuleIdx> = IndexBitSet::new(module_table.modules.len());
    for deferring in module_table.modules.iter().filter_map(Module::as_normal) {
      for rec in deferring.import_records.iter().filter(|rec| rec.is_defer_phase()) {
        let Some(deferred) = rec.resolved_module else { continue };
        let mut stack = vec![deferred];
        while let Some(idx) = stack.pop() {
          if idx == module_idx {
            return Some((deferring.idx, deferred));
          }
          if visited.set_bit(idx) {
            stack.extend(static_deps(idx));
          }
        }
      }
    }
    None
  }
}
