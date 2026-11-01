use oxc_index::IndexVec;
use rolldown_common::{
  ImportKind, ImportRecordMeta, IndexModules, Module, ModuleIdx, StmtEvalFlags, WrapKind,
  side_effects::DeterminedSideEffects,
};

use crate::{stages::link_stage::LinkStage, types::linking_metadata::LinkingMetadataVec};

/// Holds a module's scan-time side-effect verdict and statement count. `link()` later appends
/// wrapper statements that carry `UnknownSideEffect`, and those must not count as module side
/// effects. See `LinkStage::recompute_analyzed_side_effects`.
#[derive(Debug, Clone, Copy)]
pub struct ScanTimeSideEffects {
  pub side_effects: DeterminedSideEffects,
  pub stmt_info_count: usize,
}

impl LinkStage<'_> {
  /// Gives each module whose own statements have no side effects those of the modules that its
  /// import records reach; see [`linked_side_effects`].
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn determine_side_effects(&mut self) {
    let has_side_effects = linked_side_effects(&self.module_table.modules, &self.metas);
    for (idx, module) in self.module_table.modules.iter_mut_enumerated() {
      if let Module::Normal(module) = module
        && matches!(module.side_effects, DeterminedSideEffects::Analyzed(false))
      {
        module.side_effects = DeterminedSideEffects::Analyzed(has_side_effects[idx]);
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

/// Whether each module has side effects once linked. A module whose own statements have none,
/// `DeterminedSideEffects::Analyzed(false)`, has side effects when one of its import records
/// reaches a module that has some, or when it re-exports everything from a module whose
/// re-export call must stay. Side effects that statements, package.json or `no-treeshake` give
/// a module stay as they are.
///
/// Upstream walks the graph depth-first in module order and reads a module that the walk is
/// still inside as side-effect-free, so a module that reaches side effects only through an
/// import cycle could go either way: it depended on the order in which the scan numbered the
/// modules, which follows the order in which their tasks finish, and in an incremental session
/// on modules that nothing imports any more. This instead carries side effects from every module
/// that has them to the modules that import it until nothing changes, which depends on the graph
/// alone.
pub(crate) fn linked_side_effects(
  modules: &IndexModules,
  metas: &LinkingMetadataVec,
) -> IndexVec<ModuleIdx, bool> {
  propagate_side_effects(modules, |target| {
    let meta = &metas[target];
    match meta.wrap_kind() {
      WrapKind::None => meta.has_dynamic_exports,
      WrapKind::Cjs | WrapKind::Esm => true,
    }
  })
}

/// [`linked_side_effects`], with `star_keeps_call` saying whether a module's `export *` of the
/// normal module that it names keeps the call that initializes it or re-exports its dynamic
/// exports, as `export *` of a wrapped module or of one with dynamic exports does.
pub(crate) fn propagate_side_effects(
  modules: &IndexModules,
  star_keeps_call: impl Fn(ModuleIdx) -> bool,
) -> IndexVec<ModuleIdx, bool> {
  let mut has_side_effects: IndexVec<ModuleIdx, bool> =
    modules.iter().map(|module| module.side_effects().has_side_effects()).collect();
  // First resolve modules whose direct imports already have side effects or require a re-export
  // call, and count remaining Analyzed(false) modules. Only build reverse edges for those.
  let mut unresolved = Vec::new();
  for (idx, module) in modules.iter_enumerated() {
    let Module::Normal(module) = module else {
      continue;
    };
    if !matches!(module.side_effects, DeterminedSideEffects::Analyzed(false)) {
      continue;
    }
    let mut direct_se = false;
    for record in &module.import_records {
      let Some(target) = record.resolved_module else {
        continue;
      };
      if has_side_effects[target] {
        direct_se = true;
        break;
      }
      if record.kind == ImportKind::Import
        && record.meta.contains(ImportRecordMeta::IsExportStar)
        && modules[target].is_normal()
        && star_keeps_call(target)
      {
        direct_se = true;
        break;
      }
    }
    if direct_se {
      has_side_effects[idx] = true;
    } else if !module.import_records.is_empty() {
      unresolved.push(idx);
    }
  }
  if unresolved.is_empty() {
    return has_side_effects;
  }
  // The modules without side effects of their own that import each module.
  let mut importers: IndexVec<ModuleIdx, Vec<ModuleIdx>> =
    IndexVec::from_vec(vec![Vec::new(); modules.len()]);
  let mut pending = Vec::new();
  for idx in unresolved {
    let Module::Normal(module) = &modules[idx] else {
      continue;
    };
    let mut resolved_now = false;
    for record in &module.import_records {
      let Some(target) = record.resolved_module else {
        continue;
      };
      if has_side_effects[target] {
        resolved_now = true;
        break;
      }
    }
    if resolved_now {
      has_side_effects[idx] = true;
      pending.push(idx);
    } else {
      for record in &module.import_records {
        if let Some(target) = record.resolved_module {
          importers[target].push(idx);
        }
      }
    }
  }
  while let Some(target) = pending.pop() {
    for &importer in &importers[target] {
      if !has_side_effects[importer] {
        has_side_effects[importer] = true;
        pending.push(importer);
      }
    }
  }
  has_side_effects
}
