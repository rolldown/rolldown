use derivative::Derivative;
use rolldown_error::BuildError;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookLoadOutput {
  pub code: String,
  pub map: Option<String>,
}

impl TryFrom<BindingHookLoadOutput> for rolldown_plugin::HookLoadOutput {
  type Error = BuildError;

  fn try_from(value: BindingHookLoadOutput) -> Result<Self, Self::Error> {
    Ok(rolldown_plugin::HookLoadOutput {
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
