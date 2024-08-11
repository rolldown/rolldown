use derivative::Derivative;
use serde::Deserialize;

use crate::types::binding_sourcemap::BindingSourcemap;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookRenderChunkOutput {
  pub code: String,
  pub map: Option<BindingSourcemap>,
}

impl TryFrom<BindingHookRenderChunkOutput> for rolldown_plugin::HookRenderChunkOutput {
  type Error = anyhow::Error;

  fn try_from(value: BindingHookRenderChunkOutput) -> Result<Self, Self::Error> {
    Ok(rolldown_plugin::HookRenderChunkOutput {
      code: value.code,
      map: value.map.map(TryInto::try_into).transpose()?,
    })
  }
}
