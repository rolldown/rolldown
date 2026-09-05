use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordMeta, Module, ModuleIdx, StmtEvalFlags, WrapKind,
  side_effects::DeterminedSideEffects,
};

use crate::stages::link_stage::LinkStage;

/// A module's side-effect verdict as the scan stage left it, plus how many statement infos it had
/// then. `link()` appends bundler statements later (the `__esm`/`__commonJS` wrapper statement of
/// `wrap_modules` carries `UnknownSideEffect`), and those must not count as module side effects.
/// See `LinkStage::recompute_analyzed_side_effects`.
#[derive(Debug, Clone, Copy)]
pub struct ScanTimeSideEffects {
  pub side_effects: DeterminedSideEffects,
  pub stmt_info_count: usize,
}

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

  /// Re-derive the module verdicts after a pass relaxed statement flags.
  ///
  /// `determine_side_effects` runs before imports are bound because `bind_imports_and_exports`
  /// reads its result, but `cross_module_optimization` and
  /// `refine_stmt_side_effects_with_imported_constants` relax statement flags after that. A module
  /// whose only apparent side effect was such a statement must not stay side-effectful:
  /// `reference_needed_symbols` would keep a side-effect-only `import './mod'` of it and
  /// `include_statements` would emit it empty.
  ///
  /// Every `Analyzed` verdict is reset to what the scan stage computed and propagated again. A
  /// module in `relaxed_side_effect_modules` recomputes that base from its scan-time statements
  /// (`ScanTimeSideEffects::stmt_info_count`), exactly as `lazy_check_side_effects` did; the
  /// bundler statements appended since then are ignored, as they were in the first run.
  pub fn recompute_analyzed_side_effects(&mut self) {
    if self.relaxed_side_effect_modules.is_empty() {
      return;
    }
    for module in self.module_table.modules.iter_mut().filter_map(Module::as_normal_mut) {
      let scan_time = self.scan_time_side_effects[module.idx];
      module.side_effects = match scan_time.side_effects {
        DeterminedSideEffects::Analyzed(true)
          if self.relaxed_side_effect_modules.contains(&module.idx) =>
        {
          let has_side_effects = self.stmt_infos[module.idx]
            .iter()
            .take(scan_time.stmt_info_count)
            .any(|stmt_info| stmt_info.eval_flags.contains(StmtEvalFlags::UnknownSideEffect));
          DeterminedSideEffects::Analyzed(has_side_effects)
        }
        side_effects => side_effects,
      };
    }
    self.determine_side_effects();
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
          let has_side_effects = module
            .import_records
            .iter()
            .filter_map(|rec| rec.resolved_module.map(|module_idx| (rec, module_idx)))
            .any(|(import_record, module_idx)| {
              if self.determine_side_effects_for_module(module_idx, cache).has_side_effects() {
                return true;
              }

              // Check for `export * from 'wrapped-module'` patterns.
              // to ensure the module is included and properly initializes its dependencies.
              if import_record.kind == ImportKind::Import
                && import_record.meta.contains(ImportRecordMeta::IsExportStar)
              {
                if let Module::Normal(importee) = &self.module_table[module_idx] {
                  let importee_linking_info = &self.metas[importee.idx];
                  return match importee_linking_info.wrap_kind() {
                    // If importee has dynamic exports (e.g., re-exports from CJS), we need side effects
                    // to ensure the __reExport call is preserved.
                    //  ```js
                    // // index.js
                    // export * from './foo'; // importee wrap kind is `none`, but since `foo` has dynamic_export,
                    //                        // we need to preserve the `__reExport(index_exports, foo_ns)` call
                    //
                    // // foo.js
                    // export * from './bar' // importee wrap kind is `cjs`, preserved by default
                    //
                    // // bar.js
                    // module.exports = 1000
                    // ```
                    WrapKind::None => importee_linking_info.has_dynamic_exports,
                    // Wrapped modules always need the side effect(`init_xxx` for esm and `require_xxx` for cjs) for proper initialization
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
