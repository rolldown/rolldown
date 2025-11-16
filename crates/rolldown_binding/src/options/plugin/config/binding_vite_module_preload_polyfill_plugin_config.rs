use rolldown_plugin_vite_module_preload_polyfill::ViteModulePreloadPolyfillPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingViteModulePreloadPolyfillPluginConfig {
  pub is_server: Option<bool>,
}

impl From<BindingViteModulePreloadPolyfillPluginConfig> for ViteModulePreloadPolyfillPlugin {
  fn from(value: BindingViteModulePreloadPolyfillPluginConfig) -> Self {
    Self { is_server: value.is_server.unwrap_or_default() }
  }
}
