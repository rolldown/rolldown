use oxc::span::Span;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{
  EcmaModuleAstUsage, ImportKind, ImportRecordIdx, ModuleIdx, ModuleTable, NormalModule,
};
use rolldown_error::{BuildDiagnostic, ImportChainNote, RequireTla};
use rustc_hash::FxHashSet;

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

fn import_span_for(module: &NormalModule, target: ImportRecordIdx) -> Span {
  module.import_records.get(target).map_or(Span::empty(0), |rec| rec.importer_span)
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn compute_tla(&mut self) {
    if self.tla_module_count == 0 {
      return;
    }

    // On a cycle hit, this returns `None` for the edge back to the module that is still
    // `Visiting`, and sets `hit_cycle`. The modules on the current DFS path can then memoize
    // `None`, although the visiting module finds TLA later through another import.
    // `propagate_tla_source_through_cycles` fixes these results.
    fn find_tla_source(
      module_idx: ModuleIdx,
      module_table: &ModuleTable,
      visited: &mut IndexVec<ModuleIdx, TlaVisitState>,
      hit_cycle: &mut bool,
    ) -> Option<ModuleIdx> {
      match visited[module_idx] {
        TlaVisitState::Visited(result) => return result,
        TlaVisitState::Visiting => {
          *hit_cycle = true;
          return None;
        }
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
          rec.resolved_module.and_then(|dep_idx| {
            find_tla_source(module_table[dep_idx].idx(), module_table, visited, hit_cycle)
          })
        });

      visited[module_idx] = TlaVisitState::Visited(tla_source);
      tla_source
    }

    // A `Some` result of `find_tla_source` is always a real import path. Only a `None` result
    // after a cycle hit can be wrong: the module reaches TLA through a module of the cycle. This
    // gives each such module the source of an import that has one, until nothing changes. Each
    // fixed module then has an import with the same source, so `build_import_chain` still finds
    // a path.
    fn propagate_tla_source_through_cycles(
      module_table: &ModuleTable,
      visited: &mut IndexVec<ModuleIdx, TlaVisitState>,
    ) {
      // For each module, its importers that have the result `None`.
      let mut importers: IndexVec<ModuleIdx, Vec<ModuleIdx>> =
        index_vec![Vec::new(); module_table.modules.len()];
      let mut worklist = Vec::new();
      for module in module_table.modules.iter().filter_map(|m| m.as_normal()) {
        if !matches!(visited[module.idx], TlaVisitState::Visited(None)) {
          continue;
        }
        for rec in module.import_records.iter().filter(|rec| matches!(rec.kind, ImportKind::Import))
        {
          let Some(dep_idx) = rec.resolved_module else { continue };
          let dep_idx = module_table[dep_idx].idx();
          importers[dep_idx].push(module.idx);
          if matches!(visited[dep_idx], TlaVisitState::Visited(Some(_))) {
            worklist.push(dep_idx);
          }
        }
      }
      while let Some(dep_idx) = worklist.pop() {
        let TlaVisitState::Visited(Some(source)) = visited[dep_idx] else { continue };
        for importer_idx in std::mem::take(&mut importers[dep_idx]) {
          if matches!(visited[importer_idx], TlaVisitState::Visited(None)) {
            visited[importer_idx] = TlaVisitState::Visited(Some(source));
            worklist.push(importer_idx);
          }
        }
      }
    }

    fn build_import_chain(
      start_idx: ModuleIdx,
      tla_source_idx: ModuleIdx,
      module_table: &ModuleTable,
      visited: &IndexVec<ModuleIdx, TlaVisitState>,
    ) -> Vec<ImportChainNote> {
      let mut chain = Vec::new();
      let mut seen: FxHashSet<ModuleIdx> = FxHashSet::default();
      let mut current_idx = start_idx;

      // `seen` guards against cycles: cycle back-edges are memoized as
      // `Some(tla_source_idx)` just like forward edges, so a naive
      // first-match would walk into the cycle. Prefer the direct edge
      // to `tla_source_idx` when one exists so the chain stays short.
      while current_idx != tla_source_idx && seen.insert(current_idx) {
        let module = &module_table[current_idx];
        let Some(normal) = module.as_normal() else {
          break;
        };

        let mut direct = None;
        let mut indirect = None;
        for (rec_idx, rec) in normal.import_records.iter_enumerated() {
          if !matches!(rec.kind, ImportKind::Import) {
            continue;
          }
          let Some(dep_idx) = rec.resolved_module else { continue };
          let dep_module_idx = module_table[dep_idx].idx();
          if seen.contains(&dep_module_idx) {
            continue;
          }
          if dep_module_idx == tla_source_idx {
            let importee = &module_table[dep_idx];
            direct = Some((dep_module_idx, import_span_for(normal, rec_idx), importee.stable_id()));
            break;
          }
          if indirect.is_none()
            && matches!(
              visited[dep_module_idx],
              TlaVisitState::Visited(Some(source)) if source == tla_source_idx
            )
          {
            let importee = &module_table[dep_idx];
            indirect =
              Some((dep_module_idx, import_span_for(normal, rec_idx), importee.stable_id()));
          }
        }

        let Some((next_idx, import_span, importee_stable_id)) = direct.or(indirect) else {
          break;
        };

        chain.push(ImportChainNote {
          importer_stable_id: module.stable_id().as_arc_str().clone(),
          importer_source: normal.source.clone(),
          importee_stable_id: importee_stable_id.as_arc_str().clone(),
          import_span,
        });
        current_idx = next_idx;
      }

      chain
    }

    let mut visited = index_vec![TlaVisitState::NotVisited; self.module_table.modules.len()];
    let mut hit_cycle = false;
    for module in self.module_table.modules.iter().filter_map(|m| m.as_normal()) {
      find_tla_source(module.idx, &self.module_table, &mut visited, &mut hit_cycle);
    }
    if hit_cycle {
      propagate_tla_source_through_cycles(&self.module_table, &mut visited);
    }

    // All normal modules have their final result now. A call below visits only a module that
    // is not normal, which has no imports.
    self.module_table.modules.iter().filter_map(|m| m.as_normal()).for_each(|module| {
      let tla_source =
        find_tla_source(module.idx, &self.module_table, &mut visited, &mut hit_cycle);
      self.metas[module.idx].is_tla_or_contains_tla_dependency = tla_source.is_some();

      // Check for require() of TLA modules — this is forbidden.
      for (rec_idx, rec) in module.import_records.iter_enumerated() {
        if !matches!(rec.kind, ImportKind::Require) {
          continue;
        }
        let Some(resolved_module_idx) = rec.resolved_module else { continue };
        let dep_idx = self.module_table[resolved_module_idx].idx();
        let Some(tla_source_idx) =
          find_tla_source(dep_idx, &self.module_table, &mut visited, &mut hit_cycle)
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

        self.diagnostics.push(BuildDiagnostic::require_tla(RequireTla {
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
