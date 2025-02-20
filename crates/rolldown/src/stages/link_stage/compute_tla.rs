use rolldown_common::{EcmaModuleAstUsage, ImportKind, ModuleIdx, ModuleTable};
use rustc_hash::FxHashMap;

use super::LinkStage;

impl LinkStage<'_> {
  pub(super) fn compute_tla(&mut self) {
    // TODO: skip this phase if there is no module use TLA
    fn is_tla(
      module_idx: ModuleIdx,
      module_table: &ModuleTable,
      // `None` means the module is in visiting
      visited_map: &mut FxHashMap<ModuleIdx, Option<bool>>,
    ) -> bool {
      if let Some(memorized) = visited_map.get(&module_idx) {
        memorized.unwrap_or(false)
      } else {
        visited_map.insert(module_idx, None);
        let module = &module_table.modules[module_idx];
        let is_self_tla = module
          .as_normal()
          .is_some_and(|module| module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait));
        if is_self_tla {
          // If the module itself contains top-level await, then it is TLA.
          visited_map.insert(module_idx, Some(true));
          return true;
        }

        let contains_tla_dependency = module
          .import_records()
          .iter()
          // TODO: require TLA module should give a error
          .filter(|rec| matches!(rec.kind, ImportKind::Import))
          .any(|rec| {
            let importee = &module_table.modules[rec.resolved_module];
            is_tla(importee.idx(), module_table, visited_map)
          });

        visited_map.insert(module_idx, Some(contains_tla_dependency));
        contains_tla_dependency
      }
    }

    let mut visited_map = FxHashMap::default();

    self.module_table.modules.iter().filter_map(|m| m.as_normal()).for_each(|module| {
      self.metas[module.idx].is_tla_or_contains_tla_dependency =
        is_tla(module.idx, &self.module_table, &mut visited_map);
    });
  }
}
