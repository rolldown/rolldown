use rolldown_common::{
  Module, ModuleSideEffectsOverrides, WrittenSideEffects, side_effects::DeterminedSideEffects,
};
use rolldown_error::{BuildDiagnostic, BuildResult};

use crate::ecmascript::ecma_module_view_factory::{
  OptionSideEffects, normalize_side_effects, read_option_side_effects,
};
use crate::{SharedOptions, stages::scan_stage::NormalizedScanStageOutput};

/// Applies the `getModuleInfo(id).moduleSideEffects` writes of plugins to the modules of
/// `scan_stage_output`, for tree shaking. See internal-docs/cache/implementation.md.
pub async fn apply_module_side_effects_overrides(
  options: &SharedOptions,
  overrides: &ModuleSideEffectsOverrides,
  scan_stage_output: &mut NormalizedScanStageOutput,
) -> BuildResult<()> {
  if overrides.is_empty() {
    return Ok(());
  }
  let mut errors: Vec<BuildDiagnostic> = vec![];
  for module in &mut scan_stage_output.module_table.modules {
    match module {
      Module::Normal(normal) => {
        let Some(WrittenSideEffects(written)) = overrides.get(normal.id.as_str()) else {
          continue;
        };
        match normalize_side_effects(
          options,
          overrides,
          &normal.originative_resolved_id,
          Some(&scan_stage_output.stmt_infos[normal.idx]),
          written,
        )
        .await
        {
          Ok(side_effects) => normal.ecma_view.side_effects = side_effects,
          Err(error) => errors.extend(error.into_vec()),
        }
      }
      Module::External(external) => {
        let Some(WrittenSideEffects(written)) = overrides.get(external.id.as_str()) else {
          continue;
        };
        if let Some(written) = written {
          external.side_effects = written.declared();
          continue;
        }
        match read_option_side_effects(options, overrides, &external.id, true).await {
          Ok(OptionSideEffects::TreeshakeDisabled) => {
            external.side_effects = DeterminedSideEffects::NoTreeshake;
          }
          Ok(OptionSideEffects::Decided(value)) => {
            external.side_effects = DeterminedSideEffects::UserDefined(value);
          }
          Ok(OptionSideEffects::Undecided) => {
            external.side_effects = external.side_effects_without_option;
          }
          Err(error) => errors.extend(error.into_vec()),
        }
      }
    }
  }
  if errors.is_empty() { Ok(()) } else { Err(errors.into()) }
}
