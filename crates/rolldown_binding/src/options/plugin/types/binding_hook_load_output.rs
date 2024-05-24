use derivative::Derivative;
use serde::Deserialize;

use super::binding_hook_side_effects::BindingHookSideEffects;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookLoadOutput {
  pub code: String,
  pub map: Option<String>,
  pub side_effects: Option<BindingHookSideEffects>,
}

impl TryFrom<BindingHookLoadOutput> for rolldown_plugin::HookLoadOutput {
  type Error = anyhow::Error;

  fn try_from(value: BindingHookLoadOutput) -> Result<Self, Self::Error> {
    Ok(rolldown_plugin::HookLoadOutput {
      code: value.code,
      map: value
        .map
        .map(|content| {
          rolldown_sourcemap::SourceMap::from_json_string(&content)
            .map_err(|e| anyhow::format_err!("SOURCEMAP_ERROR: {:?}", e))
        })
        .transpose()?,
      side_effects: value.side_effects.map(Into::into),
    })
  }
}
