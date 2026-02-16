use oxc_index::{IndexVec, index_vec};
use rolldown_common::{EcmaModuleAstUsage, ImportKind, ModuleIdx, ModuleTable};

use super::LinkStage;

#[derive(Clone, Copy, Default)]
enum TlaVisitState {
  #[default]
  NotVisited,
  Visiting,
  Visited(bool),
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub(super) fn compute_tla(&mut self) {
    // TODO: skip this phase if there is no module use TLA
    fn is_tla(
      module_idx: ModuleIdx,
      module_table: &ModuleTable,
      visited: &mut IndexVec<ModuleIdx, TlaVisitState>,
    ) -> bool {
      match visited[module_idx] {
        TlaVisitState::Visited(result) => return result,
        TlaVisitState::Visiting => return false,
        TlaVisitState::NotVisited => {}
      }

      visited[module_idx] = TlaVisitState::Visiting;
      let module = &module_table[module_idx];
      let is_self_tla = module
        .as_normal()
        .is_some_and(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait));
      if is_self_tla {
        // If the module itself contains top-level await, then it is TLA.
        visited[module_idx] = TlaVisitState::Visited(true);
        return true;
      }

      let contains_tla_dependency = module
        .import_records()
        .iter()
        // TODO: require TLA module should give a error
        .filter(|rec| matches!(rec.kind, ImportKind::Import))
        .any(|rec| {
          rec
            .resolved_module
            .is_some_and(|module_idx| is_tla(module_table[module_idx].idx(), module_table, visited))
        });

      visited[module_idx] = TlaVisitState::Visited(contains_tla_dependency);
      contains_tla_dependency
    }

    let mut visited = index_vec![TlaVisitState::NotVisited; self.module_table.modules.len()];

    self.module_table.modules.iter().filter_map(|m| m.as_normal()).for_each(|module| {
      self.metas[module.idx].is_tla_or_contains_tla_dependency =
        is_tla(module.idx, &self.module_table, &mut visited);
    });
  }
}
