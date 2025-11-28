use rolldown_common::ImportKind;
use rolldown_common::side_effects::{DeterminedSideEffects, HookSideEffects};
use rolldown_error::BuildResult;
use rustc_hash::FxHashMap;

use crate::ecmascript::ecma_module_view_factory::normalize_side_effects;
use crate::{SharedOptions, SharedResolver, stages::scan_stage::NormalizedScanStageOutput};

pub async fn defer_sync_scan_data(
  options: &SharedOptions,
  resolver: &SharedResolver,
  scan_stage_output: &mut NormalizedScanStageOutput,
) -> BuildResult<()> {
  let Some(ref func) = options.defer_sync_scan_data else {
    return Ok(());
  };

  let result = func.exec().await?;
  if result.is_empty() {
    return Ok(());
  }

  // TODO: In incremental build mode, we can directly use `module_id_to_idx` map from `ScanStageCache`
  let module_id_to_idx = scan_stage_output
    .module_table
    .modules
    .iter()
    .filter_map(|m| m.as_normal().map(|n| (n.id.resource_id().clone(), m.idx())))
    .collect::<FxHashMap<_, _>>();

  for data in result {
    let Some(&module_idx) = module_id_to_idx.get(data.id.as_str()) else {
      continue;
    };
    let Some(normal_module) = scan_stage_output
      .module_table
      .modules
      .get_mut(module_idx)
      .and_then(|module| module.as_normal_mut())
    else {
      continue;
    };
    // TODO: Document this and recommend user to return `moduleSideEffects` in hook return
    // value rather than mutate the `ModuleInfo`
    normal_module.ecma_view.side_effects = match data.side_effects {
      Some(HookSideEffects::False) => DeterminedSideEffects::UserDefined(false),
      Some(HookSideEffects::NoTreeshake) => DeterminedSideEffects::NoTreeshake,
      _ => {
        // for Some(HookSideEffects::True) and None, we need to re resolve module source_id,
        // get package_json and re analyze the side effects
        let resolved_id = resolver
          // other params except `source_id` is not important, since we need `package_json`
          // from `resolved_id` to re analyze the side effects
          .resolve(None, &data.id, ImportKind::Import, normal_module.is_user_defined_entry)
          .expect("Should have resolved id")
          .into();
        normalize_side_effects(
          options,
          &resolved_id,
          Some(&normal_module.stmt_infos),
          Some(&normal_module.module_type),
          data.side_effects,
        )
        .await?
      }
    };
  }
  Ok(())
}
