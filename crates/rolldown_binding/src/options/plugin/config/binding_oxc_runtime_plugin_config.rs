use rolldown_plugin_oxc_runtime::OxcRuntimePlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingOxcRuntimePluginConfig {
  pub resolve_base: Option<String>,
}

impl From<BindingOxcRuntimePluginConfig> for OxcRuntimePlugin {
  fn from(value: BindingOxcRuntimePluginConfig) -> Self {
    Self { resolve_base: value.resolve_base }
  }
}
