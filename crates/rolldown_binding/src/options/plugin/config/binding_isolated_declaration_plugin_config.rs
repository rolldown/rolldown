use rolldown_plugin_isolated_declaration::IsolatedDeclarationPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingIsolatedDeclarationPluginConfig {
  pub strip_internal: Option<bool>,
}

impl From<BindingIsolatedDeclarationPluginConfig> for IsolatedDeclarationPlugin {
  fn from(value: BindingIsolatedDeclarationPluginConfig) -> Self {
    Self { strip_internal: value.strip_internal.unwrap_or_default() }
  }
}
