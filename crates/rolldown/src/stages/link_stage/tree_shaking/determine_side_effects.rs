use oxc_index::IndexVec;
use rolldown_common::{Module, ModuleIdx, side_effects::DeterminedSideEffects};

use crate::stages::link_stage::LinkStage;

#[derive(Debug, Clone, Copy)]
enum SideEffectCache {
  None,
  Visited,
  Cache(DeterminedSideEffects),
}

impl LinkStage<'_> {
  pub fn determine_side_effects(&mut self) {
    let mut index_side_effects_cache =
      oxc_index::index_vec![SideEffectCache::None; self.module_table.modules.len()];

    for idx in 0..self.module_table.modules.len() {
      let module_idx = ModuleIdx::new(idx);
      let side_effects =
        self.determine_side_effects_for_module(module_idx, &mut index_side_effects_cache);
      if let Module::Normal(module) = &mut self.module_table.modules[module_idx] {
        module.side_effects = side_effects;
      }
    }
  }

  fn determine_side_effects_for_module(
    &self,
    module_idx: ModuleIdx,
    cache: &mut IndexVec<ModuleIdx, SideEffectCache>,
  ) -> DeterminedSideEffects {
    let module = &self.module_table.modules[module_idx];

    match cache[module_idx] {
      SideEffectCache::None => {
        cache[module_idx] = SideEffectCache::Visited;
      }
      SideEffectCache::Visited => {
        return *module.side_effects();
      }
      SideEffectCache::Cache(v) => {
        return v;
      }
    }

    let module_side_effects = *module.side_effects();
    match module_side_effects {
      // should keep as is if the side effects is derived from package.json, it is already
      // true or `no-treeshake`
      DeterminedSideEffects::Analyzed(true)
      | DeterminedSideEffects::UserDefined(_)
      | DeterminedSideEffects::NoTreeshake => module_side_effects,
      // this branch means the side effects of the module is analyzed `false`
      DeterminedSideEffects::Analyzed(false) => match module {
        Module::Normal(module) => {
          let side_effects = DeterminedSideEffects::Analyzed(
            module.import_records.iter().filter(|rec| !rec.is_dummy()).any(|import_record| {
              self
                .determine_side_effects_for_module(import_record.resolved_module, cache)
                .has_side_effects()
            }),
          );

          cache[module_idx] = SideEffectCache::Cache(side_effects);

          side_effects
        }
        Module::External(_) => module_side_effects,
      },
    }
  }
}
