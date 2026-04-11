use arcstr::ArcStr;
use oxc::span::Span;
use oxc_index::{IndexVec, index_vec};
use rolldown_common::{EcmaModuleAstUsage, ImportKind, ModuleIdx, ModuleTable};
use rolldown_error::BuildDiagnostic;

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

#[derive(Debug, Clone)]
struct ImportChainStep {
  importer_stable_id: String,
  importer_source: ArcStr,
  importee_stable_id: String,
  import_span: Span,
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
    ) -> Vec<ImportChainStep> {
      let mut chain = Vec::new();
      let mut current_idx = start_idx;

      loop {
        if current_idx == tla_source_idx {
          break;
        }

        let module = &module_table[current_idx];
        let Some(normal) = module.as_normal() else {
          break;
        };

        let next = normal
          .import_records
          .iter()
          .filter(|rec| matches!(rec.kind, ImportKind::Import))
          .find_map(|rec| {
            let dep_idx = rec.resolved_module?;
            let dep_module_idx = module_table[dep_idx].idx();
            match visited[dep_module_idx] {
              TlaVisitState::Visited(Some(source)) if source == tla_source_idx => {
                let import_span = normal
                  .imports
                  .iter()
                  .find_map(|(span, &rec_idx)| {
                    if normal.import_records[rec_idx].module_request == rec.module_request {
                      Some(*span)
                    } else {
                      None
                    }
                  })
                  .unwrap_or(Span::empty(0));

                let importee = &module_table[dep_idx];
                Some((dep_module_idx, import_span, importee.stable_id().to_string()))
              }
              _ => None,
            }
          });

        if let Some((next_idx, import_span, importee_stable_id)) = next {
          chain.push(ImportChainStep {
            importer_stable_id: module.stable_id().to_string(),
            importer_source: normal.source.clone(),
            importee_stable_id,
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
      for rec in &module.import_records {
        if matches!(rec.kind, ImportKind::Require) {
          if let Some(resolved_module_idx) = rec.resolved_module {
            let dep_idx = self.module_table[resolved_module_idx].idx();
            if let Some(tla_source_idx) = find_tla_source(dep_idx, &self.module_table, &mut visited)
            {
              let is_direct = dep_idx == tla_source_idx;

              let require_span = module
                .imports
                .iter()
                .find_map(|(span, &rec_idx)| {
                  if module.import_records[rec_idx].module_request == rec.module_request {
                    Some(*span)
                  } else {
                    None
                  }
                })
                .unwrap_or(Span::empty(0));

              let import_chain = if is_direct {
                vec![]
              } else {
                build_import_chain(dep_idx, tla_source_idx, &self.module_table, &visited)
              };

              let tla_module = self.module_table[tla_source_idx].as_normal();
              let tla_stable_id = self.module_table[tla_source_idx].stable_id().to_string();
              let tla_source_text = tla_module.map(|m| m.source.clone()).unwrap_or_default();
              let tla_keyword_span =
                tla_module.and_then(|m| m.tla_keyword_span).unwrap_or(Span::empty(0));

              self.errors.push(BuildDiagnostic::require_tla(
                module.stable_id.to_string(),
                module.source.clone(),
                require_span,
                tla_stable_id,
                tla_source_text,
                tla_keyword_span,
                is_direct,
                import_chain
                  .into_iter()
                  .map(|step| {
                    (
                      step.importer_stable_id,
                      step.importer_source,
                      step.importee_stable_id,
                      step.import_span,
                    )
                  })
                  .collect(),
              ));
            }
          }
        }
      }
    });
  }
}
