use rolldown_common::side_effects::{DeterminedSideEffects, HookSideEffects};
use rolldown_common::{ModuleId, SavedJsModuleOptions};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rustc_hash::FxHashMap;

use crate::ecmascript::ecma_module_view_factory::normalize_side_effects;
use crate::module_loader::module_loader::VisitState;
use crate::{SharedOptions, stages::scan_stage::NormalizedScanStageOutput};

/// `saved_js_module_options` is `None` without the persistent cache.
pub async fn defer_sync_scan_data(
  options: &SharedOptions,
  saved_js_module_options: Option<&SavedJsModuleOptions>,
  module_id_to_idx: &FxHashMap<ModuleId, VisitState>,
  scan_stage_output: &mut NormalizedScanStageOutput,
) -> BuildResult<()> {
  let Some(ref func) = options.defer_sync_scan_data else {
    return Ok(());
  };

  // A hook that the persistent cache skipped cannot return `moduleSideEffects` again. The
  // JavaScript side takes saved values only when it reads a module, so it may not know these.
  // It needs them to tell which assignments in its `moduleOptionMap` are older.
  let saved_side_effects =
    saved_js_module_options.map(SavedJsModuleOptions::side_effects).unwrap_or_default();
  let result = func.exec(saved_side_effects).await?;

  let mut errors: Vec<BuildDiagnostic> = vec![];
  for data in result {
    let source_id = ModuleId::new(data.id.as_str());
    let Some(state) = module_id_to_idx.get(&source_id) else {
      continue;
    };
    let module_idx = state.idx();
    let Some(module) = scan_stage_output.module_table.modules.get_mut(module_idx) else {
      continue;
    };
    let Some(normal) = module.as_normal_mut() else {
      continue;
    };
    // TODO: Document this and recommend user to return `moduleSideEffects` in hook return
    // value rather than mutate the `ModuleInfo`
    let side_effects = match data.side_effects {
      Some(HookSideEffects::False) => DeterminedSideEffects::UserDefined(false),
      Some(HookSideEffects::NoTreeshake) => DeterminedSideEffects::NoTreeshake,
      _ => {
        // for Some(HookSideEffects::True) and None,
        // we need to re analyze the side effects
        match normalize_side_effects(
          options,
          &normal.originative_resolved_id,
          Some(&scan_stage_output.stmt_infos[module_idx]),
          data.side_effects,
        )
        .await
        {
          Ok(side_effects) => side_effects,
          Err(error) => {
            errors.extend(error.into_vec());
            continue;
          }
        }
      }
    };
    normal.ecma_view.side_effects = side_effects;
  }
  if errors.is_empty() { Ok(()) } else { Err(errors.into()) }
}
