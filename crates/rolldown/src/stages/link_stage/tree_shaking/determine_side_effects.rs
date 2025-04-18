use oxc_index::IndexVec;
use rolldown_common::{IndexModules, Module, ModuleIdx, side_effects::DeterminedSideEffects};

use crate::stages::link_stage::LinkStage;

impl LinkStage<'_> {
  pub fn determine_side_effects(&mut self) {
    #[derive(Debug, Clone, Copy)]
    enum SideEffectCache {
      None,
      Visited,
      Cache(DeterminedSideEffects),
    }
    type IndexSideEffectsCache = IndexVec<ModuleIdx, SideEffectCache>;

    fn determine_side_effects_for_module(
      cache: &mut IndexSideEffectsCache,
      module_idx: ModuleIdx,
      modules: &IndexModules,
    ) -> DeterminedSideEffects {
      let module = &modules[module_idx];

      match &mut cache[module_idx] {
        SideEffectCache::None => {
          cache[module_idx] = SideEffectCache::Visited;
        }
        SideEffectCache::Visited => {
          return *module.side_effects();
        }
        SideEffectCache::Cache(v) => {
          return *v;
        }
      }

      let ret = match *module.side_effects() {
        // should keep as is if the side effects is derived from package.json, it is already
        // true or `no-treeshake`
        DeterminedSideEffects::UserDefined(_) | DeterminedSideEffects::NoTreeshake => {
          *module.side_effects()
        }
        DeterminedSideEffects::Analyzed(v) if v => *module.side_effects(),
        // this branch means the side effects of the module is analyzed `false`
        DeterminedSideEffects::Analyzed(_) => match module {
          Module::Normal(module) => DeterminedSideEffects::Analyzed(
            module.import_records.iter().filter(|rec| !rec.is_dummy()).any(|import_record| {
              determine_side_effects_for_module(cache, import_record.resolved_module, modules)
                .has_side_effects()
            }),
          ),
          Module::External(module) => module.side_effects,
        },
      };

      cache[module_idx] = SideEffectCache::Cache(ret);

      ret
    }

    let mut index_side_effects_cache =
      oxc_index::index_vec![SideEffectCache::None; self.module_table.modules.len()];
    let index_module_side_effects = self
      .module_table
      .modules
      .iter()
      .map(|module| {
        determine_side_effects_for_module(
          &mut index_side_effects_cache,
          module.idx(),
          &self.module_table.modules,
        )
      })
      .collect::<Vec<_>>();

    self.module_table.modules.iter_mut().zip(index_module_side_effects).for_each(
      |(module, side_effects)| {
        if let Module::Normal(module) = module {
          module.side_effects = side_effects;
        }
      },
    );
  }
}
