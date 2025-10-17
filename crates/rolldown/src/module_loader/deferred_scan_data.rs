use crate::ecmascript::ecma_module_view_factory::normalize_side_effects;
use crate::module_loader::module_loader::VisitState;
use crate::{SharedOptions, SharedResolver, stages::scan_stage::NormalizedScanStageOutput};
use arcstr::ArcStr;
use rolldown_common::ImportKind;
use rolldown_common::side_effects::{DeterminedSideEffects, HookSideEffects};
use rolldown_error::BuildResult;
use rustc_hash::FxHashMap;

pub async fn defer_sync_scan_data(
  options: &SharedOptions,
  module_id_to_idx: &FxHashMap<ArcStr, VisitState>,
  resolver: &SharedResolver,
  scan_stage_output: &mut NormalizedScanStageOutput,
) -> BuildResult<()> {
  let Some(ref func) = options.defer_sync_scan_data else {
    return Ok(());
  };

  for data in func.exec().await? {
    let source_id = arcstr::ArcStr::from(data.id);
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
    normal.ecma_view.side_effects = match data.side_effects {
      Some(HookSideEffects::False) => DeterminedSideEffects::UserDefined(false),
      Some(HookSideEffects::NoTreeshake) => DeterminedSideEffects::NoTreeshake,
      _ => {
        // for Some(HookSideEffects::True) and None, we need to re resolve module source_id,
        // get package_json and re analyze the side effects
        let resolved_id = resolver
          // other params except `source_id` is not important, since we need `package_json`
          // from `resolved_id` to re analyze the side effects
          .resolve(None, source_id.as_str(), ImportKind::Import, normal.is_user_defined_entry)
          .expect("Should have resolved id")
          .into();
        normalize_side_effects(
          options,
          &resolved_id,
          Some(&normal.stmt_infos),
          Some(&normal.module_type),
          data.side_effects,
        )
        .await?
      }
    };
  }
  Ok(())
}
