use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookRenderChunkOutput {
  pub code: String,
}

impl From<BindingHookRenderChunkOutput> for rolldown_plugin::HookRenderChunkOutput {
  fn from(value: BindingHookRenderChunkOutput) -> Self {
    Self { code: value.code }
  }
}
