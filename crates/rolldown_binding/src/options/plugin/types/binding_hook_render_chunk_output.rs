use derivative::Derivative;
use rolldown_error::BuildError;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookRenderChunkOutput {
  pub code: String,
  pub map: Option<String>,
}

impl TryFrom<BindingHookRenderChunkOutput> for rolldown_plugin::HookRenderChunkOutput {
  type Error = BuildError;

  fn try_from(value: BindingHookRenderChunkOutput) -> Result<Self, Self::Error> {
    Ok(rolldown_plugin::HookRenderChunkOutput {
      code: value.code,
      map: value
        .map
        .map(|content| {
          rolldown_sourcemap::SourceMap::from_json_string(&content)
            .map_err(BuildError::sourcemap_error)
        })
        .transpose()?,
    })
  }
}
