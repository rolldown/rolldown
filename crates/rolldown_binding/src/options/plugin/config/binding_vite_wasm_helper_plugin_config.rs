use rolldown_plugin_vite_wasm_helper::ViteWasmHelperPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingViteWasmHelperPluginConfig {
  pub decoded_base: String,
}

impl From<BindingViteWasmHelperPluginConfig> for ViteWasmHelperPlugin {
  fn from(value: BindingViteWasmHelperPluginConfig) -> Self {
    Self { decoded_base: value.decoded_base }
  }
}
