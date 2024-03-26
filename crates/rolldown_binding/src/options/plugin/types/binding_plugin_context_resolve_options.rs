use derivative::Derivative;
use rolldown_plugin::PluginContextResolveOptions;
use serde::Deserialize;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingPluginContextResolveOptions {
  pub import_kind: String,
}

impl Default for BindingPluginContextResolveOptions {
  fn default() -> Self {
    Self { import_kind: "import".to_string() }
  }
}

impl TryFrom<BindingPluginContextResolveOptions> for PluginContextResolveOptions {
  type Error = String;

  fn try_from(value: BindingPluginContextResolveOptions) -> Result<Self, Self::Error> {
    Ok(Self { import_kind: value.import_kind.as_str().try_into()? })
  }
}
