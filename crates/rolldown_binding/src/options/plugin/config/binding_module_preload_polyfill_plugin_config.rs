use rolldown_plugin_module_preload_polyfill::ModulePreloadPolyfillPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingModulePreloadPolyfillPluginConfig {
  pub is_server: Option<bool>,
}

impl From<BindingModulePreloadPolyfillPluginConfig> for ModulePreloadPolyfillPlugin {
  fn from(value: BindingModulePreloadPolyfillPluginConfig) -> Self {
    Self { is_server: value.is_server.unwrap_or_default() }
  }
}
