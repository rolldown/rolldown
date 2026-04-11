use oxc::span::Span;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  EcmaModuleAstUsage, ImportKind, ImportRecordIdx, ModuleIdx, ModuleTable, NormalModule,
};
use rolldown_error::{BuildDiagnostic, ImportChainNote, RequireTla};

use super::LinkStage;

#[derive(Clone, Copy, Default)]
enum TlaVisitState {
  #[default]
  NotVisited,
  Visiting,
  /// `Some(module_idx)` = the module that actually contains the top-level await.
  /// `None` = no TLA in this module or its dependencies.
  Visited(Option<ModuleIdx>),
}

/// Look up the source span of a given import record within a module. Linear
/// in `imports.len()` but only called on error paths.
///
/// Every `Import`/`Require` record is registered in `module.imports` by the
/// ast scanner, so this lookup must succeed for the kinds the TLA check
/// traverses. A miss indicates a scanner invariant violation.
fn import_span_for(module: &NormalModule, target: ImportRecordIdx) -> Span {
  let span = module.imports.iter().find_map(|(span, &idx)| (idx == target).then_some(*span));
  debug_assert!(
    span.is_some(),
    "import record {target:?} missing from imports map in module {:?}",
    module.stable_id
  );
  span.unwrap_or(Span::empty(0))
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn compute_tla(&mut self) {
    if self.tla_module_count == 0 {
      return;
    }

    fn find_tla_source(
      module_idx: ModuleIdx,
      module_table: &ModuleTable,
      visited: &mut IndexVec<ModuleIdx, TlaVisitState>,
    ) -> Option<ModuleIdx> {
      match visited[module_idx] {
        TlaVisitState::Visited(result) => return result,
        TlaVisitState::Visiting => return None,
        TlaVisitState::NotVisited => {}
      }

      visited[module_idx] = TlaVisitState::Visiting;
      let module = &module_table[module_idx];
      let is_self_tla = module
        .as_normal()
        .is_some_and(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait));
      if is_self_tla {
        visited[module_idx] = TlaVisitState::Visited(Some(module_idx));
        return Some(module_idx);
      }

      let tla_source = module
        .import_records()
        .iter()
        .filter(|rec| matches!(rec.kind, ImportKind::Import))
        .find_map(|rec| {
          rec
            .resolved_module
            .and_then(|dep_idx| find_tla_source(module_table[dep_idx].idx(), module_table, visited))
        });

      visited[module_idx] = TlaVisitState::Visited(tla_source);
      tla_source
    }

    fn build_import_chain(
      start_idx: ModuleIdx,
      tla_source_idx: ModuleIdx,
      module_table: &ModuleTable,
      visited: &IndexVec<ModuleIdx, TlaVisitState>,
    ) -> Vec<ImportChainNote> {
      let mut chain = Vec::new();
      let mut current_idx = start_idx;

      while current_idx != tla_source_idx {
        let module = &module_table[current_idx];
        let Some(normal) = module.as_normal() else {
          break;
        };

        let next = normal
          .import_records
          .iter_enumerated()
          .filter(|(_, rec)| matches!(rec.kind, ImportKind::Import))
          .find_map(|(rec_idx, rec)| {
            let dep_idx = rec.resolved_module?;
            let dep_module_idx = module_table[dep_idx].idx();
            match visited[dep_module_idx] {
              TlaVisitState::Visited(Some(source)) if source == tla_source_idx => {
                let importee = &module_table[dep_idx];
                Some((dep_module_idx, import_span_for(normal, rec_idx), importee.stable_id()))
              }
              _ => None,
            }
          });

        if let Some((next_idx, import_span, importee_stable_id)) = next {
          chain.push(ImportChainNote {
            importer_stable_id: module.stable_id().as_arc_str().clone(),
            importer_source: normal.source.clone(),
            importee_stable_id: importee_stable_id.as_arc_str().clone(),
            import_span,
          });
          current_idx = next_idx;
        } else {
          break;
        }
      }

      chain
    }

    let mut visited = index_vec![TlaVisitState::NotVisited; self.module_table.modules.len()];

    self.module_table.modules.iter().filter_map(|m| m.as_normal()).for_each(|module| {
      let tla_source = find_tla_source(module.idx, &self.module_table, &mut visited);
      self.metas[module.idx].is_tla_or_contains_tla_dependency = tla_source.is_some();

      // Check for require() of TLA modules — this is forbidden.
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        if !matches!(rec.kind, ImportKind::Require) {
          continue;
        }
        let Some(resolved_module_idx) = rec.resolved_module else { continue };
        let dep_idx = self.module_table[resolved_module_idx].idx();
        let Some(tla_source_idx) = find_tla_source(dep_idx, &self.module_table, &mut visited)
        else {
          continue;
        };

        let require_span = import_span_for(module, rec_idx);
        let import_chain =
          build_import_chain(dep_idx, tla_source_idx, &self.module_table, &visited);

        let tla_module = &self.module_table[tla_source_idx];
        // `find_tla_source` only returns modules whose `ast_usage` contains
        // `TopLevelAwait`, and the scanner always records a keyword span for
        // those modules, so this map lookup must hit.
        let tla_keyword_span = self.tla_keyword_span_map.get(&tla_source_idx).copied();
        debug_assert!(
          tla_keyword_span.is_some(),
          "tla_keyword_span missing for TLA source module {tla_source_idx:?}"
        );
        let tla_keyword_span = tla_keyword_span.unwrap_or(Span::empty(0));

        self.errors.push(BuildDiagnostic::require_tla(RequireTla {
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
