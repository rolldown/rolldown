use oxc::span::Span;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  EcmaModuleAstUsage, ImportKind, ImportRecordIdx, ModuleIdx, ModuleTable, NormalModule,
  StmtInfoMeta,
};
use rolldown_error::{BuildDiagnostic, ImportChainNote, RequireTla};
use std::collections::VecDeque;

use super::LinkStage;

fn import_span_for(module: &NormalModule, target: ImportRecordIdx) -> Span {
  module.import_records.get(target).map_or(Span::empty(0), |rec| rec.importer_span)
}

impl LinkStage<'_> {
  /// The link steps of `import defer` read top-level await facts before tree shaking:
  /// `wrap_modules` chooses the tracked modules and the runtime helpers of their init wrappers,
  /// and `reference_needed_symbols` keeps the preparation of a deferred import. `compute_tla` runs
  /// after tree shaking, so these steps read this early flag: a module with `await` at the top
  /// level, or a module that reaches one through static imports. The early flag is a superset of
  /// the result of `compute_tla`, which then sets the final flag. Thus a module whose `await` tree
  /// shaking removes can keep a state record and a runtime helper that it does not use.
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn mark_tla_before_tree_shaking(&mut self) {
    if self.tla_module_count == 0 {
      return;
    }
    let modules = self.sorted_modules.iter().filter_map(|&idx| self.module_table[idx].as_normal());
    let mut queue: VecDeque<_> = modules
      .clone()
      .filter(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait))
      .map(|module| module.idx)
      .collect();
    let mut importers = index_vec![Vec::new(); self.module_table.modules.len()];
    for module in modules {
      for rec in &module.import_records {
        if matches!(rec.kind, ImportKind::Import)
          && let Some(dependency) = rec.resolved_module
        {
          importers[dependency].push(module.idx);
        }
      }
    }
    for &source in &queue {
      self.metas[source].is_tla_or_contains_tla_dependency = true;
    }
    while let Some(dependency) = queue.pop_front() {
      for &importer in &importers[dependency] {
        if !self.metas[importer].is_tla_or_contains_tla_dependency {
          self.metas[importer].is_tla_or_contains_tla_dependency = true;
          queue.push_back(importer);
        }
      }
    }
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn compute_tla(&mut self) {
    if self.tla_module_count == 0 {
      return;
    }

    // Execution order keeps diagnostic sources and import chains independent of load timing.
    let modules = self.sorted_modules.iter().filter_map(|&idx| self.module_table[idx].as_normal());
    let mut queue: VecDeque<_> = modules
      .clone()
      .filter(|module| {
        module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait)
          && self.stmt_infos[module.idx].iter_enumerated().any(|(idx, stmt)| {
            stmt.meta.contains(StmtInfoMeta::TopLevelAwait)
              && self.metas[module.idx].stmt_info_included.has_bit(idx)
          })
      })
      .map(|module| module.idx)
      .collect();

    let mut tla_sources = index_vec![None; self.module_table.modules.len()];
    let mut tla_imports = index_vec![None; self.module_table.modules.len()];
    let mut importers = index_vec![Vec::new(); self.module_table.modules.len()];
    for &source in &queue {
      tla_sources[source] = Some(source);
    }
    for module in modules {
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        if matches!(rec.kind, ImportKind::Import)
          && let Some(dependency) = rec.resolved_module
        {
          importers[dependency].push((module.idx, rec_idx));
        }
      }
    }
    while let Some(dependency) = queue.pop_front() {
      for &(importer, rec_idx) in &importers[dependency] {
        if tla_sources[importer].is_none() {
          tla_sources[importer] = tla_sources[dependency];
          tla_imports[importer] = Some((dependency, rec_idx));
          queue.push_back(importer);
        }
      }
    }

    fn build_import_chain(
      start_idx: ModuleIdx,
      module_table: &ModuleTable,
      tla_imports: &IndexVec<ModuleIdx, Option<(ModuleIdx, ImportRecordIdx)>>,
    ) -> Vec<ImportChainNote> {
      let mut chain = Vec::new();
      let mut current_idx = start_idx;

      while let Some((next_idx, rec_idx)) = tla_imports[current_idx] {
        let module = &module_table[current_idx];
        let Some(normal) = module.as_normal() else {
          break;
        };

        chain.push(ImportChainNote {
          importer_stable_id: module.stable_id().as_arc_str().clone(),
          importer_source: normal.source.clone(),
          importee_stable_id: module_table[next_idx].stable_id().as_arc_str().clone(),
          import_span: import_span_for(normal, rec_idx),
        });
        current_idx = next_idx;
      }

      chain
    }

    let import_defer = self.options.experimental.is_import_defer_enabled();
    self.module_table.modules.iter().filter_map(|m| m.as_normal()).for_each(|module| {
      let is_tla = tla_sources[module.idx].is_some();
      debug_assert!(
        !import_defer || !is_tla || self.metas[module.idx].is_tla_or_contains_tla_dependency,
        "`mark_tla_before_tree_shaking` must mark each module that `compute_tla` marks"
      );
      self.metas[module.idx].is_tla_or_contains_tla_dependency = is_tla;

      // Check for require() of TLA modules — this is forbidden.
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        if !matches!(rec.kind, ImportKind::Require) {
          continue;
        }
        let Some(resolved_module_idx) = rec.resolved_module else { continue };
        let dep_idx = self.module_table[resolved_module_idx].idx();
        let Some(tla_source_idx) = tla_sources[dep_idx] else {
          continue;
        };

        let require_span = import_span_for(module, rec_idx);
        let import_chain = build_import_chain(dep_idx, &self.module_table, &tla_imports);

        let tla_module = &self.module_table[tla_source_idx];
        // TLA sources have an included await statement, so the scanner records a keyword span.
        let tla_keyword_span = self.tla_keyword_span_map.get(&tla_source_idx).copied();
        debug_assert!(
          tla_keyword_span.is_some(),
          "tla_keyword_span missing for TLA source module {tla_source_idx:?}"
        );
        let tla_keyword_span = tla_keyword_span.unwrap_or(Span::empty(0));

        self.diagnostics.push(BuildDiagnostic::require_tla(RequireTla {
          importer_id: module.id.as_arc_str().clone(),
          importer_stable_id: module.stable_id.as_arc_str().clone(),
          importer_source: module.source.clone(),
          require_span,
          tla_source_stable_id: tla_module.stable_id().as_arc_str().clone(),
          tla_source_text: tla_module.as_normal().map(|m| m.source.clone()).unwrap_or_default(),
          tla_keyword_span,
          import_chain,
        }));
      }
    });
  }
}
