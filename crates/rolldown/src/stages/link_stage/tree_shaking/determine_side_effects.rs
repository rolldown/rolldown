use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordMeta, Module, ModuleIdx, WrapKind, side_effects::DeterminedSideEffects,
};

use crate::stages::link_stage::LinkStage;

#[derive(Debug, Clone, Copy)]
enum SideEffectCache {
  None,
  Visited,
  Cache(DeterminedSideEffects),
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn determine_side_effects(&mut self) {
    let mut index_side_effects_cache =
      oxc_index::index_vec![SideEffectCache::None; self.module_table.modules.len()];

    for idx in 0..self.module_table.modules.len() {
      let module_idx = ModuleIdx::new(idx);
      let side_effects =
        self.determine_side_effects_for_module(module_idx, &mut index_side_effects_cache);
      if let Module::Normal(module) = &mut self.module_table[module_idx] {
        module.side_effects = side_effects;
      }
    }
  }

  fn determine_side_effects_for_module(
    &self,
    module_idx: ModuleIdx,
    cache: &mut IndexVec<ModuleIdx, SideEffectCache>,
  ) -> DeterminedSideEffects {
    let module = &self.module_table[module_idx];

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
          let has_side_effects = module.import_records.iter().any(|import_record| {
            if self
              .determine_side_effects_for_module(import_record.resolved_module, cache)
              .has_side_effects()
            {
              return true;
            }

            // Check for `export * from 'wrapped-module'` patterns.
            // These require runtime helpers (__reExport) and must be marked as having side effects
            // to ensure the module is included and properly initializes its dependencies.
            if import_record.kind == ImportKind::Import
              && import_record.meta.contains(ImportRecordMeta::IsExportStar)
            {
              if let Module::Normal(importee) = &self.module_table[import_record.resolved_module] {
                let importee_linking_info = &self.metas[importee.idx];
                return match importee_linking_info.wrap_kind() {
                  // If importee has dynamic exports (e.g., re-exports from CJS), we need side effects
                  WrapKind::None => importee_linking_info.has_dynamic_exports,
                  // Wrapped modules always need the side effect for proper initialization
                  WrapKind::Cjs | WrapKind::Esm => true,
                };
              }
            }

            false
          });

          let side_effects = DeterminedSideEffects::Analyzed(has_side_effects);
          cache[module_idx] = SideEffectCache::Cache(side_effects);

          side_effects
        }
        Module::External(_) => module_side_effects,
      },
    }
  }
}
