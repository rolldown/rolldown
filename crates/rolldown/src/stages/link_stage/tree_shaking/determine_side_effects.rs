use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordMeta, Module, ModuleIdx, StmtEvalFlags, WrapKind,
  side_effects::DeterminedSideEffects,
};
use rolldown_utils::IndexBitSet;

use crate::stages::link_stage::LinkStage;

/// Holds a module's scan-time side-effect verdict and statement count. `link()` later appends
/// wrapper statements that carry `UnknownSideEffect`, and those must not count as module side
/// effects. See `LinkStage::recompute_analyzed_side_effects`.
#[derive(Debug, Clone, Copy)]
pub struct ScanTimeSideEffects {
  pub side_effects: DeterminedSideEffects,
  pub stmt_info_count: usize,
}

impl LinkStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn determine_side_effects(&mut self) {
    let mut has_side_effects: IndexBitSet<ModuleIdx> =
      IndexBitSet::new(self.module_table.modules.len());
    has_side_effects.extend(
      self
        .module_table
        .modules
        .iter()
        .filter(|module| module.side_effects().has_side_effects())
        .map(Module::idx),
    );
    let mut dependents: IndexVec<ModuleIdx, Vec<ModuleIdx>> =
      oxc_index::index_vec![Vec::new(); self.module_table.modules.len()];

    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      // User-defined verdicts override dependency effects. Modules already known to have
      // effects do not need dependency edges either.
      if !matches!(module.side_effects, DeterminedSideEffects::Analyzed(false)) {
        continue;
      }
      for import_record in &module.import_records {
        let Some(importee_idx) = import_record.resolved_module else { continue };
        dependents[importee_idx].push(module.idx);

        // Re-exporting a wrapped module must preserve its initialization. Dynamic exports
        // also need the generated `__reExport` call, even without a wrapper.
        if import_record.kind == ImportKind::Import
          && import_record.meta.contains(ImportRecordMeta::IsExportStar)
          && let Module::Normal(importee) = &self.module_table[importee_idx]
        {
          let linking_info = &self.metas[importee.idx];
          let reexport_has_effects = match linking_info.wrap_kind() {
            WrapKind::None => linking_info.has_dynamic_exports,
            WrapKind::Cjs | WrapKind::Esm => true,
          };
          if reexport_has_effects {
            has_side_effects.set_bit(module.idx);
          }
        }
      }
    }

    // Propagate effects back to importers. A recursive cache can mark a cycle member as
    // effect-free before visiting an effectful dependency of another member. Each module
    // enters this worklist at most once, so cycles converge in O(modules + imports).
    // See internal-docs/linking/reference-needed-symbols/implementation.md.
    let mut pending: Vec<_> = has_side_effects.index_of_one().collect();
    while let Some(module_idx) = pending.pop() {
      for &importer_idx in &dependents[module_idx] {
        if has_side_effects.set_bit(importer_idx) {
          pending.push(importer_idx);
        }
      }
    }

    for module in self.module_table.modules.iter_mut().filter_map(Module::as_normal_mut) {
      if matches!(module.side_effects, DeterminedSideEffects::Analyzed(false)) {
        module.side_effects = DeterminedSideEffects::Analyzed(has_side_effects.has_bit(module.idx));
      }
    }
  }

  /// Re-derive the module verdicts after a pass relaxed statement flags.
  ///
  /// `determine_side_effects` must run before `bind_imports_and_exports`, which reads its result,
  /// but `cross_module_optimization` and `refine_stmt_side_effects_with_imported_constants` relax
  /// flags later. Without this step, a module whose only side effect was such a statement keeps its
  /// side-effect-only imports, and `include_statements` emits it empty.
  ///
  /// Every `Analyzed` verdict resets to its scan-time value. A relaxed module recomputes it from
  /// its scan-time statements only, as `lazy_check_side_effects` did, so appended bundler
  /// statements do not count.
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
}
