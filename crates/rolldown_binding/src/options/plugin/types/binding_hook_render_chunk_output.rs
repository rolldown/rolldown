use rolldown_error::BuildDiagnostic;

use crate::types::binding_sourcemap::BindingSourcemap;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default, Debug)]
pub struct BindingHookRenderChunkOutput {
  pub code: String,
  pub map: Option<BindingSourcemap>,
}

impl TryFrom<BindingHookRenderChunkOutput> for rolldown_plugin::HookRenderChunkOutput {
  type Error = BuildDiagnostic;

  fn try_from(value: BindingHookRenderChunkOutput) -> Result<Self, Self::Error> {
    Ok(rolldown_plugin::HookRenderChunkOutput {
      code: value.code,
      map: value.map.map(TryInto::try_into).transpose()?,
    })
  }
}
